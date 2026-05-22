#pragma once
#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>
#include <string>
#include <vector>
#include "morph_node.h"
#include "gl_renderer.h"
#include "event.h"

class MorphWindow {
    std::string m_title;
    int m_width, m_height;
    bool m_visible;
    GLFWwindow* m_handle = nullptr;
    MorphNode* m_root = nullptr;
    GLRenderer m_renderer;
    GLFWcursor* m_handCursor = nullptr;
    GLFWcursor* m_textCursor = nullptr;

    static void ortho(float* m, float l, float r, float b, float t, float n, float f) {
        // column-major 4x4 orthographic projection matrix
        m[0]  = 2.0f / (r - l);  m[4]  = 0.0f;               m[8]  = 0.0f;               m[12] = -(r + l) / (r - l);
        m[1]  = 0.0f;               m[5]  = 2.0f / (t - b);  m[9]  = 0.0f;               m[13] = -(t + b) / (t - b);
        m[2]  = 0.0f;               m[6]  = 0.0f;               m[10] = -2.0f / (f - n);  m[14] = -(f + n) / (f - n);
        m[3]  = 0.0f;               m[7]  = 0.0f;               m[11] = 0.0f;               m[15] = 1.0f;
    }

public:
    static void mouseButtonCb(GLFWwindow* win, int btn, int act, int mods) {
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
        // Also dispatch Click on press
        if (act == GLFW_PRESS) {
            e.type = EventType::Click;
            self->m_root->dispatchEvent(e, (float)mx, (float)my);
        }
    }

    static void cursorPosCb(GLFWwindow* win, double mx, double my) {
        auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
        if (!self || !self->m_root) return;
        MorphEvent e;
        e.type = EventType::MouseMove;
        e.x = (float)mx;
        e.y = (float)my;
        self->m_root->dispatchEvent(e, (float)mx, (float)my);

        // Update cursor based on hovered element
        auto* target = self->m_root->hitTest((float)mx, (float)my);
        if (target && target->style.cursor == "pointer") {
            glfwSetCursor(win, self->m_handCursor);
        } else if (target && target->style.cursor == "text") {
            glfwSetCursor(win, self->m_textCursor);
        } else {
            glfwSetCursor(win, nullptr);
        }
    }

    static void windowSizeCb(GLFWwindow* win, int width, int height) {
        auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
        if (!self) return;
        self->m_width = width;
        self->m_height = height;
    }

    static void scrollCb(GLFWwindow* win, double dx, double dy) {
        auto* self = (MorphWindow*)glfwGetWindowUserPointer(win);
        if (!self || !self->m_root) return;
        double mx, my;
        glfwGetCursorPos(win, &mx, &my);
        MorphEvent e;
        e.type = EventType::Scroll;
        e.scroll = (float)dy;
        e.x = (float)mx;
        e.y = (float)my;
        // Dispatch from root so scroll bubbles down to the hovered scroll container
        self->m_root->dispatchEvent(e, (float)mx, (float)my);
    }

    MorphWindow(const std::string& title, int width, int height, bool visible = true)
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
            m_handCursor = glfwCreateStandardCursor(GLFW_HAND_CURSOR);
            m_textCursor = glfwCreateStandardCursor(GLFW_IBEAM_CURSOR);
        }
    }

    void addChild(MorphNode* node) { m_root = node; }
    bool shouldClose() const { return m_handle && glfwWindowShouldClose(m_handle); }
    bool isVisible() const { return m_visible && m_handle && !glfwWindowShouldClose(m_handle); }

    void render() {
        if (!m_handle) return;
        glfwMakeContextCurrent(m_handle);

        glViewport(0, 0, m_width, m_height);
        m_renderer.setFBHeight(m_height);

        float proj[16];
        ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);
        m_renderer.clear();

        if (m_root) {
            m_root->layout(0.0f, 0.0f, (float)m_width, (float)m_height, &m_renderer);
            m_root->draw(m_renderer);
        }

        m_renderer.flush(proj);
        glfwSwapBuffers(m_handle);
    }

    ~MorphWindow() {
        if (m_handCursor) glfwDestroyCursor(m_handCursor);
        if (m_textCursor) glfwDestroyCursor(m_textCursor);
        if (m_handle) glfwDestroyWindow(m_handle);
    }
};
