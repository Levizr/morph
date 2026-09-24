#include "window.h"
#include "window_manager.h"
#include "renderers/flash/flash.h"
#include "renderers/forge/forge.h"
#include "renderers/forge/damage.h"
#ifdef MORPH_FEATURE_INPUT
#include "../ui/input.h"
#endif
#include <GLFW/glfw3.h>
// X11 pointer grab for scrollbar thumb drags (devtools-style: the drag
// keeps tracking past the window edge with the cursor visible). App-side
// Xlib only — no GLFW rebuild needed. Non-X11 builds skip the grab and
// keep node-capture behavior inside the window.
#if defined(__linux__) && !defined(__ANDROID__)
#define GLFW_EXPOSE_NATIVE_X11
#include <GLFW/glfw3native.h>
#include <X11/Xlib.h>
#endif
#include <algorithm>
// <print> is C++23 but not in libc++ until LLVM 17 (macOS Xcode 16 and
// older lack it), so include it only where the toolchain provides it.
#if __has_include(<print>)
#include <print>
#endif

RepaintHookFn g_repaintHook = nullptr;

// Double-click detection: threshold in seconds
static double s_lastClickTime = 0.0;
static const double DBL_CLICK_THRESHOLD = 0.3;
// Pressed-button bitmask for e.buttons (bit = GLFW button index).
static int s_buttonsDown = 0;
// Consecutive click count for e.detail + last clicked node.
static int s_clickCount = 0;
static MorphNode* s_lastClickNode = nullptr;
// True while an X11 pointer grab for a scrollbar drag is held.
static bool s_pointerGrabActive = false;

// Grab the pointer so a scrollbar thumb drag keeps receiving motion and
// button events past the window edge (cursor stays visible). No-op when
// X11 is unavailable (Wayland session, grab conflict) — the drag then
// tracks inside the window only, as before.
static void grabPointerForDrag(GLFWwindow* win)
{
#if defined(__linux__) && !defined(__ANDROID__)
    if (s_pointerGrabActive || !win)
    {
        return;
    }
    Display* dpy = glfwGetX11Display();
    ::Window xw = glfwGetX11Window(win);
    if (!dpy || !xw)
    {
        return;
    }
    int rc = XGrabPointer(dpy, xw, True,
        ButtonPressMask | ButtonReleaseMask | PointerMotionMask,
        GrabModeAsync, GrabModeAsync, None, None, CurrentTime);
    if (rc == GrabSuccess)
    {
        s_pointerGrabActive = true;
    }
#else
    (void)win;
#endif
}

static void ungrabPointerForDrag()
{
#if defined(__linux__) && !defined(__ANDROID__)
    if (!s_pointerGrabActive)
    {
        return;
    }
    Display* dpy = glfwGetX11Display();
    if (dpy)
    {
        XUngrabPointer(dpy, CurrentTime);
        XFlush(dpy);
    }
    s_pointerGrabActive = false;
#endif
}
// Node currently :active (pressed) — cleared on release / tree rebuild
// Any live GLFWwindow, kept so widgets can reach the OS clipboard without
// depending on GLFW themselves.
static GLFWwindow* s_clipboardWindow = nullptr;

// Used by MorphWindow::render()'s non-dirty path before its (later) definition.
static void clearAllDirty(MorphNode *n);

namespace morph {
void setClipboard(const std::string &text) {
    if (s_clipboardWindow)
        glfwSetClipboardString(s_clipboardWindow, text.c_str());
}
std::string getClipboard() {
    if (!s_clipboardWindow) return "";
    const char *c = glfwGetClipboardString(s_clipboardWindow);
    return c ? std::string(c) : std::string();
}
} // namespace morph

// :hover / :active match a *set*: the pointer node plus every ancestor. These
// helpers only fire onHover/onActive on nodes whose membership in the set
// actually changed, so the pointer sliding from a button into its own label
// does not re-trigger the button's hover/active transition.
static int _chainOf(MorphNode* n, MorphNode* out[64]) {
    int len = 0;
    while (n && len < 64) {
        out[len++] = n;
        n = n->parent;
    }
    return len;
}

static void _fireHoverDelta(MorphNode* oldNode, MorphNode* newNode) {
    MorphNode* oldChain[64];
    MorphNode* newChain[64];
    int ol = _chainOf(oldNode, oldChain);
    int nl = _chainOf(newNode, newChain);
    int common = 0;
    while (common < ol && common < nl && oldChain[ol - 1 - common] == newChain[nl - 1 - common])
        common++;
    for (int i = 0; i < ol - common; ++i)
        oldChain[i]->onHover(false);
    for (int i = 0; i < nl - common; ++i)
        newChain[i]->onHover(true);
}

static void _applyActiveChain(MorphNode* n, bool state) {
    for (MorphNode* a = n; a; a = a->parent)
        a->onActive(state);
}

void MorphWindow::mouseButtonCb(GLFWwindow *win, int btn, int act, int mods)
{
    // Right press opens the context menu (no press tracking or focus
    // change); the release only updates the button mask and dispatches.
    if (btn == GLFW_MOUSE_BUTTON_2)
    {
        auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
        if (!self || !self->m_root)
            return;
        double mx, my;
        glfwGetCursorPos(win, &mx, &my);
        MorphEvent e;
        e.button = btn;
        e.x = (float)mx;
        e.y = (float)my;
        if (act == GLFW_PRESS)
            s_buttonsDown |= (1 << btn);
        else
            s_buttonsDown &= ~(1 << btn);
        e.buttons = s_buttonsDown;
        e.type = (act == GLFW_PRESS) ? EventType::ContextMenu : EventType::MouseUp;
        e.detail = 0;
        self->m_root->dispatchEvent(e, (float)mx, (float)my);
        return;
    }
    if (btn == GLFW_MOUSE_BUTTON_1)
    {
        (void)mods;
        auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
        if (!self || !self->m_root)
            return;
        double mx, my;
        glfwGetCursorPos(win, &mx, &my);
        MorphEvent e;
        e.type = (act == GLFW_PRESS) ? EventType::MouseDown : EventType::MouseUp;
        e.button = btn;
        e.x = (float)mx;
        e.y = (float)my;
        if (act == GLFW_PRESS)
            s_buttonsDown |= (1 << btn);
        else
            s_buttonsDown &= ~(1 << btn);
        e.buttons = s_buttonsDown;

        // Press node + click count up front so every dispatched event
        // (down, up, pointer, click) carries the current e.detail.
        MorphNode* pressNodeNow = nullptr;
        if (act == GLFW_PRESS)
        {
            pressNodeNow = self->m_root->hitTest((float)mx, (float)my);
            double now = glfwGetTime();
            if (pressNodeNow == s_lastClickNode && now - s_lastClickTime < DBL_CLICK_THRESHOLD)
                s_clickCount++;
            else
                s_clickCount = 1;
            s_lastClickNode = pressNodeNow;
        }
        e.detail = s_clickCount;
        // End a mouse drag started inside a captured node (e.g. <input>
        // selection, scrollbar thumb) even when the button is released
        // outside its box. Without this a release outside the scrollbar
        // leaves scrollDragging set and the thumb follows the cursor
        // forever, held or not.
        if (act == GLFW_RELEASE && MorphNode::s_mouseCapture)
        {
            MorphNode* cap = MorphNode::s_mouseCapture;
            e.type = EventType::MouseUp;
            cap->onEvent(e);
#ifdef MORPH_FEATURE_SCROLL
            cap->scrollDragging = false;
#endif
            ungrabPointerForDrag();
            MorphNode::s_mouseCapture = nullptr;
            e.type = EventType::MouseUp;   // normal dispatch still runs below
        }
        else if (act == GLFW_RELEASE && s_pointerGrabActive)
        {
            // Captured node died mid-drag (conditional branch swapped):
            // drop the orphaned grab so motion events flow normally again.
            ungrabPointerForDrag();
        }

        self->m_root->dispatchEvent(e, (float)mx, (float)my);
        if (act == GLFW_PRESS)
        {
            e.type = EventType::PointerDown;
            self->m_root->dispatchEvent(e, (float)mx, (float)my);
            e.type = EventType::MouseDown;
        }
        else
        {
            e.type = EventType::PointerUp;
            self->m_root->dispatchEvent(e, (float)mx, (float)my);
            e.type = EventType::MouseUp;
        }

        // :active pseudo-class — apply on press, release on button up
        if (act == GLFW_PRESS)
        {
            if (MorphNode::s_activePressNode)
                _applyActiveChain(MorphNode::s_activePressNode, false);
            MorphNode::s_activePressNode = pressNodeNow;
            if (MorphNode::s_activePressNode)
                _applyActiveChain(MorphNode::s_activePressNode, true);

            // ── Keyboard focus model ────────────────────────────
            // Clicking an enabled <input> focuses it (caret placement and
            // drag-selection are handled by the input itself via onEvent);
            // clicking anywhere else releases focus, browser-style.
#ifdef MORPH_FEATURE_INPUT
            if (MorphNode::s_activePressNode && MorphNode::s_activePressNode->type == NodeType::Input && !static_cast<InputNode *>(MorphNode::s_activePressNode)->disabled)
                MorphNode::s_activePressNode->requestFocus();
            else if (MorphNode::s_focusedNode)
                MorphNode::s_focusedNode->blur();
#endif
#ifdef MORPH_FEATURE_SCROLL
            // A thumb press captured the node above: grab the pointer so
            // the drag survives past the window edge. Every press, not
            // just rapid double-presses.
            if (MorphNode::s_mouseCapture && MorphNode::s_mouseCapture->scrollDragging)
            {
                grabPointerForDrag(win);
            }
#endif
        }
        else
        {
            // Click fires on release, only when press and release hit the
            // same node — press-drag-release elsewhere is a drag, not a
            // click. DoubleClick follows the same match on second release.
            MorphNode* pressNode = MorphNode::s_activePressNode;
            if (pressNode)
            {
                MorphNode* releaseNode = self->m_root->hitTest((float)mx, (float)my);
                if (releaseNode == pressNode)
                {
                    double now = glfwGetTime();
                    e.type = EventType::Click;
                    self->m_root->dispatchEvent(e, (float)mx, (float)my);
                    if (now - s_lastClickTime < DBL_CLICK_THRESHOLD)
                    {
                        e.type = EventType::DoubleClick;
                        self->m_root->dispatchEvent(e, (float)mx, (float)my);
                    }
                    s_lastClickTime = now;
                }
                _applyActiveChain(pressNode, false);
            }
            MorphNode::s_activePressNode = nullptr;
        }
    }
}

// glfwGetKeyName only names printable keys; control keys (arrows, Home,
// End, Delete...) come back NULL, so map the common ones by GLFW code.
static std::string keyEventName(int key, int scancode)
{
    switch (key) {
        case GLFW_KEY_ESCAPE:     return "escape";
        case GLFW_KEY_ENTER:      return "enter";
        case GLFW_KEY_KP_ENTER:   return "kp_enter";
        case GLFW_KEY_TAB:        return "tab";
        case GLFW_KEY_BACKSPACE:  return "backspace";
        case GLFW_KEY_INSERT:     return "insert";
        case GLFW_KEY_DELETE:     return "delete";
        case GLFW_KEY_RIGHT:      return "right";
        case GLFW_KEY_LEFT:       return "left";
        case GLFW_KEY_DOWN:       return "down";
        case GLFW_KEY_UP:         return "up";
        case GLFW_KEY_PAGE_UP:    return "page_up";
        case GLFW_KEY_PAGE_DOWN:  return "page_down";
        case GLFW_KEY_HOME:       return "home";
        case GLFW_KEY_END:        return "end";
        default: break;
    }
    if (key >= GLFW_KEY_F1 && key <= GLFW_KEY_F12)
        return "f" + std::to_string(key - GLFW_KEY_F1 + 1);
    if (const char *n = glfwGetKeyName(key, scancode))
        return n;
    if (key == GLFW_KEY_SPACE) return "space";
    if (key == GLFW_KEY_LEFT_SHIFT || key == GLFW_KEY_RIGHT_SHIFT) return "shift";
    if (key == GLFW_KEY_LEFT_CONTROL || key == GLFW_KEY_RIGHT_CONTROL) return "control";
    if (key == GLFW_KEY_LEFT_ALT || key == GLFW_KEY_RIGHT_ALT) return "alt";
    if (key == GLFW_KEY_LEFT_SUPER || key == GLFW_KEY_RIGHT_SUPER) return "meta";
    return "";
}

// Physical key code (KeyA, Digit1, Escape, …) for e.code.
static std::string keyCodeName(int key)
{
    if (key >= GLFW_KEY_A && key <= GLFW_KEY_Z)
        return std::string("Key") + (char)('A' + key - GLFW_KEY_A);
    if (key >= GLFW_KEY_0 && key <= GLFW_KEY_9)
        return std::string("Digit") + (char)('0' + key - GLFW_KEY_0);
    if (key >= GLFW_KEY_F1 && key <= GLFW_KEY_F12)
        return "F" + std::to_string(key - GLFW_KEY_F1 + 1);
    switch (key) {
        case GLFW_KEY_ESCAPE: return "Escape";
        case GLFW_KEY_ENTER: case GLFW_KEY_KP_ENTER: return "Enter";
        case GLFW_KEY_TAB: return "Tab";
        case GLFW_KEY_BACKSPACE: return "Backspace";
        case GLFW_KEY_INSERT: return "Insert";
        case GLFW_KEY_DELETE: return "Delete";
        case GLFW_KEY_RIGHT: return "ArrowRight";
        case GLFW_KEY_LEFT: return "ArrowLeft";
        case GLFW_KEY_DOWN: return "ArrowDown";
        case GLFW_KEY_UP: return "ArrowUp";
        case GLFW_KEY_PAGE_UP: return "PageUp";
        case GLFW_KEY_PAGE_DOWN: return "PageDown";
        case GLFW_KEY_HOME: return "Home";
        case GLFW_KEY_END: return "End";
        case GLFW_KEY_SPACE: return "Space";
        case GLFW_KEY_LEFT_SHIFT: case GLFW_KEY_RIGHT_SHIFT: return "Shift";
        case GLFW_KEY_LEFT_CONTROL: case GLFW_KEY_RIGHT_CONTROL: return "Control";
        case GLFW_KEY_LEFT_ALT: case GLFW_KEY_RIGHT_ALT: return "Alt";
        case GLFW_KEY_LEFT_SUPER: case GLFW_KEY_RIGHT_SUPER: return "Meta";
        default: break;
    }
    return "";
}

void MorphWindow::KeyCb(GLFWwindow *win, int key, int scancode, int act, int mods)
{
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root)
        return;
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    MorphEvent e;
    e.type = (act == GLFW_PRESS || act == GLFW_REPEAT) ? EventType::KeyDown : EventType::KeyUp;
    e.key = keyEventName(key, scancode);
    e.code = keyCodeName(key);
    e.x = (float)mx;
    e.y = (float)my;
    e.mods = mods;
    // GLFW does not report a modifier as active on its own press event;
    // browsers do (Control keydown has ctrlKey=true), so set it here.
    // Releases read the live mask (the key is already up).
    if (act == GLFW_PRESS || act == GLFW_REPEAT) {
        if (key == GLFW_KEY_LEFT_SHIFT || key == GLFW_KEY_RIGHT_SHIFT) e.mods |= 0x01;
        else if (key == GLFW_KEY_LEFT_CONTROL || key == GLFW_KEY_RIGHT_CONTROL) e.mods |= 0x02;
        else if (key == GLFW_KEY_LEFT_ALT || key == GLFW_KEY_RIGHT_ALT) e.mods |= 0x04;
        else if (key == GLFW_KEY_LEFT_SUPER || key == GLFW_KEY_RIGHT_SUPER) e.mods |= 0x08;
    }
    e.repeat = (act == GLFW_REPEAT);
    e.buttons = s_buttonsDown;

    // ── Focus routing ───────────────────────────────────────
    // A focused node (e.g. an <input>) gets keys first; consuming the
    // event stops it from also reaching whatever is under the cursor.
    if ((act == GLFW_PRESS || act == GLFW_REPEAT) && MorphNode::s_focusedNode) {
        if (MorphNode::s_focusedNode->onKeyEvent(e))
            return;
    }
    self->m_root->dispatchEvent(e, (float)mx, (float)my);
}

void MorphWindow::CharCb(GLFWwindow *win, unsigned int codepoint)
{
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root)
        return;
    // Text input goes to the focused node only (browser semantics).
    if (MorphNode::s_focusedNode)
        MorphNode::s_focusedNode->onTextChar(codepoint);
}

void MorphWindow::clearHoverState() { MorphNode::s_lastHoveredNode = nullptr; }

void MorphWindow::clearActiveState() { MorphNode::s_activePressNode = nullptr; }

void MorphWindow::cursorPosCb(GLFWwindow *win, double mx, double my)
{
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root)
        return;

    auto *newHover = self->m_root->hitTest((float)mx, (float)my);
    auto*& hovered = MorphNode::s_lastHoveredNode;
    if (newHover != hovered)
    {
        _fireHoverDelta(hovered, newHover);
        if (hovered)
        {
            if (hovered->onMouseLeave)
            {
                MorphEvent he;
                he.type = EventType::MouseLeave;
                he.x = (float)mx;
                he.y = (float)my;
                he.buttons = s_buttonsDown;
                JsObject evt = hovered->buildEventJs(he);
                if (newHover) evt.set("relatedTarget", newHover->targetJs());
                hovered->onMouseLeave(evt);
            }
        }
        if (newHover)
        {
            if (newHover->onMouseEnter)
            {
                MorphEvent he;
                he.type = EventType::MouseEnter;
                he.x = (float)mx;
                he.y = (float)my;
                he.buttons = s_buttonsDown;
                JsObject evt = newHover->buildEventJs(he);
                if (hovered) evt.set("relatedTarget", hovered->targetJs());
                newHover->onMouseEnter(evt);
            }
        }
        hovered = newHover;
    }

    MorphEvent e;
    e.type = EventType::MouseMove;
    e.x = (float)mx;
    e.y = (float)my;
    e.buttons = s_buttonsDown;

    // Mouse-drag capture: a node that started a drag (e.g. <input> drag
    // selection, scrollbar thumb) keeps receiving moves even when the
    // cursor leaves its box.
    if (MorphNode::s_mouseCapture)
    {
        e.type = EventType::MouseMove;
        MorphNode* cap = MorphNode::s_mouseCapture;
        cap->onEvent(e);
#ifdef MORPH_FEATURE_SCROLL
        if (cap->scrollDragging)
        {
            cap->scrollDragTo((float)my);
        }
#endif
    }

    self->m_root->dispatchEvent(e, (float)mx, (float)my);
    e.type = EventType::PointerMove;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);

#ifdef MORPH_FEATURE_CURSOR
    auto *target = newHover;
    const CSS::Cursor *cur = nullptr;
    for (auto *n = target; n; n = n->parent)
    {
        // Exact parity: garbage values stop the walk like any non-default
        // (they resolve to nullptr below), matching today's string check.
        if (std::string_view(CSS::toString(n->style.cursor)) != "default")
        {
            cur = &n->style.cursor;
            break;
        }
    }
    if (cur && *cur == CSS::Cursor::Pointer)
        glfwSetCursor(win, self->m_handCursor);
    else if (cur && *cur == CSS::Cursor::Text)
        glfwSetCursor(win, self->m_textCursor);
    else
        glfwSetCursor(win, nullptr);
#endif
}

void MorphWindow::windowSizeCb(GLFWwindow *win, int width, int height)
{
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self)
        return;
    self->m_width = width;
    self->m_height = height;
    self->m_pendingRender = true;
    if (self->m_root)
    {
        self->m_root->markDirty(SubtreeDirty);
    }
}

// Modal follow: a moved window re-locks its followers (and a moved
// follower re-locks onto its parent) via the registry. Corrective
// moves re-fire this callback and are skipped there by guard.
void MorphWindow::windowPosCb(GLFWwindow *win, int x, int y)
{
    (void)x;
    (void)y;
    WindowManager::get().noteMoved(win);
}

void MorphWindow::scrollCb(GLFWwindow *win, double dx, double dy)
{
    (void)dx;
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root)
        return;
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    MorphEvent e;
    e.type = EventType::Scroll;
    e.scroll = (float)dy;
    e.x = (float)mx;
    e.y = (float)my;
    e.buttons = s_buttonsDown;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);
}

// Browser-style: losing window focus ends any in-progress mouse drag and
// drops keyboard focus from <input> fields (selection highlight clears too).
void MorphWindow::windowFocusCb(GLFWwindow *win, int focused)
{
    if (focused == GLFW_TRUE)
    {
        WindowManager::get().noteFocus(win);
        return;
    }
    // Losing focus ends any in-progress drag: Alt-Tab mid-thumb-drag
    // would otherwise lose the release and stick the scrollbar.
    if (MorphNode::s_mouseCapture)
    {
#ifdef MORPH_FEATURE_SCROLL
        MorphNode::s_mouseCapture->scrollDragging = false;
#endif
        MorphNode::s_mouseCapture = nullptr;
    }
    ungrabPointerForDrag();
#ifdef MORPH_FEATURE_INPUT
    if (MorphNode::s_focusedNode)
        MorphNode::s_focusedNode->blur();
#endif
}

MorphWindow::MorphWindow(const std::string &title, int width, int height, bool visible)
    : m_title(title), m_width(width), m_height(height), m_visible(visible)
{
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    // Created-but-hidden windows (WindowManager::open shows them later):
    // the hint must be set before glfwCreateWindow — hiding after the
    // fact flashes a visible frame.
    glfwWindowHint(GLFW_VISIBLE, visible ? GLFW_TRUE : GLFW_FALSE);
    m_handle = glfwCreateWindow(width, height, title.c_str(), nullptr, nullptr);
    glfwWindowHint(GLFW_VISIBLE, GLFW_TRUE);
    // Creating a window must not disturb the calling thread: a dynamic
    // window can be born inside an effect handler or event dispatch while
    // another window's context is current (its atlas uploads would land
    // in the wrong context and paint black). Save and restore.
    GLFWwindow* prevContext = glfwGetCurrentContext();
    if (m_handle)
    {
        glfwMakeContextCurrent(m_handle);
        gladLoadGLLoader(reinterpret_cast<GLADloadproc>(glfwGetProcAddress));
        // Cap presents at the monitor refresh rate. Dirty rendering already
        // skips the swap when nothing changed; vsync keeps active animation
        // from spinning at hundreds of FPS.
        glfwSwapInterval(1);
        glfwSetWindowUserPointer(m_handle, this);
        // Input callbacks register only for features that need them —
        // unregistered handlers GC-drop with the callbacks that reference
        // them (TEXT_INPUT for fields, CLICK for any onClick/link, HOVER
        // for hover styles/cursor shapes, SCROLL for scrollables).
        glfwSetMouseButtonCallback(m_handle, mouseButtonCb);
#ifdef MORPH_FEATURE_INPUT
        glfwSetKeyCallback(m_handle, KeyCb);
        glfwSetCharCallback(m_handle, CharCb);
#endif
#ifdef MORPH_FEATURE_HOVER
        glfwSetCursorPosCallback(m_handle, cursorPosCb);
#endif
#ifdef MORPH_FEATURE_SCROLL
        glfwSetScrollCallback(m_handle, scrollCb);
#endif
        glfwSetWindowSizeCallback(m_handle, windowSizeCb);
        // Modal follow needs position events; unowned apps never
        // register the callback, so the follow chain GC-drops.
#ifdef MORPH_FEATURE_OWNERSHIP
        glfwSetWindowPosCallback(m_handle, windowPosCb);
#endif
        glfwSetWindowFocusCallback(m_handle, windowFocusCb);
        s_clipboardWindow = m_handle;
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
#ifdef MORPH_FEATURE_CURSOR
        m_handCursor = glfwCreateStandardCursor(GLFW_HAND_CURSOR);
        m_textCursor = glfwCreateStandardCursor(GLFW_IBEAM_CURSOR);
#endif
        // Keep context current on main thread; compositor does CPU-only work
    }
    if (prevContext != m_handle)
        glfwMakeContextCurrent(prevContext);
}

void MorphWindow::setTitle(const std::string &title)
{
    m_title = title;
    if (m_handle)
        glfwSetWindowTitle(m_handle, title.c_str());
}

void MorphWindow::show()
{
    m_visible = true;
    m_pendingRender = true;
    if (m_handle)
        glfwShowWindow(m_handle);
}

void MorphWindow::hide()
{
    m_visible = false;
    if (m_handle)
        glfwHideWindow(m_handle);
}

// Dirty-tree helpers are declared in window.h and shared with the flash renderer.

void MorphWindow::setSize(int width, int height)
{
    m_width = width;
    m_height = height;
    if (m_handle)
        glfwSetWindowSize(m_handle, width, height);
}

void MorphWindow::setPosition(int x, int y)
{
    if (m_handle)
        glfwSetWindowPos(m_handle, x, y);
}

void MorphWindow::position(int& x, int& y) const
{
    x = 0;
    y = 0;
    if (m_handle)
        glfwGetWindowPos(m_handle, &x, &y);
}

void MorphWindow::setFloating(bool floating)
{
    if (m_handle)
        glfwSetWindowAttrib(m_handle, GLFW_FLOATING, floating ? GLFW_TRUE : GLFW_FALSE);
}

void MorphWindow::setConstraints(int minWidth, int minHeight, int maxWidth, int maxHeight)
{
    if (m_handle)
        glfwSetWindowSizeLimits(m_handle, minWidth, minHeight, maxWidth, maxHeight);
}

MorphWindow::~MorphWindow()
{
    stopCompositor();
    // The clipboard window is a non-owning handle: creating a popup
    // repoints it, so a close must not leave it dangling at the dead
    // window (clipboard degrades to no-op until a live window exists).
    if (s_clipboardWindow == m_handle)
        s_clipboardWindow = nullptr;
    if (m_handle)
    {
        // The renderer's GL names live in this window's context only —
        // release them here, while it is current. Deleting them later
        // (member destruction) or on a foreign context would free the
        // surviving windows' same-numbered objects.
        glfwMakeContextCurrent(m_handle);
        m_renderer.shutdown();
    }
    delete m_root;
    m_root = nullptr;
#ifdef MORPH_FEATURE_CURSOR
    if (m_handCursor)
        glfwDestroyCursor(m_handCursor);
    if (m_textCursor)
        glfwDestroyCursor(m_textCursor);
#endif
    if (m_handle)
        glfwDestroyWindow(m_handle);
    // Never leave a deleted context current — pump rebinds per window.
    // (Headless windows skip everything above: null handle, no GL.)
    if (m_handle)
    {
        m_handle = nullptr;
        glfwMakeContextCurrent(nullptr);
    }
}

void MorphWindow::startCompositor(bool vsync)
{
    if (m_compositor)
        return;
    m_vsync = vsync;
    m_compositor = new Compositor(m_handle, m_width, m_height, &m_frameChannel);
    m_compositor->setVSync(vsync);
    m_compositor->start();
}

void MorphWindow::stopCompositor()
{
    if (m_compositor)
    {
        m_compositor->stop();
        delete m_compositor;
        m_compositor = nullptr;
    }
}

void MorphWindow::commitFrame()
{
    if (!m_root)
        return;
#ifdef MORPH_RENDERER_FORGE
    forge::forgeCommit(*this);
#else
    flash::flashCommit(*this);
#endif
}

void MorphWindow::drawOpsForNode(GLRenderer &r, const RenderFrame *frame, int nodeIdx,
                                 float ox, float oy)
{
    const auto &node = frame->nodes[nodeIdx];
    for (int i = node.dlOffset; i < node.dlOffset + node.dlCount; i++)
    {
        const auto &op = frame->drawOps[i];
        float px = op.x + ox;
        float py = op.y + oy;
        switch (op.type)
        {
        case DrawOp::Rect:
            r.drawRect(px, py, op.w, op.h, (float *)&op.r);
            break;
        case DrawOp::RoundedRect:
            r.drawRoundedRect(px, py, op.w, op.h, op.data[0], (float *)&op.r);
            break;
        case DrawOp::BorderedRect:
            r.drawBorderedRect(px, py, op.w, op.h, (float *)&op.r, op.data[1], (float *)&op.br);
            break;
        case DrawOp::BorderedRoundedRect:
            r.drawBorderedRoundedRect(px, py, op.w, op.h, op.data[0], (float *)&op.r,
                                      op.data[1], (float *)&op.br);
            break;
        case DrawOp::BorderRing:
            r.drawBorderRing(px, py, op.w, op.h, op.data[0], op.data[1], (float *)&op.br);
            break;
        case DrawOp::BeginClip:
            r.beginClip(px, py, op.w, op.h);
            break;
        case DrawOp::EndClip:
            r.endClip();
            break;
        case DrawOp::BeginRoundedClip:
            r.beginRoundedClip(px, py, op.w, op.h, op.data[0]);
            break;
        case DrawOp::EndRoundedClip:
            r.endRoundedClip();
            break;
        case DrawOp::PushScroll:
            r.pushScrollOffset(0, op.r);
            break;
        case DrawOp::PopScroll:
            r.popScrollOffset(0, op.r);
            break;
        case DrawOp::Scrollbar:
            break; // handled in renderNode
        case DrawOp::TextureQuad:
            r.drawTexture(op.texId, px, py, op.w, op.h, &op.r);
            break;
        case DrawOp::TextureBordered:
            r.drawTexture(op.texId, px, py, op.w, op.h, &op.r);
            break;
        }
    }
}

void MorphWindow::drawScrollbar(GLRenderer &r, const FlatRenderNode &node,
                                float sx, float sy, float sw, float sh)
{
    float sbw = node.scrollbarWidth;
    float trackX = sx + sw - sbw;
    r.drawRect(trackX, sy, sbw, sh, (float *)node.scrollbarTrackColor);
    float thumbH = (sh / node.contentH) * sh;
    float thumbY = sy + (node.scrollY / (node.contentH - sh)) * (sh - thumbH);
    if (thumbY < sy)
        thumbY = sy;
    if (thumbY + thumbH > sy + sh)
        thumbY = sy + sh - thumbH;
    float radius = node.scrollbarBorderRadius;
    if (radius > thumbH * 0.5f)
        radius = thumbH * 0.5f;
    if (radius < 0.5f)
        radius = 0.5f;
    r.drawRoundedRect(trackX, thumbY, sbw, thumbH, radius, (float *)node.scrollbarThumbColor);
}

void MorphWindow::renderNode(const RenderFrame *frame, int nodeIdx,
                             const DamageSet *damageClip, float scrollOffset)
{
    const auto &node = frame->nodes[nodeIdx];

    auto sc = [&](float v)
    { return node.hasLayoutTransition ? v : std::round(v); };
    float sx = sc(node.x + node.animOffsetX);
    float sy = sc(node.y + node.animOffsetY);
    float sw = sc(node.w);
    float sh = sc(node.h);

    bool overflowClipped = (node.overflow != CSS::Overflow::Visible);
    bool radiusClip = node.borderRadius > 0.0f;
    bool scrolling = node.scrollEnabled && node.contentH > sh;

    // Effective screen position: node coords are absolute root-space but do
    // NOT include scroll — a scrolling ancestor shifts the whole subtree via
    // pushScrollOffset(0, -scrollY), accumulated in scrollOffset.
    float screenY = sy - scrollOffset;

#ifdef MORPH_FEATURE_TRANSFORM
    // Transform model path: once any ancestor (or this node) has a transform,
    // the whole subtree renders through the accumulated model matrix. The
    // flatten pass computed a screen-space AABB (cullX/Y/W/H) that already
    // accounts for every ancestor transform and scroll, so cull against that
    // instead of the untransformed box.
    bool underTransform = m_renderer.transformStackActive();
    bool transformed = underTransform || node.transformSet;
#else
    bool transformed = false;
#endif

    // Viewport cull (flash + forge): skip anything fully outside the scene —
    // its pixels can't be seen and the GPU never needs to touch them.
    // sx/sy already include the interpolated animOffset, so nodes animating
    // into view are never culled.
    float cw = contentWidth();
    float ch = contentHeight();
#ifdef MORPH_FEATURE_TRANSFORM
    if (transformed)
    {
        // Skip the cull entirely while a layout transition is interpolating:
        // the AABB was computed from the non-interpolated layout position.
        if (!node.hasLayoutTransition &&
            (node.cullX + node.cullW <= 0.0f || node.cullX >= cw ||
             node.cullY + node.cullH <= 0.0f || node.cullY >= ch))
        {
            if (overflowClipped || radiusClip)
                return;
            for (int childIdx : node.children)
                renderNode(frame, childIdx, damageClip, scrollOffset);
            return;
        }
    }
    else
#endif
    if (sx + sw <= 0.0f || sx >= cw || screenY + sh <= 0.0f || screenY >= ch)
    {
        // Clipping nodes fully contain their descendants, so skipping the
        // whole subtree is safe. Unclipped nodes can have overflowed
        // children that DO reach the view — recurse them without touching
        // this node's pixels.
        if (overflowClipped || radiusClip)
            return;
        for (int childIdx : node.children)
            renderNode(frame, childIdx, damageClip, scrollOffset);
        return;
    }

    // Damage-limited re-raster: skip anything whose own box can't touch the
    // repaint region — its pixels are already correct in the retained surface.
    if (damageClip)
    {
        DamageRect box{(int)sx, (int)sy, (int)sw, (int)sh};
#ifdef MORPH_FEATURE_TRANSFORM
        if (transformed)
        {
            if (!node.hasLayoutTransition)
                box = DamageRect{(int)node.cullX, (int)node.cullY,
                                 (int)node.cullW, (int)node.cullH};
        }
#endif
        if (!damageClip->intersects(box))
        {
            // Clipping nodes fully contain their descendants, so skipping the
            // whole subtree is safe. Unclipped nodes can have overflowed
            // children that DO reach the damage — recurse them without
            // touching this node's pixels.
            if (overflowClipped || radiusClip)
                return;
            for (int childIdx : node.children)
                renderNode(frame, childIdx, damageClip, scrollOffset);
            return;
        }
    }

#ifdef MORPH_FEATURE_TRANSFORM
    // Model-path push: the stack telescopes accumulated absolute transforms,
    // so rel (this node's position delta from its parent) × own matrix.
    // The anchor is this node's (interpolated) layout position, which every
    // draw call on the model path cancels against.
    bool pushedTransform = false;
    if (transformed)
    {
        float parentX = 0.0f, parentY = 0.0f;
        if (node.parentId >= 0)
        {
            const auto &p = frame->nodes[node.parentId];
            parentX = sc(p.x + p.animOffsetX);
            parentY = sc(p.y + p.animOffsetY);
        }
        float rel[16], m[16], tmp[16];
        morph::mat4Identity(rel);
        // The anchor cancellation maps baked absolute instance coords through
        // m_model × T(-anchor).  m_model telescopes the absolute transform of
        // every transformed ancestor, so a node entering the transform path
        // (stack inactive) must bake its ABSOLUTE position; only nodes
        // already under the stack bake the parent-relative delta.
        rel[12] = underTransform ? (sx - parentX) : sx;
        rel[13] = underTransform ? (sy - parentY) : sy;
        if (node.transformSet)
        {
            // T(rel) × T(o) × M × T(-o) — the transform-origin is baked in so
            // the box rotates about its own origin point (default center);
            // the anchor stays the node's position, which every instance's
            // model cancels against.
            float ox = node.originX * node.w, oy = node.originY * node.h;
            float t0[16], t1[16];
            morph::mat4Identity(t0);
            t0[12] = ox; t0[13] = oy;
            morph::mat4Identity(t1);
            t1[12] = -ox; t1[13] = -oy;
            morph::mat4Multiply(m, rel, t0);          // T(rel)×T(o)
            morph::mat4Multiply(t0, m, node.matrix);  // ×M
            morph::mat4Multiply(tmp, t0, t1);         // ×T(-o)
        }
        else
        {
            morph::mat4Identity(m);
            morph::mat4Multiply(tmp, rel, m);
        }
        m_renderer.pushTransform(tmp, sx, sy);
        pushedTransform = true;
    }
#endif

    // 1. Draw self (background from display list)
    drawOpsForNode(m_renderer, frame, nodeIdx, node.animOffsetX, node.animOffsetY);

    // 1b. Input edit overlay: selection highlight + caret, then the field's
    // text — all clipped to the content box so nothing paints outside the
    // field (browser behavior). Text ops ride the same clip.
    bool inputClip = node.clipText;
    if (inputClip)
        m_renderer.beginClip(sx + node.textClipPadX, sy,
                             sw - 2.0f * node.textClipPadX, sh);
    if (!std::isnan(node.selX0) && node.selX1 != node.selX0)
    {
        float left = std::min(node.selX0, node.selX1);
        float right = std::max(node.selX0, node.selX1);
        float sc[4] = {node.selColor[0], node.selColor[1],
                       node.selColor[2], node.selColor[3]};
        m_renderer.drawRect(left + node.animOffsetX,
                            node.caretY + node.animOffsetY,
                            right - left, node.caretH, sc);
    }
    if (!std::isnan(node.caretX))
    {
        float cc[4] = {node.caretColor[0], node.caretColor[1],
                       node.caretColor[2], node.caretColor[3]};
        m_renderer.drawRect(node.caretX + node.animOffsetX,
                            node.caretY + node.animOffsetY,
                            2.0f, node.caretH, cc);
    }

    // Text rendering
    for (int i = node.textOpOffset; i < node.textOpOffset + node.textOpCount; i++)
    {
        if (i >= (int)frame->textOps.size())
            break;
        const auto &to = frame->textOps[i];
        float tx = to.x + node.animOffsetX;
        float ty = to.y + node.animOffsetY;
        // Renderer alignment has no Justify (behaves as left, per CSS docs).
        TextAlign align = TextAlign::Left;
        if (to.align == CSS::TextAlign::Center)
            align = TextAlign::Center;
        else if (to.align == CSS::TextAlign::Right)
            align = TextAlign::Right;
        m_renderer.drawText(to.text, tx, ty, const_cast<float *>(to.color),
                            align, to.fontSize,
                            to.fontWeight,
                            to.centerInk != 0);
    }
    if (inputClip)
        m_renderer.endClip();

    // 2. Clip setup
    if (overflowClipped || radiusClip)
    {
        // Under a transform the scissor rect would be axis-aligned in root
        // space while the content is transformed, so fall back to the stencil
        // clip (draws the node's box through the model path, masking the
        // transformed shape exactly).
        if (overflowClipped)
        {
            if (transformed)
                m_renderer.beginRoundedClip(sx, sy, sw, sh, 0.0f);
            else
                m_renderer.beginClip(sx, sy, sw, sh);
        }
        if (radiusClip)
            m_renderer.beginRoundedClip(sx, sy, sw, sh, node.borderRadius);
    }

    // 3. Scroll push + children
    if (scrolling)
        m_renderer.pushScrollOffset(0, -node.scrollY);
    float childScroll = scrollOffset + (scrolling ? node.scrollY : 0.0f);
    for (int childIdx : node.children)
    {
        if (scrolling && !transformed)
        {
            const auto &child = frame->nodes[childIdx];
            float childVisY = child.y + child.animOffsetY - node.scrollY;
            if (childVisY + child.h > sy && childVisY < sy + sh)
            {
                renderNode(frame, childIdx, damageClip, childScroll);
            }
        }
        else
        {
            renderNode(frame, childIdx, damageClip, childScroll);
        }
    }
    if (scrolling)
        m_renderer.popScrollOffset(0, -node.scrollY);

    // 4. Clip teardown
    if (overflowClipped || radiusClip)
    {
        if (radiusClip)
            m_renderer.endRoundedClip();
        if (overflowClipped)
            m_renderer.endClip();
    }

    // 5. Scrollbar
    if (scrolling)
    {
        drawScrollbar(m_renderer, node, sx, sy, sw, sh);
    }

#ifdef MORPH_FEATURE_TRANSFORM
    if (pushedTransform)
        m_renderer.popTransform();
#endif
}

void MorphWindow::renderFrame(std::function<void(GLRenderer &, DirtyStats &)> overlayFn)
{
    if (!m_handle)
        return;

    if (!m_root)
    {
        // Rootless window (created but no route mounted yet): present a
        // blank frame. Spinning on the compositor handshake instead would
        // hang forever — no commit can complete without a tree — and a
        // stuck main thread starves every window.
        glViewport(0, 0, m_width, m_height);
        m_renderer.setFBHeight(m_height);
        float proj[16];
        ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);
        m_renderer.setClearColor(1.0f, 1.0f, 1.0f, 1.0f);
        m_renderer.clear();
        m_renderer.setProjection(proj);
        m_renderer.flush(proj);
        glfwSwapBuffers(m_handle);
        clearPendingRender();
        return;
    }

    if (activeRenderMode() == RenderMode::Forge)
    {
#ifdef MORPH_RENDERER_FORGE
        forge::forgePresent(*this, overlayFn);
        return;
#endif
    }

    // Wait for compositor to finish interpolation
    // (typically already done by the time we get here, but spin if not)
    while (!m_frameChannel.frameInterpolated.load(std::memory_order_acquire))
    {
        std::this_thread::yield();
    }
    m_frameChannel.frameInterpolated.store(false, std::memory_order_release);

    auto *frame = m_frameChannel.frontFrame.load(std::memory_order_acquire);
    if (!frame)
        return;

    glViewport(0, 0, m_width, m_height);
    m_renderer.setFBHeight(m_height);

    float proj[16];
    ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);

    float clear[4];
    bodyClearColor(clear);
    m_renderer.setClearColor(clear[0], clear[1], clear[2], clear[3]);
    m_renderer.clear();
    m_renderer.setProjection(proj);

    // Render the flat node tree (compositor has already interpolated animations)
    for (size_t i = 0; i < frame->nodes.size(); i++)
    {
        if (frame->nodes[i].parentId == -1)
        {
            renderNode(frame, (int)i);
        }
    }

    m_renderer.flush(proj);
    if (overlayFn)
    {
        overlayFn(m_renderer, m_dirtyStats);
        m_renderer.flush(proj);
    }
    glfwSwapBuffers(m_handle);
}

void MorphWindow::drawFrameNodes(const DamageSet *damageClip)
{
    auto *frame = m_frameChannel.frontFrame.load(std::memory_order_acquire);
    if (!frame)
        return;

    glViewport(0, 0, m_width, m_height);
    m_renderer.setFBHeight(m_height);

    float proj[16];
    ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);
    m_renderer.setProjection(proj);

    for (size_t i = 0; i < frame->nodes.size(); i++)
    {
        if (frame->nodes[i].parentId == -1)
            renderNode(frame, (int)i, damageClip);
    }
    m_renderer.flush(proj);
}

// ── Legacy single-threaded render path ──
void MorphWindow::render(std::function<void(GLRenderer &, DirtyStats &)> overlayFn)
{
    if (!m_handle)
        return;
    glfwMakeContextCurrent(m_handle);
    glViewport(0, 0, m_width, m_height);
    m_renderer.setFBHeight(m_height);
    float proj[16];
    ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);

    m_dirtyStats.reset();

    if (m_root)
    {
        {
            float clear[4];
            bodyClearColor(clear);
            m_renderer.setClearColor(clear[0], clear[1], clear[2], clear[3]);
        }

#ifdef MORPH_FEATURE_DIRTY_RENDERING
        m_renderer.ensureReady();
        m_root->layoutIfNeeded(0.0f, 0.0f, contentWidth(), contentHeight(),
                               &m_renderer, &m_dirtyStats);
        m_dirtyStats.fullTreeCount = countNodes(m_root);
#ifdef MORPH_FEATURE_DEV
        syncPaintDirtyTree(m_root);
#endif
        recordPaintTree(m_root, m_renderer, m_dirtyStats);
        m_renderer.clear();
        m_renderer.setProjection(proj);
        m_root->executeDisplayList(m_renderer);
#else
        m_renderer.ensureReady();
        m_root->layout(0.0f, 0.0f, contentWidth(), contentHeight(), &m_renderer);
        m_renderer.clear();
        m_renderer.setProjection(proj);
        m_root->draw(m_renderer);
        // Full relayout + redraw already happened, so nothing stays dirty.
        clearAllDirty(m_root);
#endif
    }

    m_renderer.flush(proj);
    if (overlayFn)
    {
        overlayFn(m_renderer, m_dirtyStats);
        m_renderer.flush(proj);
    }
    glfwSwapBuffers(m_handle);

#ifdef MORPH_FEATURE_DIRTY_RENDERING
    m_prevHadDirty = !m_root || !m_root->isFullyClean();
#endif
    m_pendingRender = false;
}

void MorphWindow::bodyClearColor(float out[4]) const
{
    // The clear color follows the app body's background (black body =
    // black clear). m_root is the window container, so the body is its
    // first child. Prefer the committed frame (matches what the
    // compositor / forge paths draw), fall back to the live tree, then
    // opaque white.
    auto *frame = m_frameChannel.frontFrame.load(std::memory_order_acquire);
    if (frame)
    {
        int rootIdx = -1;
        for (size_t i = 0; i < frame->nodes.size(); i++)
        {
            if (frame->nodes[i].parentId == -1)
            {
                rootIdx = (int)i;
                break;
            }
        }
        for (const auto &n : frame->nodes)
        {
            bool isBody = (rootIdx >= 0) ? (n.parentId == rootIdx) : (n.parentId == -1);
            if (isBody && n.bgColor[3] > 0.0f)
            {
                out[0] = n.bgColor[0]; out[1] = n.bgColor[1];
                out[2] = n.bgColor[2]; out[3] = n.bgColor[3];
                return;
            }
        }
    }
    if (m_root)
    {
        const MorphNode* body = m_root->children.empty() ? m_root : m_root->children[0];
        auto &bg = body->style.bgColor;
        if (bg[3] > 0.0f)
        {
            out[0] = bg[0]; out[1] = bg[1]; out[2] = bg[2]; out[3] = bg[3];
            return;
        }
        auto &wbg = m_root->style.bgColor;
        if (wbg[3] > 0.0f)
        {
            out[0] = wbg[0]; out[1] = wbg[1]; out[2] = wbg[2]; out[3] = wbg[3];
            return;
        }
    }
    out[0] = 1.0f; out[1] = 1.0f; out[2] = 1.0f; out[3] = 1.0f;
}

int countNodes(MorphNode *n)
{
    int c = 1;
    for (auto *child : n->children)
        c += countNodes(child);
    return c;
}

static void clearAllDirty(MorphNode *n)
{
    n->clearDirty(StyleDirty);
    n->clearDirty(LayoutDirty);
    n->clearDirty(PaintDirty);
    n->clearDirty(ScrollDirty);
    n->clearDirty(SubtreeDirty);
    for (auto *c : n->children)
        clearAllDirty(c);
}

#ifdef MORPH_FEATURE_DEV
void syncPaintDirtyTree(MorphNode *n)
{
    n->syncPaintDirtyAfterLayout();
    for (auto *c : n->children)
        syncPaintDirtyTree(c);
}
#endif

void recordPaintTree(MorphNode *n, Renderer &r, DirtyStats &stats)
{
    if (n->isDirty(PaintDirty) || n->isDirty(ScrollDirty) || n->isDirty(StyleDirty))
    {
        stats.paintCount++;
        n->recordDisplayList(r);
        n->clearDirty(PaintDirty);
        n->clearDirty(ScrollDirty);
        if (g_repaintHook)
            g_repaintHook(n);
    }
    for (auto *child : n->children)
        recordPaintTree(child, r, stats);
}
