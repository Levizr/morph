#pragma once
#include <string>

enum class EventType {
    Click, DoubleClick, MouseMove, MouseDown, MouseUp,
    KeyDown, KeyUp, Scroll, Resize, Focus, Blur,
    MouseEnter, MouseLeave, ContextMenu,
    PointerDown, PointerMove, PointerUp
};

struct MorphEvent {
    EventType type;
    float x = 0, y = 0;
    std::string key;
    std::string code;
    int   button = 0;
    int   buttons = 0;
    int   detail = 0;
    float scroll = 0;
    int   mods = 0;     // GLFW key modifier bitmask (Shift/Ctrl/Alt/Super)
    bool  repeat = false;
};
