#pragma once
#include <string>
#include "../core/morph_node.h"
#include "../core/renderer.h"

class TextNode : public MorphNode {
public:
    std::string text;

    TextNode(const std::string& text) : text(text) {}

    void layout(float px, float py, float parentW, float parentH) override {
        MorphNode::layout(px, py, parentW, parentH);
        if (h == 0.0f)
            h = style.fontSize * 1.4f;
    }

    void draw(Renderer& r) override {
        r.drawText(text, x, y, style.color, TextAlign::Left, style.fontSize, style.fontWeight);
        for (auto* child : children)
            child->draw(r);
    }
};
