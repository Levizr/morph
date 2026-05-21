#pragma once
#include <GLFW/glfw3.h>
#include <string>
#include <vector>
#include "morph_node.h"

class MorphWindow {
    std::string m_title;
    int m_width, m_height;
    bool m_visible;
    GLFWwindow* m_handle = nullptr;
    MorphNode* m_root = nullptr;

public:
    MorphWindow(const std::string& title, int width, int height, bool visible = true)
        : m_title(title), m_width(width), m_height(height), m_visible(visible) {
        glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
        glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 3);
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
        // TODO: actual rendering
        glfwSwapBuffers(m_handle);
    }

    ~MorphWindow() {
        if (m_handle) glfwDestroyWindow(m_handle);
    }
};
