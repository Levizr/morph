#pragma once
#include <functional>
#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>
#include <string>
#include "node.h"
#include "../render/gl_renderer.h"
#include "event.h"
#include "compositor.h"
#include "../renderers/renderer.h"
#include "render_frame.h"

// Forward-declared (used only as an optional pointer in render signatures).
// Full definition lives in renderers/forge/damage.h.
struct DamageSet;

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

    // Docked devtools: the panel occupies the right side of the window and the
    // app's layout is constrained to the remaining content area, so the panel
    // never covers app elements (browser-style docking).
    void setDevtoolsWidth(float w) {
        m_devtoolsWidth = w < 0.0f ? 0.0f : w;
        m_pendingRender = true;
        if (m_root) m_root->markDirty(SubtreeDirty);
    }
    float devtoolsWidth() const { return m_devtoolsWidth; }
    // App content area (window minus the devtools strip). Clamped so the app
    // never collapses to zero even if the user drags the panel very wide.
    float contentWidth() const {
        float cw = (float)m_width - m_devtoolsWidth;
        return cw < 120.0f ? 120.0f : cw;
    }
    float contentHeight() const { return (float)m_height; }

    // Single-threaded render (legacy / fallback)
    void render(std::function<void(GLRenderer &, DirtyStats &)> overlayFn = {});

    // New: commit a frame for the compositor thread
    void commitFrame();

    // Render the latest interpolated frame (main thread, GL context current)
    void renderFrame(std::function<void(GLRenderer &, DirtyStats &)> overlayFn = {});

    // Draw the flat node tree into the renderer's current target (used by the
    // forge retained-FBO present path). GL context must be current; callers
    // bind the target framebuffer beforehand.
    // damageClip (optional): when set, nodes whose drawn rect does not touch
    // the damage are skipped — their pixels are already correct in the retained
    // surface. Clipping nodes never skip (their clip reveals descendants).
    void drawFrameNodes(const DamageSet *damageClip = nullptr);

    // Start/stop compositor thread
    void startCompositor(bool vsync = true);
    void stopCompositor();

    // Dirty rendering accessors (for devtools)
    DirtyStats &dirtyStats() { return m_dirtyStats; }
    GLRenderer &renderer() { return m_renderer; }
    bool hasRoot() const { return m_root != nullptr; }
    MorphNode* root() const { return m_root; }

private:
    void renderNode(const RenderFrame *frame, int nodeIdx,
                    const DamageSet *damageClip = nullptr,
                    float scrollOffset = 0.0f);
    static void drawOpsForNode(GLRenderer &r, const RenderFrame *frame, int nodeIdx,
                               float ox, float oy);
    static void drawScrollbar(GLRenderer &r, const FlatRenderNode &node,
                              float sx, float sy, float sw, float sh);

    DirtyStats m_dirtyStats;
    bool m_prevHadDirty = true;
    bool m_pendingRender = true;
    float m_devtoolsWidth = 0.0f;

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

// Shared dirty-tree helpers used by both the fallback single-threaded path
// (window.cpp) and the flash renderer. Non-static so renderers can call them.
int countNodes(MorphNode *n);
void recordPaintTree(MorphNode *n, Renderer &r, DirtyStats &stats);
#ifdef MORPH_FEATURE_DEV
void syncPaintDirtyTree(MorphNode *n);
#endif
