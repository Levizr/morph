#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <csignal>
#include <unistd.h>
#include <chrono>
#include <dlfcn.h>
#include <string>

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

static volatile bool g_running = true;
static void signalHandler(int) { g_running = false; }

// ── Logic .so loading ──
static void* g_logic_handle = nullptr;
static std::string g_logic_path;

static void close_logic() {
    if (g_logic_handle) {
        void (*cleanup)() = (void (*)())dlsym(g_logic_handle, "morph_logic_cleanup");
        if (cleanup) cleanup();
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
            usleep(10000);
        }
        g_logic_handle = nullptr;
    }
}

static void open_logic(const std::string& path, NodeRegistry& registry, SignalStore& store) {
    close_logic();
    g_logic_path = path;
    void* handle = dlopen(path.c_str(), RTLD_NOW);
    if (!handle) {
        fprintf(stderr, "[devrt] failed to load logic.so: %s\n", dlerror());
        return;
    }
    void (*init)(NodeRegistry&, SignalStore&) =
        (void (*)(NodeRegistry&, SignalStore&))dlsym(handle, "morph_logic_init");
    if (!init) {
        fprintf(stderr, "[devrt] morph_logic_init not found in logic.so: %s\n", dlerror());
        dlclose(handle);
        return;
    }
    init(registry, store);
    g_logic_handle = handle;
    fprintf(stderr, "[devrt] logic.so loaded & initialized (registry has %zu nodes)", registry.size());
    // Debug: print first few node IDs
    registry.debug_print();
    fprintf(stderr, "\n");
}

static bool reload_logic(const std::string& path, NodeRegistry& registry, SignalStore& store) {
    close_logic();
    open_logic(path, registry, store);
    return g_logic_handle != nullptr;
}

// ── DevTools (global for GLFW key callback) ────────────────
static DevTools* g_devtools = nullptr;

static void keyCb(GLFWwindow* win, int key, int, int action, int) {
    (void)win;
    if (!g_devtools) return;
    if (key == GLFW_KEY_F12 && action == GLFW_PRESS) {
        g_devtools->toggle();
    }
    if (key == GLFW_KEY_F2 && action == GLFW_PRESS && g_devtools->open) {
        g_devtools->toggleInspect();
    }
}

int main() {
    signal(SIGINT, signalHandler);
    signal(SIGTERM, signalHandler);

    fprintf(stderr, "[devrt] starting...\n");

    // ── Socket server ──────────────────────────────────────
    DevSocket sock;
    if (!sock.listen("/tmp/morph_dev.sock")) {
        fprintf(stderr, "[devrt] failed to create socket\n");
        return 1;
    }

    // ── Wait for first client connection ───────────────────
    fprintf(stderr, "[devrt] waiting for client...\n");
    while (g_running && !sock.acceptClient()) {
        usleep(100000); // 100ms — no GLFW yet
    }
    if (!g_running) return 0;
    if (!sock.isConnected()) {
        fprintf(stderr, "[devrt] no client connected\n");
        return 1;
    }

    // ── Read first IR ──────────────────────────────────────
    std::string json;
    fprintf(stderr, "[devrt] reading initial IR...\n");
    while (g_running && !sock.readMessage(json, 2000)) {
        if (!sock.isConnected()) {
            fprintf(stderr, "[devrt] client disconnected during read\n");
            return 1;
        }
    }
    if (!g_running) return 0;
    if (json.empty()) {
        fprintf(stderr, "[devrt] empty IR, exiting\n");
        return 1;
    }

    // ── Parse IR, extract window config ────────────────────
    JsonValue root;
    try {
        root = JsonValue::parse(json);
    } catch (std::exception& e) {
        fprintf(stderr, "[devrt] JSON parse error: %s\n", e.what());
        return 1;
    }

    NodeRegistry registry;
    SignalStore signalStore;
    std::vector<StateVarInfo> stateVars;
    DevWindowConfig config;
    MorphNode* rootNode = nullptr;
    if (!parseIR(root, rootNode, config, registry, stateVars)) {
        fprintf(stderr, "[devrt] failed to parse IR\n");
        return 1;
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
        fprintf(stderr, "[devrt] signal \"%s\" (%s) = %s\n",
                sv.getter.c_str(), sv.type.c_str(), sv.init.c_str());
    }

    // Load logic.so — prefer path from IR, fallback to env var
    std::string logicSoPath;
    if (root.has("logic_so_path")) {
        logicSoPath = root["logic_so_path"].asString();
    }
    if (logicSoPath.empty()) {
        const char* logicPath = getenv("MORPH_LOGIC_PATH");
        logicSoPath = logicPath ? logicPath : "/tmp/morph_cache/logic.so";
    }
    if (access(logicSoPath.c_str(), F_OK) == 0) {
        open_logic(logicSoPath, registry, signalStore);
    } else {
        fprintf(stderr, "[devrt] no logic.so found at %s (interactivity disabled)\n",
                logicSoPath.c_str());
    }

    fprintf(stderr, "[devrt] window: %s %dx%d\n",
            config.title.c_str(), config.width, config.height);

    // ── Init GLFW and create window ────────────────────────
    glfwSetErrorCallback([](int code, const char* msg) {
        fprintf(stderr, "[GLFW] error %d: %s\n", code, msg);
    });

    if (!glfwInit()) {
        fprintf(stderr, "[devrt] glfwInit() failed\n");
        return 1;
    }

    {
    MorphWindow window(config.title, config.width, config.height, config.visible);
    if (!window.handle()) {
        fprintf(stderr, "[devrt] glfwCreateWindow failed — cannot create OpenGL window\n");
        fprintf(stderr, "[devrt] On Wayland, try: GDK_BACKEND=x11 GDK_DEBUG=x11-override\n");
        glfwTerminate();
        return 1;
    }
    window.addChild(rootNode);

    // ── DevTools ─────────────────────────────────────────────
    DevTools devtools;
    g_devtools = &devtools;
    glfwSetKeyCallback(window.handle(), keyCb);

    // ── Start compositor thread ─────────────────────────────
    window.startCompositor(true); // vsync on
    fprintf(stderr, "[devrt] compositor thread started\n");

    // ── Main loop ───────────────────────────────────────────
    fprintf(stderr, "[devrt] entering render loop\n");

    auto lastFrameTime = std::chrono::steady_clock::now();

    while (g_running && !window.shouldClose()) {
        // Process all pending GLFW events (non-blocking)
        glfwPollEvents();

        // Check for new IR from socket
        std::string msg;
        while (sock.readMessage(msg, 0)) {
            if (msg.empty()) continue;

            // Parse error messages from Python
            if (msg.find("__error__") != std::string::npos) {
                fprintf(stderr, "[devrt] error from client: %s\n", msg.c_str());
                continue;
            }

            // Parse new IR and rebuild tree
            JsonValue newRoot;
            try {
                newRoot = JsonValue::parse(msg);
            } catch (std::exception&) {
                fprintf(stderr, "[devrt] bad JSON from client\n");
                continue;
            }

            NodeRegistry newRegistry;
            std::vector<StateVarInfo> newStateVars;
            DevWindowConfig newConfig;
            MorphNode* newNode = nullptr;
            if (parseIR(newRoot, newNode, newConfig, newRegistry, newStateVars)) {
                // Apply window config (title only; resizing disrupts dev flow)
                window.setTitle(newConfig.title);
                // Replace node tree
                window.addChild(newNode);
                deleteNodeTree(rootNode);
                rootNode = newNode;
                registry = std::move(newRegistry);
                devtools.hoveredNode = nullptr; // tree changed, clear stale ref
                MorphWindow::clearHoverState(); // clear stale hover pointer

                // Reload logic .so with new tree but preserved signals
                // (signals in signalStore are NOT cleared — state survives)
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
                // Use .so path from IR if provided (ensures unique file = fresh dlopen)
                std::string reloadPath = logicSoPath;
                if (newRoot.has("logic_so_path")) {
                    reloadPath = newRoot["logic_so_path"].asString();
                }
                reload_logic(reloadPath, registry, signalStore);
                window.notifyPendingRender();
                fprintf(stderr, "[devrt] hot reloaded (so=%s)\n", reloadPath.c_str());
            } else {
                fprintf(stderr, "[devrt] failed to parse IR from client\n");
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
            window.commitFrame();
            window.renderFrame([&](GLRenderer& r, DirtyStats&) {
                devtools.render(r, (float)window.width(), (float)window.height(),
                                window.dirtyStats());
            });
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
    fprintf(stderr, "[devrt] done\n");
    return 0;
}

