#pragma once
#include <thread>
#include <atomic>
#include <GLFW/glfw3.h>
#include "render_frame.h"

class Compositor {
public:
    Compositor(GLFWwindow* window, int fbWidth, int fbHeight);
    ~Compositor();

    void start();
    void stop();
    void setVSync(bool on) { m_vsync = on; }
    void setFBSize(int w, int h) { m_fbWidth = w; m_fbHeight = h; }

private:
    void run();

    GLFWwindow* m_window;
    int m_fbWidth, m_fbHeight;
    bool m_vsync = true;
    std::thread m_thread;
    std::atomic<bool> m_running{false};

    double getTime() const;
};
