#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <csignal>
#include <unistd.h>
#include <chrono>

#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>

#include "../core/window.h"
#include "../core/window_manager.h"
#include "../core/node.h"
#include "../core/renderer.h"
#include "../render/gl_renderer.h"

#include "dev_socket.h"
#include "ir_deserializer.h"
#include "inspector.h"

static volatile bool g_running = true;
static void signalHandler(int) { g_running = false; }

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

static void mouseCb(GLFWwindow* win, int btn, int action, int) {
    if (!g_devtools || btn != GLFW_MOUSE_BUTTON_LEFT || action != GLFW_PRESS) return;
    int ww;
    glfwGetWindowSize(win, &ww, nullptr);
    double mx, my;
    glfwGetCursorPos(win, &mx, &my);
    g_devtools->handleClick((float)mx, (float)my, (float)ww);
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

    DevWindowConfig config;
    MorphNode* rootNode = nullptr;
    if (!parseIR(root, rootNode, config)) {
        fprintf(stderr, "[devrt] failed to parse IR\n");
        return 1;
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
    glfwSetMouseButtonCallback(window.handle(), mouseCb);

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

            DevWindowConfig newConfig;
            MorphNode* newNode = nullptr;
            if (parseIR(newRoot, newNode, newConfig)) {
                // Apply window config (title only; resizing disrupts dev flow)
                window.setTitle(newConfig.title);
                // Replace node tree
                window.addChild(newNode);
                deleteNodeTree(rootNode);
                rootNode = newNode;
                devtools.hoveredNode = nullptr; // tree changed, clear stale ref
                window.notifyPendingRender();
                fprintf(stderr, "[devrt] hot reloaded\n");
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

        // Update animations (advances running animations, marks dirty on change)
        if (rootNode) rootNode->update(dt);

        // Render current frame (only if there's actual work pending)
        if (window.isVisible()) {
            if (window.hasPendingRender()) {
                // Track mouse for devtools inspect
                if (devtools.inspecting) {
                    double mx, my;
                    glfwGetCursorPos(window.handle(), &mx, &my);
                    devtools.mouseX = (float)mx;
                    devtools.mouseY = (float)my;
                    devtools.updateHover(rootNode);
                }
                window.render([&](GLRenderer& r, DirtyStats& ds) {
                    devtools.render(r, (float)window.width(), (float)window.height(), ds);
                });
            } else {
                // Nothing changed — sleep until next event or ~16ms
                glfwWaitEventsTimeout(1.0 / 60.0);
            }
        } else {
            glfwWaitEventsTimeout(1.0 / 60.0);
        }
    }

    // ── Cleanup ─────────────────────────────────────────────
    deleteNodeTree(rootNode);
    // window destroyed here (before glfwTerminate)
    } // ~MorphWindow, ~DevTools

    glfwTerminate();
    fprintf(stderr, "[devrt] done\n");
    return 0;
}

