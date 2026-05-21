#pragma once
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

public:
    MorphWindow(const std::string& title, int width, int height, bool visible = true)
        : m_title(title), m_width(width), m_height(height), m_visible(visible) {
        glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 2);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 1);
        m_handle = glfwCreateWindow(width, height, title.c_str(), nullptr, nullptr);
        if (m_handle) {
            glfwMakeContextCurrent(m_handle);
            glfwSetWindowUserPointer(m_handle, this);
        }
    }

    void addChild(MorphNode* node) { m_root = node; }
    bool isVisible() const { return m_visible && m_handle && !glfwWindowShouldClose(m_handle); }

    void render() {
        if (!m_handle) return;
        glfwMakeContextCurrent(m_handle);

        // Orthographic projection for 2D rendering
        glViewport(0, 0, m_width, m_height);
        glMatrixMode(GL_PROJECTION);
        glLoadIdentity();
        glOrtho(0.0, m_width, m_height, 0.0, -1.0, 1.0);
        glMatrixMode(GL_MODELVIEW);
        glLoadIdentity();

        m_renderer.clear();

        if (m_root) {
            m_root->layout(m_width, m_height);
            m_root->draw(m_renderer);
        }

        glfwSwapBuffers(m_handle);
    }

    ~MorphWindow() {
        if (m_handle) glfwDestroyWindow(m_handle);
    }
};
