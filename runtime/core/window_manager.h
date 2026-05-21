#pragma once
#include <unordered_map>
#include <string>
#include <GLFW/glfw3.h>

class MorphWindow;

class WindowManager {
    std::unordered_map<std::string, MorphWindow*> m_windows;

public:
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
        // TODO: hide window
    }

    void navigate(const std::string& windowId, const std::string& pageId) {
        // TODO: page navigation
    }

    bool allClosed() const {
        if (m_windows.empty()) return true;
        return false;
    }
};
