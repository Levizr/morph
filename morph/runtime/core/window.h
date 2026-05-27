#pragma once
#include <functional>
#include "vendor/glad/glad.h"
#include <GLFW/glfw3.h>
#include <string>
#include "node.h"
#include "../render/gl_renderer.h"
#include "event.h"

class MorphWindow {
    std::string m_title;
    int m_width, m_height;
    bool m_visible;
    GLFWwindow* m_handle = nullptr;
    MorphNode* m_root = nullptr;
    GLRenderer m_renderer;
#ifdef MORPH_FEATURE_CURSOR
    GLFWcursor* m_handCursor = nullptr;
    GLFWcursor* m_textCursor = nullptr;
#endif

    static void ortho(float* m, float l, float r, float b, float t, float n, float f) {
        m[0]  = 2.0f / (r - l);  m[4]  = 0.0f;               m[8]  = 0.0f;               m[12] = -(r + l) / (r - l);
        m[1]  = 0.0f;               m[5]  = 2.0f / (t - b);  m[9]  = 0.0f;               m[13] = -(t + b) / (t - b);
        m[2]  = 0.0f;               m[6]  = 0.0f;               m[10] = -2.0f / (f - n);  m[14] = -(f + n) / (f - n);
        m[3]  = 0.0f;               m[7]  = 0.0f;               m[11] = 0.0f;               m[15] = 1.0f;
    }

    static void mouseButtonCb(GLFWwindow* win, int btn, int act, int mods);
    static void cursorPosCb(GLFWwindow* win, double mx, double my);
    static void windowSizeCb(GLFWwindow* win, int width, int height);
    static void scrollCb(GLFWwindow* win, double dx, double dy);

public:
    MorphWindow(const std::string& title, int width, int height, bool visible = true);
    ~MorphWindow();

    void addChild(MorphNode* node) { m_root = node; }
    void setTitle(const std::string& title);
    void setSize(int width, int height);
    int width() const { return m_width; }
    int height() const { return m_height; }
    GLFWwindow* handle() const { return m_handle; }
    bool shouldClose() const { return m_handle && glfwWindowShouldClose(m_handle); }
    bool isVisible() const { return m_visible && m_handle && !glfwWindowShouldClose(m_handle); }
    void render(std::function<void(GLRenderer&)> overlayFn = {});
};
