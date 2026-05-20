#pragma once
#include <string>

enum class EventType {
    Click, MouseMove, MouseDown, MouseUp,
    KeyDown, KeyUp, Scroll, Resize, Focus, Blur
};

struct MorphEvent {
    EventType type;
    float x = 0, y = 0;
    int   key = 0;
    int   button = 0;
    float scroll = 0;
};
