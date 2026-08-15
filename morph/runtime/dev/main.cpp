#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <csignal>
#include <chrono>
#include <thread>
#include <string>
#include <functional>

#ifdef _WIN32
#include <windows.h>
#else
#include <unistd.h>
#include <dlfcn.h>
#endif

#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>

#include "../core/window.h"
#include "../core/window_manager.h"
#include "../core/node.h"
#include "../core/renderer.h"
#include "../render/gl_renderer.h"
#include "../reactivity/signal.h"

#include "dev_socket.h"
#include "ir_deserializer.h"
#include "inspector.h"
#include "signal_store.h"
#include "node_registry.h"
#include "dev_log.h"

static volatile bool g_running = true;
static void signalHandler(int) { g_running = false; }

// ── Logic library loading (dlopen on POSIX, LoadLibrary on Windows) ──
static void* g_logic_handle = nullptr;
static std::string g_logic_path;

static bool fileExists(const char* path) {
    FILE* f = fopen(path, "rb");
    if (f) {
        fclose(f);
        return true;
    }
    return false;
}

static void* libraryLoad(const char* path) {
#ifdef _WIN32
    return (void*)LoadLibraryA(path);
#else
    return dlopen(path, RTLD_NOW);
#endif
}

static void* librarySym(void* handle, const char* name) {
#ifdef _WIN32
    return (void*)GetProcAddress((HMODULE)handle, name);
#else
    return dlsym(handle, name);
#endif
}

static const char* libraryError() {
#ifdef _WIN32
    return "LoadLibrary failed";
#else
    return dlerror();
#endif
}

static void close_logic() {
    if (g_logic_handle) {
        void (*cleanup)() = (void (*)())librarySym(g_logic_handle, "morph_logic_cleanup");
        if (cleanup) cleanup();
#ifdef _WIN32
        FreeLibrary((HMODULE)g_logic_handle);
#else
        for (int i = 0; i < 5; i++) {
            int rc = dlclose(g_logic_handle);
            if (rc == 0) {
                // Verify truly unloaded: RTLD_NOLOAD returns NULL if not loaded
                if (!g_logic_path.empty()) {
                    void* still = dlopen(g_logic_path.c_str(), RTLD_NOLOAD | RTLD_NOW);
                    if (!still) break;
                    dlclose(still);
                } else {
                    break;
                }
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
#endif
        g_logic_handle = nullptr;
    }
}

static void open_logic(const std::string& path, NodeRegistry& registry, SignalStore& store) {
    close_logic();
    g_logic_path = path;
    void* handle = libraryLoad(path.c_str());
    if (!handle) {
        fprintf(stderr, "[morph] failed to load logic library: %s\n", libraryError());
        devLogAdd(LOG_ERROR, "logic library failed to load: " + std::string(libraryError()));
        return;
    }
    void (*init)(NodeRegistry&, SignalStore&) =
        (void (*)(NodeRegistry&, SignalStore&))librarySym(handle, "morph_logic_init");
    if (!init) {
        fprintf(stderr, "[morph] morph_logic_init not found in logic library: %s\n", libraryError());
        devLogAdd(LOG_ERROR, "logic library missing morph_logic_init: " + std::string(libraryError()));
#ifdef _WIN32
        FreeLibrary((HMODULE)handle);
#else
        dlclose(handle);
#endif
        return;
    }
    init(registry, store);
    g_logic_handle = handle;
    devLogAdd(LOG_OK, "logic library loaded & initialized (registry has " +
              std::to_string(registry.size()) + " nodes)");
}

// Re-initialize logic in place when the same library is already loaded.
// Avoids unload/reload so file-scope state (signal statics, effect
// signatures) survives hot reloads of styles/tree without re-running effects.
static void reinit_logic(NodeRegistry& registry, SignalStore& store) {
    if (!g_logic_handle) {
        open_logic(g_logic_path, registry, store);
        return;
    }
    void (*cleanup)() = (void (*)())librarySym(g_logic_handle, "morph_logic_cleanup");
    if (cleanup) cleanup();
    void (*rewire)(NodeRegistry&, SignalStore&) =
        (void (*)(NodeRegistry&, SignalStore&))librarySym(g_logic_handle, "morph_logic_rewire");
    if (rewire) {
        rewire(registry, store);
        return;
    }
    void (*init)(NodeRegistry&, SignalStore&) =
        (void (*)(NodeRegistry&, SignalStore&))librarySym(g_logic_handle, "morph_logic_init");
    if (init) init(registry, store);
}

static bool reload_logic(const std::string& path, NodeRegistry& registry, SignalStore& store) {
    close_logic();
    open_logic(path, registry, store);
    return g_logic_handle != nullptr;
}

// ── DevTools (global for GLFW callbacks) ────────────────
static DevTools* g_devtools = nullptr;

// Lazily-created horizontal-resize cursor for the DevTools panel resize handle.
static GLFWcursor* s_ewCursor = nullptr;
static GLFWcursor* ewResizeCursor() {
    if (!s_ewCursor)
        s_ewCursor = glfwCreateStandardCursor(GLFW_HRESIZE_CURSOR);
    return s_ewCursor;
}

static void collectRepaint(MorphNode* n) {
    if (g_devtools) g_devtools->noteRepaint(n);
}

static void keyCb(GLFWwindow* win, int key, int scancode, int action, int mods) {
    if (g_devtools && action == GLFW_PRESS) {
        if (key == GLFW_KEY_F12) {
            g_devtools->toggle();
            auto* mwin = (MorphWindow*)glfwGetWindowUserPointer(win);
            if (mwin)
                mwin->setDevtoolsWidth(g_devtools->open ? g_devtools->m_panelW : 0.0f);
            return;
        }
        if (key == GLFW_KEY_F2 && g_devtools->open) { g_devtools->toggleInspect(); return; }
        if (key == GLFW_KEY_ESCAPE && (g_devtools->open || g_devtools->inspecting)) {
            g_devtools->cancelInspect();
            return;
        }
    }
    MorphWindow::KeyCb(win, key, scancode, action, mods);
}

static void mouseCb(GLFWwindow* win, int btn, int act, int mods) {
    if (btn != GLFW_MOUSE_BUTTON_1 || !g_devtools) {
        MorphWindow::mouseButtonCb(win, btn, act, mods);
        return;
    }
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    int w = 0, h = 0;
    glfwGetWindowSize(win, &w, &h);
    if (g_devtools->open) {
        float px = (float)w - g_devtools->m_panelW;
        // Resize handle strip (spans 8px across the panel's left edge)
        if (mx >= px - 5.0f && mx <= px + 3.0f) {
            if (act == GLFW_PRESS) {
                g_devtools->m_resizing = true;
                g_devtools->m_resizeGrabX = (float)mx;
            } else if (act == GLFW_RELEASE) {
                g_devtools->m_resizing = false;
            }
            return; // consumed by the resize handle
        }
        if (mx >= px) {
            if (act == GLFW_PRESS)
                g_devtools->handleClick((float)mx, (float)my, (float)w, (float)h);
            else if (act == GLFW_RELEASE)
                g_devtools->endLogDrag();
            return; // consumed by the panel
        }
    }
    if (g_devtools->inspecting && act == GLFW_PRESS) {
        g_devtools->selectHovered();
        return; // consumed by inspect mode
    }
    MorphWindow::mouseButtonCb(win, btn, act, mods);
}

static void scrollCb(GLFWwindow* win, double dx, double dy) {
    if (g_devtools && g_devtools->open) {
        double mx;
        glfwGetCursorPos(win, &mx, nullptr);
        int w = 0;
        glfwGetWindowSize(win, &w, nullptr);
        if (mx >= w - g_devtools->m_panelW) {
            g_devtools->scroll((float)dy);
            return;
        }
    }
    MorphWindow::scrollCb(win, dx, dy);
}

static void cursorCb(GLFWwindow* win, double mx, double my) {
    if (g_devtools) {
        if (g_devtools->m_resizing) {
            int w = 0, h = 0;
            glfwGetWindowSize(win, &w, &h);
            float pw = (float)w - (float)mx;
            float maxPw = (float)w - 360.0f; // keep the app at least ~360px wide
            if (pw < DevTools::kMinPanelW) pw = DevTools::kMinPanelW;
            if (pw > maxPw) pw = maxPw;
            if (pw != g_devtools->m_panelW) {
                g_devtools->m_panelW = pw;
                auto* mwin = (MorphWindow*)glfwGetWindowUserPointer(win);
                if (mwin) mwin->setDevtoolsWidth(pw);
            }
            glfwSetCursor(win, ewResizeCursor());
            return;
        }
        if (g_devtools->open) {
            int w = 0, h = 0;
            glfwGetWindowSize(win, &w, &h);
            float px = (float)w - g_devtools->m_panelW;
            // Resize handle hover → horizontal-resize cursor
            if (mx >= px - 5.0f && mx <= px + 3.0f) {
                glfwSetCursor(win, ewResizeCursor());
                return;
            }
            if (mx >= px)
                g_devtools->handleCursorPos((float)mx, (float)my, (float)w, (float)h);
        }
    }
    MorphWindow::cursorPosCb(win, mx, my);
}

// Handle non-IR control messages (errors + logs) from the Python client.
static bool handleControlMessage(const std::string& msg, bool haveDevtools) {
    if (msg.find("__error__") == std::string::npos &&
        msg.find("__log__") == std::string::npos) {
        return false;
    }
    JsonValue in;
    try {
        in = JsonValue::parse(msg);
    } catch (std::exception&) {
        return false;
    }
    if (in.type() != JsonType::Object) return false;
    if (in.has("__error__")) {
        std::string err = in["__error__"].asString();
        fprintf(stderr, "[morph] error from client: %s\n", err.c_str());
        devLogAdd(LOG_ERROR, err);
        if (haveDevtools) g_devtools->showToast(LOG_ERROR, err);
        return true;
    }
    if (in.has("__log__")) {
        auto& l = in["__log__"];
        std::string lvl = l["level"].asString();
        std::string txt = l["msg"].asString();
        int level = LOG_INFO;
        if (lvl == "error") level = LOG_ERROR;
        else if (lvl == "warn") level = LOG_WARN;
        else if (lvl == "ok") level = LOG_OK;
        devLogAdd(level, txt);
        if (haveDevtools && (level == LOG_ERROR || level == LOG_WARN))
            g_devtools->showToast(level, txt);
        // Only echo warnings/errors to the terminal; info/ok updates (file
        // changed, hot reloaded) are shown as a single-line spinner on the
        // Python side and kept in the DevTools log panel.
        if (level == LOG_ERROR || level == LOG_WARN)
            fprintf(stderr, "[morph] log[%s]: %s\n", devLogLevelName(level), txt.c_str());
        return true;
    }
    return false;
}

int main() {
    signal(SIGINT, signalHandler);
    signal(SIGTERM, signalHandler);

    // Unbuffered stdio so piped logs appear in real time, in order
    setvbuf(stdout, nullptr, _IONBF, 0);
    setvbuf(stderr, nullptr, _IONBF, 0);

    // ── Socket server ──────────────────────────────────────
    DevSocket sock;
    if (!sock.listen()) {
        fprintf(stderr, "[morph] dev server: failed to create socket\n");
        return 1;
    }

    // ── Wait for first client connection ───────────────────
    while (g_running && !sock.acceptClient()) {
        std::this_thread::sleep_for(std::chrono::milliseconds(100)); // no GLFW yet
    }
    if (!g_running) return 0;
    if (!sock.isConnected()) {
        fprintf(stderr, "[morph] dev server: no client connected\n");
        return 1;
    }

    // ── Read first IR (skipping any control/error messages) ──
    std::string json;
    while (g_running) {
        if (!sock.readMessage(json, 2000)) {
            if (!sock.isConnected()) {
                fprintf(stderr, "[morph] dev server: client disconnected during read\n");
                return 1;
            }
            continue;
        }
        if (json.empty()) continue;
        if (handleControlMessage(json, false)) {
            json.clear();
            continue;
        }
        break;
    }
    if (!g_running) return 0;
    if (json.empty()) {
        fprintf(stderr, "[morph] dev server: empty IR, exiting\n");
        return 1;
    }

    // ── Parse IR, extract window config ────────────────────
    JsonValue root;
    try {
        root = JsonValue::parse(json);
    } catch (std::exception& e) {
        fprintf(stderr, "[morph] dev server: JSON parse error: %s\n", e.what());
        return 1;
    }

    NodeRegistry registry;
    SignalStore signalStore;
    std::vector<StateVarInfo> stateVars;
    DevWindowConfig config;
    MorphNode* rootNode = nullptr;
    if (!parseIR(root, rootNode, config, registry, stateVars)) {
        fprintf(stderr, "[morph] dev server: failed to parse IR\n");
        return 1;
    }

    // Handle renderer selection from config
    if (config.renderer == "forge") {
#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
        setRenderMode(RenderMode::Forge);
#endif
        fprintf(stderr, "[morph] renderer: forge (selected via config)\n");
    } else {
#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
        setRenderMode(RenderMode::Flash);
#endif
        fprintf(stderr, "[morph] renderer: flash (default)\n");
    }

    // Create signals from state vars
    for (auto& sv : stateVars) {
        std::string raw = sv.init;
        // Strip quotes if string
        if (raw.size() >= 2 && raw[0] == '\'' && raw.back() == '\'')
            raw = raw.substr(1, raw.size() - 2);
        if (sv.type == "int")
            signalStore.get_or_create<int>(sv.getter, std::stoi(raw));
        else if (sv.type == "double")
            signalStore.get_or_create<double>(sv.getter, std::stod(raw));
        else if (sv.type == "bool")
            signalStore.get_or_create<bool>(sv.getter, raw == "true");
        else if (sv.type == "std::string")
            signalStore.get_or_create<std::string>(sv.getter, raw);
    }

    // Load the logic library — prefer path from IR, fallback to env var
    std::string logicSoPath;
    if (root.has("logic_so_path")) {
        logicSoPath = root["logic_so_path"].asString();
    }
    if (logicSoPath.empty()) {
        const char* logicPath = getenv("MORPH_LOGIC_PATH");
        if (logicPath && *logicPath) {
            logicSoPath = logicPath;
        } else {
#ifdef _WIN32
            logicSoPath = "logic.dll";
#else
            logicSoPath = "/tmp/morph_cache/logic.so";
#endif
        }
    }
    if (fileExists(logicSoPath.c_str())) {
        open_logic(logicSoPath, registry, signalStore);
    } else {
        fprintf(stderr, "[morph] no logic library found at %s (interactivity disabled)\n",
                logicSoPath.c_str());
    }

    // ── Init GLFW and create window ────────────────────────
    glfwSetErrorCallback([](int code, const char* msg) {
        fprintf(stderr, "[morph] GLFW error %d: %s\n", code, msg);
    });

    if (!glfwInit()) {
        fprintf(stderr, "[morph] glfwInit() failed\n");
        return 1;
    }

    {
    MorphWindow window(config.title, config.width, config.height, config.visible);
    if (!window.handle()) {
        fprintf(stderr, "[morph] glfwCreateWindow failed — cannot create OpenGL window\n");
        fprintf(stderr, "[morph] On Wayland, try: GDK_BACKEND=x11 GDK_DEBUG=x11-override\n");
        glfwTerminate();
        return 1;
    }
    window.addChild(rootNode);

    // ── DevTools ─────────────────────────────────────────────
    DevTools devtools;
    g_devtools = &devtools;
    g_repaintHook = collectRepaint;
    glfwSetKeyCallback(window.handle(), keyCb);
    glfwSetMouseButtonCallback(window.handle(), mouseCb);
    glfwSetScrollCallback(window.handle(), scrollCb);
    glfwSetCursorPosCallback(window.handle(), cursorCb);

    // ── Start compositor thread ─────────────────────────────
    window.startCompositor(true); // vsync on

    // ── Main loop ───────────────────────────────────────────
    auto lastFrameTime = std::chrono::steady_clock::now();

    while (g_running && !window.shouldClose()) {
        // Process all pending GLFW events (non-blocking)
        glfwPollEvents();

        // Check for new IR from socket
        std::string msg;
        while (sock.readMessage(msg, 0)) {
            if (msg.empty()) continue;

            // Control messages (errors + logs) from Python
            if (handleControlMessage(msg, true)) {
                continue;
            }

            // Parse new IR and rebuild tree
            JsonValue newRoot;
            try {
                newRoot = JsonValue::parse(msg);
            } catch (std::exception&) {
                fprintf(stderr, "[morph] dev server: bad JSON from client\n");
                continue;
            }

            NodeRegistry newRegistry;
            std::vector<StateVarInfo> newStateVars;
            DevWindowConfig newConfig;
            MorphNode* newNode = nullptr;
            if (parseIR(newRoot, newNode, newConfig, newRegistry, newStateVars)) {
                // Apply window config (title only; resizing disrupts dev flow)
                window.setTitle(newConfig.title);
                if (newConfig.renderer == "forge")
                    setRenderMode(RenderMode::Forge);
                else
                    setRenderMode(RenderMode::Flash);
                // Replace node tree
                window.addChild(newNode);
                deleteNodeTree(rootNode);
                rootNode = newNode;
                registry = std::move(newRegistry);
                devtools.hoveredNode = nullptr; // tree changed, clear stale ref
                devtools.selectedNode = nullptr; // tree changed, clear stale ref
                devtools.clearRepaintTimers(); // nodes will be deleted
                MorphWindow::clearHoverState(); // clear stale hover pointer
                MorphWindow::clearActiveState(); // clear stale :active pointer

                // Add any new signals from updated state vars
                for (auto& sv : newStateVars) {
                    if (!signalStore.has(sv.getter)) {
                        std::string raw = sv.init;
                        if (raw.size() >= 2 && raw[0] == '\'' && raw.back() == '\'')
                            raw = raw.substr(1, raw.size() - 2);
                        if (sv.type == "int")
                            signalStore.get_or_create<int>(sv.getter, std::stoi(raw));
                        else if (sv.type == "double")
                            signalStore.get_or_create<double>(sv.getter, std::stod(raw));
                        else if (sv.type == "bool")
                            signalStore.get_or_create<bool>(sv.getter, raw == "true");
                        else if (sv.type == "std::string")
                            signalStore.get_or_create<std::string>(sv.getter, raw);
                    }
                }
                // If the logic .so is unchanged, re-init in place so effects
                // aren't torn down/re-run (no stale "startup" output per edit).
                // Otherwise do a full close + reopen.
                std::string reloadPath = logicSoPath;
                if (newRoot.has("logic_so_path")) {
                    reloadPath = newRoot["logic_so_path"].asString();
                }
                if (g_logic_handle && !reloadPath.empty() && reloadPath == g_logic_path) {
                    reinit_logic(registry, signalStore);
                    devLogAdd(LOG_INFO, "logic re-initialized in place");
                } else {
                    reload_logic(reloadPath, registry, signalStore);
                    devLogAdd(LOG_OK, "hot reloaded logic (" + reloadPath + ")");
                }
                window.notifyPendingRender();
            } else {
                fprintf(stderr, "[morph] dev server: failed to parse IR from client\n");
            }
        }

        // Re-accept client if disconnected
        if (!sock.isConnected()) {
            sock.acceptClient();
        }

        // Delta time for animations
        auto now = std::chrono::steady_clock::now();
        float dt = std::chrono::duration<float>(now - lastFrameTime).count();
        if (dt > 0.1f) dt = 0.1f; // cap to prevent spiral of death
        lastFrameTime = now;

        // Run pending reactive effects
        morph::run_pending_effects();

        // Update main-thread animation state (only tracks time, compositor interpolates)
        if (rootNode) rootNode->update(dt);

        // Commit frame — layout + paint + flatten + atomic swap
        if (window.isVisible()) {
            // Track mouse for devtools inspect
            if (devtools.inspecting) {
                double mx, my;
                glfwGetCursorPos(window.handle(), &mx, &my);
                devtools.mouseX = (float)mx;
                devtools.mouseY = (float)my;
                devtools.updateHover(rootNode);
            }
            // Skip idle frames entirely (no layout, paint, flatten, or GL
            // work) unless something changed or the devtools panel is open.
            // The devtools overlay is drawn outside the app tree, so it can't
            // rely on dirty flags to trigger repaints.
            bool debugRender = devtools.open || window.hasPendingRender();
            if (debugRender) {
                window.commitFrame();
                window.renderFrame([&](GLRenderer& r, DirtyStats&) {
                    devtools.render(r, (float)window.width(), (float)window.height(),
                                    window.dirtyStats());
                });
            }
        } else {
            // Hidden window — no point spinning, wait for events
            glfwWaitEvents();
        }
    }

    window.stopCompositor();

    // ~MorphWindow cleans up rootNode — don't double-delete
    // window destroyed here (before glfwTerminate)
    } // ~MorphWindow, ~DevTools

    close_logic();
    glfwTerminate();
    return 0;
}

