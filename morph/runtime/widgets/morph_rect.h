#pragma once
#include "../core/node.h"

class RectNode : public MorphNode {
public:
    RectNode(float x, float y, float w, float h) {
        this->x = x; this->y = y;
        this->w = w; this->h = h;
    }

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();

        // Background rect + border (only rendering ops — no clip/scroll state)
        DrawOp bg;
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = style.borderWidth;
            if (style.boxSizing == "border-box") {
                bg.setBordered(x, y, w, h, style.borderRadius, style.bgColor, bw, style.borderColor);
            } else {
                bg.setBordered(x - bw, y - bw, w + 2.0f * bw, h + 2.0f * bw,
                               style.borderRadius, style.bgColor, bw, style.borderColor);
            }
        } else
#endif
#ifdef MORPH_FEATURE_RADIUS
        if (style.borderRadius > 0.0f) {
            bg.setRounded(x, y, w, h, style.borderRadius, style.bgColor);
        } else
#endif
        {
            bg.setRect(x, y, w, h, style.bgColor);
        }
        m_displayList.push_back(bg);
    }

    void executeDisplayList(Renderer& r) override {
        // 1. Render self (background rect — from display list)
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::Rect:
                    r.drawRect(op.x, op.y, op.w, op.h, &op.r); break;
                case DrawOp::RoundedRect:
                    r.drawRoundedRect(op.x, op.y, op.w, op.h, op.data[0], &op.r); break;
                case DrawOp::BorderedRect:
                    r.drawBorderedRect(op.x, op.y, op.w, op.h, &op.r, op.data[1], &op.br); break;
                case DrawOp::BorderedRoundedRect:
                    r.drawBorderedRoundedRect(op.x, op.y, op.w, op.h, op.data[0], &op.r, op.data[1], &op.br); break;
                case DrawOp::BorderRing:
                    r.drawBorderRing(op.x, op.y, op.w, op.h, op.data[0], op.data[1], &op.br); break;
                default: break;
            }
        }

        // 2. Clip setup (from node state — correct interleaving)
        bool needClip = (style.overflow == "hidden" || style.overflow == "scroll" || style.overflow == "auto");
        bool needRadiusClip = style.borderRadius > 0.0f;
        bool scrolling = scrollEnabled && contentH > h;

        if (needClip || needRadiusClip) {
            if (needClip) r.beginClip(x, y, w, h);
            if (needRadiusClip) r.beginRoundedClip(x, y, w, h, style.borderRadius);
        }

        // 3. Scroll + children
        if (scrolling) r.pushScrollOffset(0, -scrollY);
        for (auto* child : children) {
            if (scrolling) {
                float childVisY = child->y - scrollY;
                if (childVisY + child->h > y && childVisY < y + h)
                    child->executeDisplayList(r);
            } else {
                child->executeDisplayList(r);
            }
        }
        if (scrolling) r.popScrollOffset(0, -scrollY);

        // 4. Clip teardown
        if (needClip || needRadiusClip) {
            if (needRadiusClip) r.endRoundedClip();
            if (needClip) r.endClip();
        }

        // 5. Scrollbar
#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) drawScrollbar(r);
#endif
    }

    void draw(Renderer& r) override {
        // ── 1. Draw self background + border ──────────────────────
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = style.borderWidth;
            if (style.boxSizing == "border-box") {
                r.drawBorderedRoundedRect(x, y, w, h, style.borderRadius,
                                          style.bgColor, bw, style.borderColor);
            } else {
                r.drawBorderedRoundedRect(x - bw, y - bw,
                                          w + 2.0f * bw, h + 2.0f * bw,
                                          style.borderRadius,
                                          style.bgColor, bw, style.borderColor);
            }
        } else
#endif
#ifdef MORPH_FEATURE_RADIUS
        if (style.borderRadius > 0.0f) {
            r.drawRoundedRect(x, y, w, h, style.borderRadius, style.bgColor);
        } else
#endif
            r.drawRect(x, y, w, h, style.bgColor);

        // ── 2. Children (clipped when overflow is non-visible) ────
        bool overflowClipped = (style.overflow == "hidden" ||
                                style.overflow == "scroll" ||
                                style.overflow == "auto");
        bool needRectClip = overflowClipped;
        bool needRadiusClip = style.borderRadius > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollEnabled && contentH > h;
#else
        bool scrolling = false;
#endif

        if (needRectClip || needRadiusClip) {
            if (needRectClip) r.beginClip(x, y, w, h);
            if (needRadiusClip) r.beginRoundedClip(x, y, w, h, style.borderRadius);

            r.pushScrollOffset(0, -scrollY);
            for (auto* child : children) {
                if (scrolling) {
                    float childVisY = child->y - scrollY;
                    if (childVisY + child->h > y && childVisY < y + h)
                        child->draw(r);
                } else {
                    child->draw(r);
                }
            }
            r.popScrollOffset(0, -scrollY);

            if (needRadiusClip) r.endRoundedClip();
            if (needRectClip) r.endClip();
        } else {
            for (auto* child : children)
                child->draw(r);
        }

        // ── 3. Scrollbar ──────────────────────────────────────────
#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) {
            drawScrollbar(r);
        }
#endif
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
