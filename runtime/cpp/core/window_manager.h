#pragma once
#include "vendor/glad/glad.h"
#include <unordered_map>
#include <string>
#include <vector>
#include <memory>
#include <functional>
#include <cstdlib>
#include <GLFW/glfw3.h>

#include "window.h"

// Window instance id. Every window-id literal in user code is interned at
// build time to a WID (the MID pattern: useWindow("login-a") emits
// useWindow_WID(3)) — switch-dispatched, zero hash lookups on the hot path.
// RID (route ids, app::routes::) live in the generated morph_routes.h.
using WID = int;
inline constexpr WID kInvalidWid = -1;

// One mounted route instance: the route's context (signals + owned
// effects, type-erased — the route's own deleter destroys effects)
// plus the teardown that runs before the handle is dropped. Touched
// only on mount/unmount/navigate/close — never per frame.
struct MountHandle {
    int rid = -1;
    std::shared_ptr<void> ctx;
    std::function<void()> teardown;
};

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
    // Mounted route per window (empty for declarative entry windows).
    std::unordered_map<WID, MountHandle> m_mounts;
    WID m_focusedWid = kInvalidWid;
    // Focus recency (front = most recent). Backs by-route lookup
    // (`useWindow("/r")` picks the first live window on that route).
    // Touched only on focus/destroy — never per frame.
    std::vector<WID> m_focusOrder;
    // Runtime-minted WIDs start far above codegen-assigned ones
    // (declaration order from 0) and manual native ids (< 1M by
    // convention) so the three schemes never collide.
    WID m_nextMinted = 1000000;

    void noteFocusedLocked(WID wid)
    {
        m_focusedWid = wid;
        for (size_t i = 0; i < m_focusOrder.size(); i++)
        {
            if (m_focusOrder[i] == wid)
            {
                m_focusOrder.erase(m_focusOrder.begin() + i);
                break;
            }
        }
        m_focusOrder.insert(m_focusOrder.begin(), wid);
    }

    void forgetLocked(WID wid)
    {
        for (size_t i = 0; i < m_focusOrder.size(); i++)
        {
            if (m_focusOrder[i] == wid)
            {
                m_focusOrder.erase(m_focusOrder.begin() + i);
                break;
            }
        }
        if (m_focusedWid == wid)
            m_focusedWid = kInvalidWid;
    }

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
        forgetLocked(wid);
        clearMount(wid);
        // ~MorphWindow binds its own context for renderer teardown and
        // detaches afterwards — no context work needed here.
        m_windows.erase(it);
    }

public:
    ~WindowManager()
    {
        for (auto& [wid, _] : m_windows)
            clearMount(wid);
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

    // Hide a window without destroying it (close() destroys).
    // No-op when the window doesn't exist.
    void hide(WID wid)
    {
        auto w = get(wid);
        if (w)
            w->hide();
    }

    // Live title (empty when the window doesn't exist).
    std::string title(WID wid) const
    {
        auto w = get(wid);
        return w ? w->title() : "";
    }

    // Retitle a live window. No-op when it doesn't exist.
    void setTitle(WID wid, const std::string& title)
    {
        auto w = get(wid);
        if (w)
            w->setTitle(title);
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

    // Drop a window's mount, running its teardown (effect destruction)
    // first so no owned effect outlives its signals. Safe on empty.
    // Runs before tree deletion in every close path (explicit close,
    // X-close sweep, navigate-away).
    void clearMount(WID wid)
    {
        auto it = m_mounts.find(wid);
        if (it == m_mounts.end())
            return;
        if (it->second.teardown)
            it->second.teardown();
        m_mounts.erase(it);
    }

    // Record a route mount on a window (replaces any live mount after
    // tearing it down — callers normally unmount explicitly first).
    void setMount(WID wid, MountHandle handle)
    {
        if (!exists(wid))
            return;
        clearMount(wid);
        m_mounts[wid] = std::move(handle);
    }

    // Mounted route id, or -1 for declarative (unmounted) windows.
    int mountedRid(WID wid) const
    {
        auto it = m_mounts.find(wid);
        return it == m_mounts.end() ? -1 : it->second.rid;
    }

    // Called from MorphWindow::windowFocusCb via the GLFW focus callback.
    void noteFocus(GLFWwindow* handle)
    {
        auto it = m_handles.find(handle);
        if (it == m_handles.end())
            return;
        noteFocusedLocked(it->second);
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

    // Mint a runtime WID for dynamic windows (`new Window`). Starts far
    // above codegen and manual ids — the schemes never collide.
    WID mintWid()
    {
        return m_nextMinted++;
    }

    // WID for an explicit id, or kInvalidWid (dynamic-id fallback for
    // useWindow("id")).
    WID widForAlias(const std::string& name) const
    {
        auto it = m_aliases.find(name);
        if (it == m_aliases.end() || !exists(it->second))
            return kInvalidWid;
        return it->second;
    }

    // Most-recently-focused live window mounted on a route, or
    // kInvalidWid. Backs useWindow("/route").
    WID widForRoute(int rid) const
    {
        for (WID wid : m_focusOrder)
        {
            auto it = m_mounts.find(wid);
            if (it != m_mounts.end() && it->second.rid == rid && exists(wid))
                return wid;
        }
        return kInvalidWid;
    }

    // Open an external URL in the OS browser (never a Morph window).
    static void openUrl(const std::string& url)
    {
#if defined(__linux__)
        std::string cmd = "xdg-open \"" + url + "\" >/dev/null 2>&1 &";
        if (std::system(cmd.c_str()))
        {
        }
#elif defined(__APPLE__)
        std::string cmd = "open \"" + url + "\" >/dev/null 2>&1 &";
        if (std::system(cmd.c_str()))
        {
        }
#elif defined(_WIN32)
        std::string cmd = "start \"\" \"" + url + "\"";
        if (std::system(cmd.c_str()))
        {
        }
#endif
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
