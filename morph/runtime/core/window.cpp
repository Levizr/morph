#include "window.h"

void MorphWindow::mouseButtonCb(GLFWwindow* win, int btn, int act, int mods) {
    (void)mods;
    auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root) return;
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    MorphEvent e;
    e.type = (act == GLFW_PRESS) ? EventType::MouseDown : EventType::MouseUp;
    e.button = btn;
    e.x = (float)mx;
    e.y = (float)my;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);
    if (act == GLFW_PRESS) {
        e.type = EventType::Click;
        self->m_root->dispatchEvent(e, (float)mx, (float)my);
    }
}

static MorphNode* s_lastHoverNode = nullptr;

// Called on hot-reload to clear stale hover pointer
void MorphWindow::clearHoverState() { s_lastHoverNode = nullptr; }

void MorphWindow::cursorPosCb(GLFWwindow* win, double mx, double my) {
    auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root) return;

    // Hover state tracking
    auto* newHover = self->m_root->hitTest((float)mx, (float)my);
    if (newHover != s_lastHoverNode) {
        if (s_lastHoverNode)
            s_lastHoverNode->onHover(false);
        if (newHover)
            newHover->onHover(true);
        s_lastHoverNode = newHover;
    }

    MorphEvent e;
    e.type = EventType::MouseMove;
    e.x = (float)mx;
    e.y = (float)my;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);

#ifdef MORPH_FEATURE_CURSOR
    auto* target = newHover;
    const std::string* cur = nullptr;
    for (auto* n = target; n; n = n->parent) {
        if (n->style.cursor != "default") {
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

void MorphWindow::windowSizeCb(GLFWwindow* win, int width, int height) {
    auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
    if (!self) return;
    self->m_width = width;
    self->m_height = height;
    self->m_pendingRender = true;
    if (self->m_root) {
        self->m_root->markDirty(SubtreeDirty);
    }
}

void MorphWindow::scrollCb(GLFWwindow* win, double dx, double dy) {
    (void)dx;
    auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
    if (!self || !self->m_root) return;
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    MorphEvent e;
    e.type = EventType::Scroll;
    e.scroll = (float)dy;
    e.x = (float)mx;
    e.y = (float)my;
    self->m_root->dispatchEvent(e, (float)mx, (float)my);
}

MorphWindow::MorphWindow(const std::string& title, int width, int height, bool visible)
    : m_title(title), m_width(width), m_height(height), m_visible(visible) {
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
    glfwWindowHint(GLFW_OPENGL_PROFILE, GLFW_OPENGL_CORE_PROFILE);
    m_handle = glfwCreateWindow(width, height, title.c_str(), nullptr, nullptr);
    if (m_handle) {
        glfwMakeContextCurrent(m_handle);
        gladLoadGLLoader(reinterpret_cast<GLADloadproc>(glfwGetProcAddress));
        glfwSetWindowUserPointer(m_handle, this);
        glfwSetMouseButtonCallback(m_handle, mouseButtonCb);
        glfwSetCursorPosCallback(m_handle, cursorPosCb);
        glfwSetScrollCallback(m_handle, scrollCb);
        glfwSetWindowSizeCallback(m_handle, windowSizeCb);
        glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
#ifdef MORPH_FEATURE_CURSOR
        m_handCursor = glfwCreateStandardCursor(GLFW_HAND_CURSOR);
        m_textCursor = glfwCreateStandardCursor(GLFW_IBEAM_CURSOR);
#endif
    }
}

void MorphWindow::setTitle(const std::string& title) {
    m_title = title;
    if (m_handle) glfwSetWindowTitle(m_handle, title.c_str());
}

// Forward declarations for dirty rendering helpers
static int countNodes(MorphNode* n);
static void recordPaintTree(MorphNode* n, Renderer& r, DirtyStats& stats);

void MorphWindow::setSize(int width, int height) {
    m_width = width;
    m_height = height;
    if (m_handle) glfwSetWindowSize(m_handle, width, height);
}

MorphWindow::~MorphWindow() {
#ifdef MORPH_FEATURE_CURSOR
    if (m_handCursor) glfwDestroyCursor(m_handCursor);
    if (m_textCursor) glfwDestroyCursor(m_textCursor);
#endif
    if (m_handle) glfwDestroyWindow(m_handle);
}

void MorphWindow::render(std::function<void(GLRenderer&, DirtyStats&)> overlayFn) {
    if (!m_handle) return;
    glfwMakeContextCurrent(m_handle);
    glViewport(0, 0, m_width, m_height);
    m_renderer.setFBHeight(m_height);
    float proj[16];
    ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);

    m_dirtyStats.reset();

    if (m_root) {
        // Set clear color from body background (matches body's CSS background)
        {
            auto& bg = m_root->style.bgColor;
            if (bg[3] > 0.0f)
                m_renderer.setClearColor(bg[0], bg[1], bg[2], bg[3]);
            else
                m_renderer.setClearColor(1.0f, 1.0f, 1.0f, 1.0f);
        }

#ifdef MORPH_FEATURE_DIRTY_RENDERING
        // Ensure renderer is ready (FreeType init, shaders) before layout,
        // since layout may call measureTextWidth which needs font atlases
        m_renderer.ensureReady();

        // Phase 1: Incremental layout (only dirty nodes)
        m_root->layoutIfNeeded(0.0f, 0.0f, (float)m_width, (float)m_height,
                               &m_renderer, &m_dirtyStats);
        m_dirtyStats.fullTreeCount = countNodes(m_root);

        // Phase 2: Record display lists for paint-dirty nodes
        recordPaintTree(m_root, m_renderer, m_dirtyStats);

        // Phase 3: Clear + render
        m_renderer.clear();
        m_renderer.setProjection(proj);

        // Phase 4: Execute display lists (all nodes)
        m_root->executeDisplayList(m_renderer);
#else
        // Legacy: full rebuild every frame
        m_renderer.ensureReady();
        m_root->layout(0.0f, 0.0f, (float)m_width, (float)m_height, &m_renderer);
        m_renderer.setProjection(proj);
        m_root->draw(m_renderer);
#endif
    }

    m_renderer.flush(proj);
    if (overlayFn) {
        overlayFn(m_renderer, m_dirtyStats);
        m_renderer.flush(proj);
    }
    glfwSwapBuffers(m_handle);

#ifdef MORPH_FEATURE_DIRTY_RENDERING
    m_prevHadDirty = !m_root || !m_root->isFullyClean();
#endif
    m_pendingRender = false;
}

static int countNodes(MorphNode* n) {
    int c = 1;
    for (auto* child : n->children) c += countNodes(child);
    return c;
}

static void recordPaintTree(MorphNode* n, Renderer& r, DirtyStats& stats) {
    if (n->isDirty(PaintDirty) || n->isDirty(ScrollDirty) || n->isDirty(StyleDirty)) {
        stats.paintCount++;
        n->recordDisplayList(r);
        n->clearDirty(PaintDirty);
        n->clearDirty(ScrollDirty);
    }
    for (auto* child : n->children)
        recordPaintTree(child, r, stats);
}
