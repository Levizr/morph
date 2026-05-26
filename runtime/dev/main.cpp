#include <cstdio>
#include <cstring>
#include <cstdlib>
#include <csignal>
#include <unistd.h>

#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>

#include "../core/window.h"
#include "../core/window_manager.h"
#include "../core/node.h"
#include "../core/renderer.h"
#include "../render/gl_renderer.h"

#include "dev_socket.h"
#include "ir_deserializer.h"

static volatile bool g_running = true;
static void signalHandler(int) { g_running = false; }

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

    MorphWindow window(config.title, config.width, config.height, config.visible);
    window.addChild(rootNode);

    // ── Main loop ───────────────────────────────────────────
    fprintf(stderr, "[devrt] entering render loop\n");

    while (g_running && !window.shouldClose()) {
        // Poll GLFW events (covers mouse, keyboard, resize)
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
                fprintf(stderr, "[devrt] hot reloaded\n");
            } else {
                fprintf(stderr, "[devrt] failed to parse IR from client\n");
            }
        }

        // Re-accept client if disconnected
        if (!sock.isConnected()) {
            sock.acceptClient();
        }

        // Render current frame
        if (window.isVisible()) {
            window.render();
        }
    }

    // ── Cleanup ─────────────────────────────────────────────
    deleteNodeTree(rootNode);
    glfwTerminate();
    fprintf(stderr, "[devrt] done\n");
    return 0;
}
