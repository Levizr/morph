#pragma once
#include "vendor/glad/glad.h"
#include <unordered_map>
#include <string>
#include <GLFW/glfw3.h>

#include "window.h"

class WindowManager {
    std::unordered_map<std::string, MorphWindow*> m_windows;

public:
    ~WindowManager() {
        for (auto& [_, w] : m_windows) delete w;
        m_windows.clear();
    }

    static WindowManager& get() {
        static WindowManager inst;
        return inst;
    }

    void registerWindow(const std::string& id, MorphWindow* w) {
        m_windows[id] = w;
    }

    void open(const std::string& id) {
        // TODO: show window
    }

    void close(const std::string& id) {
        auto it = m_windows.find(id);
        if (it != m_windows.end()) {
            delete it->second;
            m_windows.erase(it);
        }
    }

    void navigate(const std::string& windowId, const std::string& pageId) {
        // TODO: page navigation
    }

    bool allClosed() const {
        if (m_windows.empty()) return true;
        for (const auto& [_, w] : m_windows) {
            if (!w->shouldClose()) return false;
        }
        return true;
    }
};
