#pragma once
#include <unordered_map>
#include <string>

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

    void open(const std::string& id);
    void close(const std::string& id);
    void navigate(const std::string& windowId, const std::string& pageId);

    bool allClosed() const;
};
