#pragma once
#include "../core/node.h"
#include "../core/renderer.h"

class RectNode : public MorphNode {
public:
    RectNode(float x, float y, float w, float h) {
        this->x = x; this->y = y;
        this->w = w; this->h = h;
    }

    void draw(Renderer& r) override {
#ifdef MORPH_FEATURE_RADIUS
        if (style.borderRadius > 0.0f) {
            r.drawRoundedRect(x, y, w, h, style.borderRadius, style.bgColor);
        } else
#endif
            r.drawRect(x, y, w, h, style.bgColor);

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
        float trackH = h;
        r.drawRect(trackX, y, sw, trackH, style.scrollbarTrackColor);

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
