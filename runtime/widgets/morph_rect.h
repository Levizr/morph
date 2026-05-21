#pragma once
#include "../core/morph_node.h"
#include "../core/renderer.h"

class RectNode : public MorphNode {
public:
    RectNode(float x, float y, float w, float h) {
        this->x = x; this->y = y;
        this->w = w; this->h = h;
    }

    void draw(Renderer& r) override {
        if (style.borderRadius > 0.0f)
            r.drawRoundedRect(x, y, w, h, style.borderRadius, style.bgColor);
        else
            r.drawRect(x, y, w, h, style.bgColor);
        for (auto* child : children)
            child->draw(r);
    }
};
