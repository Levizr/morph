#pragma once
#include <string>

enum class EventType {
    Click, DoubleClick, MouseMove, MouseDown, MouseUp,
    KeyDown, KeyUp, Scroll, Resize, Focus, Blur,
    MouseEnter, MouseLeave
};

struct MorphEvent {
    EventType type;
    float x = 0, y = 0;
    std::string key;
    int   button = 0;
    float scroll = 0;
    int   mods = 0;     // GLFW key modifier bitmask (Shift/Ctrl/Alt/Super)
    bool  repeat = false;
};
