#include "window.h"
#include "renderers/flash/flash.h"
#include "renderers/forge/forge.h"
#include <GLFW/glfw3.h>
#include <algorithm>
#include <print>

RepaintHookFn g_repaintHook = nullptr;

// Double-click detection: threshold in seconds
static double s_lastClickTime = 0.0;
static const double DBL_CLICK_THRESHOLD = 0.3;
// Node currently :active (pressed) — cleared on release / tree rebuild
static MorphNode* s_activeNode = nullptr;

void MorphWindow::mouseButtonCb(GLFWwindow *win, int btn, int act, int mods)
{
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
        self->m_root->dispatchEvent(e, (float)mx, (float)my);

        // :active pseudo-class — apply on press, release on button up
        if (act == GLFW_PRESS)
        {
            if (s_activeNode)
                s_activeNode->onActive(false);
            s_activeNode = self->m_root->hitTest((float)mx, (float)my);
            if (s_activeNode)
                s_activeNode->onActive(true);
        }
        else
        {
            if (s_activeNode)
                s_activeNode->onActive(false);
            s_activeNode = nullptr;
        }

        if (act == GLFW_PRESS)
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
    }
}

void MorphWindow::KeyCb(GLFWwindow *win, int key, int scancode, int act, int mods)
{
    (void)mods;
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root)
        return;
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    MorphEvent e;
    e.type = (act == GLFW_PRESS) ? EventType::KeyDown : EventType::KeyUp;
    const char *keyName = glfwGetKeyName(key, scancode);

    e.key = keyName ? std::string(keyName) : key == 259 ? "Backspace"
                                         : key == 32    ? "Space"
                                                        : "";
    e.x = (float)mx;
    e.y = (float)my;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);
}

void MorphWindow::clearHoverState() { MorphNode::s_lastHoveredNode = nullptr; }

void MorphWindow::clearActiveState() { s_activeNode = nullptr; }

void MorphWindow::cursorPosCb(GLFWwindow *win, double mx, double my)
{
    auto *self = (MorphWindow *)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root)
        return;

    auto *newHover = self->m_root->hitTest((float)mx, (float)my);
    auto*& hovered = MorphNode::s_lastHoveredNode;
    if (newHover != hovered)
    {
        if (hovered)
        {
            if (hovered->onMouseLeave)
            {
                JsObject evt;
                evt.set("x", JsNumber((float)mx));
                evt.set("y", JsNumber((float)my));
                evt.set("type", JsString("mouseleave"));
                hovered->onMouseLeave(evt);
            }
            hovered->onHover(false);
        }
        if (newHover)
        {
            if (newHover->onMouseEnter)
            {
                JsObject evt;
                evt.set("x", JsNumber((float)mx));
                evt.set("y", JsNumber((float)my));
                evt.set("type", JsString("mouseenter"));
                newHover->onMouseEnter(evt);
            }
            newHover->onHover(true);
        }
        hovered = newHover;
    }

    MorphEvent e;
    e.type = EventType::MouseMove;
    e.x = (float)mx;
    e.y = (float)my;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);

#ifdef MORPH_FEATURE_CURSOR
    auto *target = newHover;
    const std::string *cur = nullptr;
    for (auto *n = target; n; n = n->parent)
    {
        if (n->style.cursor != "default")
        {
            cur = &n->style.cursor;
            break;
        }
    }
    if (cur && *cur == "pointer")
        glfwSetCursor(win, self->m_handCursor);
    else if (cur && *cur == "text")
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
    self->m_root->dispatchEvent(e, (float)mx, (float)my);
}

MorphWindow::MorphWindow(const std::string &title, int width, int height, bool visible)
    : m_title(title), m_width(width), m_height(height), m_visible(visible)
{
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    m_handle = glfwCreateWindow(width, height, title.c_str(), nullptr, nullptr);
    if (m_handle)
    {
        glfwMakeContextCurrent(m_handle);
        gladLoadGLLoader(reinterpret_cast<GLADloadproc>(glfwGetProcAddress));
        // Cap presents at the monitor refresh rate. Dirty rendering already
        // skips the swap when nothing changed; vsync keeps active animation
        // from spinning at hundreds of FPS.
        glfwSwapInterval(1);
        glfwSetWindowUserPointer(m_handle, this);
        glfwSetMouseButtonCallback(m_handle, mouseButtonCb);
        glfwSetKeyCallback(m_handle, KeyCb);
        glfwSetCursorPosCallback(m_handle, cursorPosCb);
        glfwSetScrollCallback(m_handle, scrollCb);
        glfwSetWindowSizeCallback(m_handle, windowSizeCb);
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
#ifdef MORPH_FEATURE_CURSOR
        m_handCursor = glfwCreateStandardCursor(GLFW_HAND_CURSOR);
        m_textCursor = glfwCreateStandardCursor(GLFW_IBEAM_CURSOR);
#endif
        // Keep context current on main thread; compositor does CPU-only work
    }
}

void MorphWindow::setTitle(const std::string &title)
{
    m_title = title;
    if (m_handle)
        glfwSetWindowTitle(m_handle, title.c_str());
}

// Dirty-tree helpers are declared in window.h and shared with the flash renderer.

void MorphWindow::setSize(int width, int height)
{
    m_width = width;
    m_height = height;
    if (m_handle)
        glfwSetWindowSize(m_handle, width, height);
}

MorphWindow::~MorphWindow()
{
    stopCompositor();
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
}

void MorphWindow::startCompositor(bool vsync)
{
    if (m_compositor)
        return;
    m_vsync = vsync;
    m_compositor = new Compositor(m_handle, m_width, m_height);
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

    if (activeRenderMode() == RenderMode::Forge)
        forge::forgeCommit(*this);
    else
        flash::flashCommit(*this);
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
            r.drawTexture(op.texId, px, py, op.w, op.h);
            break;
        case DrawOp::TextureBordered:
            r.drawTexture(op.texId, px, py, op.w, op.h);
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

void MorphWindow::renderNode(const RenderFrame *frame, int nodeIdx)
{
    const auto &node = frame->nodes[nodeIdx];

    auto sc = [&](float v)
    { return node.hasLayoutTransition ? v : std::round(v); };
    float sx = sc(node.x + node.animOffsetX);
    float sy = sc(node.y + node.animOffsetY);
    float sw = sc(node.w);
    float sh = sc(node.h);

    bool overflowClipped = (node.overflow == 1 || node.overflow == 2 || node.overflow == 3);
    bool radiusClip = node.borderRadius > 0.0f;
    bool scrolling = node.scrollEnabled && node.contentH > sh;

    // 1. Draw self (background from display list)
    drawOpsForNode(m_renderer, frame, nodeIdx, node.animOffsetX, node.animOffsetY);

    // Text rendering
    for (int i = node.textOpOffset; i < node.textOpOffset + node.textOpCount; i++)
    {
        if (i >= (int)frame->textOps.size())
            break;
        const auto &to = frame->textOps[i];
        float tx = to.x + node.animOffsetX;
        float ty = to.y + node.animOffsetY;
        m_renderer.drawText(to.text, tx, ty, const_cast<float *>(to.color),
                            (TextAlign)to.align, to.fontSize,
                            to.fontWeight ? "bold" : "normal");
    }

    // 2. Clip setup
    if (overflowClipped || radiusClip)
    {
        if (overflowClipped)
            m_renderer.beginClip(sx, sy, sw, sh);
        if (radiusClip)
            m_renderer.beginRoundedClip(sx, sy, sw, sh, node.borderRadius);
    }

    // 3. Scroll push + children
    if (scrolling)
        m_renderer.pushScrollOffset(0, -node.scrollY);
    for (int childIdx : node.children)
    {
        if (scrolling)
        {
            const auto &child = frame->nodes[childIdx];
            float childVisY = child.y + child.animOffsetY - node.scrollY;
            if (childVisY + child.h > sy && childVisY < sy + sh)
            {
                renderNode(frame, childIdx);
            }
        }
        else
        {
            renderNode(frame, childIdx);
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
}

void MorphWindow::renderFrame(std::function<void(GLRenderer &, DirtyStats &)> overlayFn)
{
    if (!m_handle)
        return;

    if (activeRenderMode() == RenderMode::Forge)
    {
        forge::forgePresent(*this, overlayFn);
        return;
    }

    // Wait for compositor to finish interpolation
    // (typically already done by the time we get here, but spin if not)
    while (!g_frameInterpolated.load(std::memory_order_acquire))
    {
        std::this_thread::yield();
    }
    g_frameInterpolated.store(false, std::memory_order_release);

    auto *frame = g_frontFrame.load(std::memory_order_acquire);
    if (!frame)
        return;

    glViewport(0, 0, m_width, m_height);
    m_renderer.setFBHeight(m_height);

    float proj[16];
    ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);

    m_renderer.setClearColor(1.0f, 1.0f, 1.0f, 1.0f);
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

void MorphWindow::drawFrameNodes()
{
    auto *frame = g_frontFrame.load(std::memory_order_acquire);
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
            renderNode(frame, (int)i);
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
            auto &bg = m_root->style.bgColor;
            if (bg[3] > 0.0f)
                m_renderer.setClearColor(bg[0], bg[1], bg[2], bg[3]);
            else
                m_renderer.setClearColor(1.0f, 1.0f, 1.0f, 1.0f);
        }

#ifdef MORPH_FEATURE_DIRTY_RENDERING
        m_renderer.ensureReady();
        m_root->layoutIfNeeded(0.0f, 0.0f, (float)m_width, (float)m_height,
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
        m_root->layout(0.0f, 0.0f, (float)m_width, (float)m_height, &m_renderer);
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
