#pragma once
#include "../core/morph_node.h"
#include "../core/renderer.h"
#include "../core/event.h"
#include <functional>

class ButtonNode : public MorphNode {
public:
    std::function<void()> onClick;

    bool onEvent(MorphEvent& e) override {
        if (e.type == EventType::Click && onClick) {
            onClick();
            return true;
        }
        return false;
    }

    void draw(Renderer& r) override {
        r.drawRoundedRect(x, y, w, h, 6.0f, style.bgColor);
#ifdef MORPH_FEATURE_SCROLL
        if (scrollEnabled && contentH > h) {
            r.beginClip(x, y, w, h);
            r.pushScrollOffset(0, -scrollY);
            for (auto* child : children) {
                float childVisY = child->y - scrollY;
                if (childVisY + child->h > y && childVisY < y + h)
                    child->draw(r);
            }
            r.popScrollOffset(0, -scrollY);
            r.endClip();
            drawScrollbar(r);
        } else
#endif
        {
            for (auto* child : children)
                child->draw(r);
        }
    }

#ifdef MORPH_FEATURE_SCROLL
    void drawScrollbar(Renderer& r) {
        float sw = style.scrollbarWidth;
        float trackX = x + w - sw;
        r.drawRect(trackX, y, sw, h, style.scrollbarTrackColor);
        float thumbH = (h / contentH) * h;
        float thumbY = y + (scrollY / (contentH - h)) * (h - thumbH);
        if (thumbY < y) thumbY = y;
        if (thumbY + thumbH > y + h) thumbY = y + h - thumbH;
        float radius = style.scrollbarBorderRadius;
        if (radius > thumbH * 0.5f) radius = thumbH * 0.5f;
        if (radius < 0.5f) radius = 0.5f;
        r.drawRoundedRect(trackX, thumbY, sw, thumbH, radius, style.scrollbarThumbColor);
    }
#endif
};
