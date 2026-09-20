#pragma once
#include "vendor/glad/glad.h"
#include <unordered_map>
#include <string>
#include <vector>
#include <memory>
#include <functional>
#include <GLFW/glfw3.h>

#include "window.h"

// Window instance id. Every window-id literal in user code is interned at
// build time to a WID (the MID pattern: useWindow("login-a") emits
// useWindow_WID(3)) — switch-dispatched, zero hash lookups on the hot path.
// RID (route ids, app::routes::) live in the generated morph_routes.h.
using WID = int;
inline constexpr WID kInvalidWid = -1;

class WindowManager {
    // Registry owns every window (shared_ptr). Handles resolve their WID
    // through the registry at every call, so an X-button / task-manager
    // close can never dangle a handle — it just starts missing.
    std::unordered_map<WID, std::shared_ptr<MorphWindow>> m_windows;
    // Build-time string tables, consulted once at registration — never
    // per call. Aliases cover explicit `id:` literals (dynamic ids fall
    // back to a runtime lookup + mx-window-dynamic warning).
    std::unordered_map<std::string, WID> m_aliases;
    // Native handle -> WID, for focus callbacks that only get a GLFWwindow*.
    std::unordered_map<GLFWwindow*, WID> m_handles;
    // Close handlers per window (JS on('close') lowers to these).
    std::unordered_map<WID, std::vector<std::function<void()>>> m_closeHandlers;
    WID m_focusedWid = kInvalidWid;

    // Shared close path: user X-button (via sweepClosed) and explicit
    // close() both end here. Callers collect WIDs first, then erase —
    // never erase while iterating m_windows.
    void destroyLocked(WID wid)
    {
        auto it = m_windows.find(wid);
        if (it == m_windows.end())
            return;
        auto handlers = std::move(m_closeHandlers[wid]);
        m_closeHandlers.erase(wid);
        for (auto& fn : handlers)
        {
            if (fn)
                fn();
        }
        it->second->stopCompositor();
        if (it->second->handle())
            m_handles.erase(it->second->handle());
        if (m_focusedWid == wid)
            m_focusedWid = kInvalidWid;
        // ~MorphWindow binds its own context for renderer teardown and
        // detaches afterwards — no context work needed here.
        m_windows.erase(it);
    }

public:
    ~WindowManager()
    {
        m_windows.clear();
        m_aliases.clear();
        m_handles.clear();
        m_closeHandlers.clear();
        // All windows (and their GLFW handles/cursors) are gone now, so this
        // is the last safe moment to shut down the GLFW library. Terminating
        // earlier (e.g. in main()) makes ~MorphWindow call into a dead GLFW.
        glfwTerminate();
    }

    static WindowManager& get()
    {
        static WindowManager inst;
        return inst;
    }

    void registerWindow(WID wid, std::shared_ptr<MorphWindow> w)
    {
        m_windows[wid] = std::move(w);
        if (m_windows[wid] && m_windows[wid]->handle())
            m_handles[m_windows[wid]->handle()] = wid;
    }

    void registerAlias(const std::string& name, WID wid)
    {
        m_aliases[name] = wid;
    }

    bool resolveAlias(const std::string& name, WID& wid) const
    {
        auto it = m_aliases.find(name);
        if (it == m_aliases.end())
            return false;
        wid = it->second;
        return m_windows.count(wid) > 0;
    }

    std::shared_ptr<MorphWindow> get(WID wid) const
    {
        auto it = m_windows.find(wid);
        return it == m_windows.end() ? nullptr : it->second;
    }

    bool exists(WID wid) const
    {
        return m_windows.count(wid) > 0;
    }

    // True once closed — by you OR by the user. Unknown WIDs read as
    // closed: the safe answer when the registry knows nothing.
    bool closed(WID wid) const
    {
        return !exists(wid);
    }

    // Show a registered-but-hidden window (created with visible=false).
    // No-op when the window doesn't exist.
    void open(WID wid)
    {
        auto w = get(wid);
        if (w)
            w->show();
    }

    // Destroy + erase. Fires on_close handlers. Safe no-op returning
    // false when the window is already gone — never crashes.
    bool close(WID wid)
    {
        if (!exists(wid))
            return false;
        destroyLocked(wid);
        return true;
    }

    void onClose(WID wid, std::function<void()> fn)
    {
        if (exists(wid) && fn)
            m_closeHandlers[wid].push_back(std::move(fn));
    }

    // Called from MorphWindow::windowFocusCb via the GLFW focus callback.
    void noteFocus(GLFWwindow* handle)
    {
        auto it = m_handles.find(handle);
        m_focusedWid = it == m_handles.end() ? kInvalidWid : it->second;
    }

    // Most-recently-focused live window, or kInvalidWid. Backs
    // useWindow("/route") duplicate resolution (per-route focus arrives
    // with RID tracking).
    WID focusedWid() const
    {
        if (m_focusedWid != kInvalidWid && !exists(m_focusedWid))
            return kInvalidWid;
        return m_focusedWid;
    }

    // Page navigation lands with the route manifest + factories
    // (docs/future/file-routing.md) — until then the signature reserves
    // the WID/RID shape so call sites don't churn.
    void navigate(WID windowId, int routeId)
    {
        (void)windowId;
        (void)routeId;
    }

    // Pump every visible window: advance animations, render when dirty.
    // The generated main loop calls this instead of touching a static
    // window list, so dynamically created windows render too.
    void pump(float dt)
    {
        for (auto& [wid, w] : m_windows)
        {
            if (!w->isVisible())
                continue;
            // renderFrame issues GL against the calling thread's context,
            // so bind this window explicitly — creation order (or a dynamic
            // birth mid-frame) must never decide what paints.
            if (w->handle())
                glfwMakeContextCurrent(w->handle());
            // Advance transitions/animations; cheap when idle.
            w->update(dt);
            // Dirty rendering: skip layout, paint and the GL swap entirely
            // unless something changed (input, effect, size, or animation).
            if (w->hasPendingRender())
            {
                w->commitFrame();
                w->renderFrame();
            }
        }
    }

    // Reap windows the user closed via X / task manager / OS: stop their
    // compositor, fire on_close, erase. Runs in the main loop (never
    // inside a GLFW callback) so destruction is always safe.
    void sweepClosed()
    {
        std::vector<WID> dead;
        for (auto& [wid, w] : m_windows)
        {
            if (w->shouldClose())
                dead.push_back(wid);
        }
        for (WID wid : dead)
            destroyLocked(wid);
    }

    void startAllCompositors(bool vsync = true)
    {
        for (auto& [_, w] : m_windows)
            w->startCompositor(vsync);
    }

    void stopAllCompositors()
    {
        for (auto& [_, w] : m_windows)
            w->stopCompositor();
    }

    bool allClosed() const
    {
        if (m_windows.empty())
            return true;
        for (const auto& [_, w] : m_windows)
        {
            if (!w->shouldClose())
                return false;
        }
        return true;
    }
};
