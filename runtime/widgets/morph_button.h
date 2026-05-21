#pragma once
#include "../core/morph_node.h"
#include "../core/renderer.h"
#include "../core/event.h"
#include <functional>

class ButtonNode : public MorphNode {
public:
    std::function<void()> onClick;

    void onEvent(MorphEvent& e) override {
        if (e.type == EventType::Click && onClick)
            onClick();
    }

    void draw(Renderer& r) override {
        r.drawRoundedRect(x, y, w, h, 6.0f, style.bgColor);
    }
};
