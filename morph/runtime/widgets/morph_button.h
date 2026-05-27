#pragma once
#include "../core/node.h"
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

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        float rad = style.borderRadius > 0.0f ? style.borderRadius : 6.0f;
        DrawOp bg;
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = style.borderWidth;
            bool inner = (style.boxSizing == "border-box");
            if (inner)
                bg.setBordered(x, y, w, h, rad, style.bgColor, bw, style.borderColor);
            else
                bg.setBordered(x - bw, y - bw, w + 2.0f * bw, h + 2.0f * bw,
                               rad, style.bgColor, bw, style.borderColor);
        } else
#endif
        {
            bg.setRounded(x, y, w, h, rad, style.bgColor);
        }
        m_displayList.push_back(bg);
    }

    void executeDisplayList(Renderer& r) override {
        // 1. Render self (background)
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::Rect: r.drawRect(op.x,op.y,op.w,op.h,&op.r); break;
                case DrawOp::RoundedRect: r.drawRoundedRect(op.x,op.y,op.w,op.h,op.data[0],&op.r); break;
                case DrawOp::BorderedRect: r.drawBorderedRect(op.x,op.y,op.w,op.h,&op.r,op.data[1],&op.br); break;
                case DrawOp::BorderedRoundedRect: r.drawBorderedRoundedRect(op.x,op.y,op.w,op.h,op.data[0],&op.r,op.data[1],&op.br); break;
                default: break;
            }
        }

        // 2. Clip + scroll + children
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollEnabled && contentH > h;
        if (scrolling) {
            r.beginClip(x, y, w, h);
            r.pushScrollOffset(0, -scrollY);
        }
#endif
        for (auto* child : children)
            child->executeDisplayList(r);
#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) {
            r.popScrollOffset(0, -scrollY);
            r.endClip();
            drawScrollbar(r);
        }
#endif
    }

    void draw(Renderer& r) override {
        float rad = style.borderRadius > 0.0f ? style.borderRadius : 6.0f;
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = style.borderWidth;
            bool inner = (style.boxSizing == "border-box");
            if (inner)
                r.drawBorderedRoundedRect(x, y, w, h, rad, style.bgColor,
                                          bw, style.borderColor);
            else
                r.drawBorderedRoundedRect(x - bw, y - bw,
                                          w + 2.0f * bw, h + 2.0f * bw,
                                          rad, style.bgColor,
                                          bw, style.borderColor);
        } else
#endif
            r.drawRoundedRect(x, y, w, h, rad, style.bgColor);
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
