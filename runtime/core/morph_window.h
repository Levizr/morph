#pragma once
#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>
#include <string>
#include <vector>
#include "morph_node.h"
#include "gl_renderer.h"

class MorphWindow {
    std::string m_title;
    int m_width, m_height;
    bool m_visible;
    GLFWwindow* m_handle = nullptr;
    MorphNode* m_root = nullptr;
    GLRenderer m_renderer;

    static void ortho(float* m, float l, float r, float b, float t, float n, float f) {
        // column-major 4x4 orthographic projection matrix
        m[0]  = 2.0f / (r - l);  m[4]  = 0.0f;               m[8]  = 0.0f;               m[12] = -(r + l) / (r - l);
        m[1]  = 0.0f;               m[5]  = 2.0f / (t - b);  m[9]  = 0.0f;               m[13] = -(t + b) / (t - b);
        m[2]  = 0.0f;               m[6]  = 0.0f;               m[10] = -2.0f / (f - n);  m[14] = -(f + n) / (f - n);
        m[3]  = 0.0f;               m[7]  = 0.0f;               m[11] = 0.0f;               m[15] = 1.0f;
    }

public:
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
            glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
        }
    }

    void addChild(MorphNode* node) { m_root = node; }
    bool shouldClose() const { return m_handle && glfwWindowShouldClose(m_handle); }
    bool isVisible() const { return m_visible && m_handle && !glfwWindowShouldClose(m_handle); }

    void render() {
        if (!m_handle) return;
        glfwMakeContextCurrent(m_handle);

        glViewport(0, 0, m_width, m_height);

        float proj[16];
        ortho(proj, 0.0f, (float)m_width, (float)m_height, 0.0f, -1.0f, 1.0f);
        m_renderer.clear();

        if (m_root) {
            m_root->layout(m_width, m_height);
            m_root->draw(m_renderer);
        }

        m_renderer.flush(proj);
        glfwSwapBuffers(m_handle);
    }

    ~MorphWindow() {
        if (m_handle) glfwDestroyWindow(m_handle);
    }
};
