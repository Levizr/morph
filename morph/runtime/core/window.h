#pragma once
#include <functional>
#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>
#include <string>
#include "node.h"
#include "../render/gl_renderer.h"
#include "event.h"
#include "compositor.h"

// Optional hook for dev tools to observe which nodes are repainted each frame.
// Left null in production builds — zero cost. Dev runtime sets it at startup.
using RepaintHookFn = void (*)(MorphNode *);
extern RepaintHookFn g_repaintHook;

class MorphWindow
{
    std::string m_title;
    int m_width, m_height;
    bool m_visible;
    bool m_vsync = true;
    GLFWwindow *m_handle = nullptr;
    MorphNode *m_root = nullptr;
    GLRenderer m_renderer; // fallback renderer (single-threaded path)
    Compositor *m_compositor = nullptr;
#ifdef MORPH_FEATURE_CURSOR
    GLFWcursor *m_handCursor = nullptr;
    GLFWcursor *m_textCursor = nullptr;
#endif

    static void ortho(float *m, float l, float r, float b, float t, float n, float f)
    {
        m[0] = 2.0f / (r - l);
        m[4] = 0.0f;
        m[8] = 0.0f;
        m[12] = -(r + l) / (r - l);
        m[1] = 0.0f;
        m[5] = 2.0f / (t - b);
        m[9] = 0.0f;
        m[13] = -(t + b) / (t - b);
        m[2] = 0.0f;
        m[6] = 0.0f;
        m[10] = -2.0f / (f - n);
        m[14] = -(f + n) / (f - n);
        m[3] = 0.0f;
        m[7] = 0.0f;
        m[11] = 0.0f;
        m[15] = 1.0f;
    }

public:
    // Static GLFW callbacks are public so the dev runtime can chain them
    // after intercepting its own DevTools input.
    static void mouseButtonCb(GLFWwindow *win, int btn, int act, int mods);
    static void KeyCb(GLFWwindow* win, int key, int scancode, int action, int mods);
    static void cursorPosCb(GLFWwindow *win, double mx, double my);
    static void windowSizeCb(GLFWwindow *win, int width, int height);
    static void scrollCb(GLFWwindow *win, double dx, double dy);

public:
    MorphWindow(const std::string &title, int width, int height, bool visible = true);
    ~MorphWindow();
    void addChild(MorphNode *node) { m_root = node; }
    void update(float dt)
    {
        if (m_root)
            m_root->update(dt);
    }
    static void clearHoverState();
    static void clearActiveState();
    void setTitle(const std::string &title);
    void setSize(int width, int height);
    int width() const { return m_width; }
    int height() const { return m_height; }
    GLFWwindow *handle() const { return m_handle; }
    bool shouldClose() const { return m_handle && glfwWindowShouldClose(m_handle); }
    bool isVisible() const { return m_visible && m_handle && !glfwWindowShouldClose(m_handle); }

    // Single-threaded render (legacy / fallback)
    void render(std::function<void(GLRenderer &, DirtyStats &)> overlayFn = {});

    // New: commit a frame for the compositor thread
    void commitFrame();

    // Render the latest interpolated frame (main thread, GL context current)
    void renderFrame(std::function<void(GLRenderer &, DirtyStats &)> overlayFn = {});

    // Start/stop compositor thread
    void startCompositor(bool vsync = true);
    void stopCompositor();

    // Dirty rendering accessors (for devtools)
    DirtyStats &dirtyStats() { return m_dirtyStats; }
    GLRenderer &renderer() { return m_renderer; }

private:
    void renderNode(const RenderFrame *frame, int nodeIdx);
    static void drawOpsForNode(GLRenderer &r, const RenderFrame *frame, int nodeIdx,
                               float ox, float oy);
    static void drawScrollbar(GLRenderer &r, const FlatRenderNode &node,
                              float sx, float sy, float sw, float sh);

    DirtyStats m_dirtyStats;
    bool m_prevHadDirty = true;
    bool m_pendingRender = true;

public:
    bool hasPendingRender() const
    {
        if (m_pendingRender)
            return true;
        if (m_root && !m_root->isFullyClean())
            return true;
        return false;
    }
    void clearPendingRender() { m_pendingRender = false; }
    void notifyPendingRender() { m_pendingRender = true; }
};
