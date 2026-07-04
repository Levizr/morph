#pragma once
#include "../core/node.h"
#include "morph_radius.h"

class RectNode : public MorphNode {
public:
    RectNode(float x, float y, float w, float h) {
        this->x = x; this->y = y;
        this->w = w; this->h = h;
    }

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();

        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);

        // Background rect + border (only rendering ops — no clip/scroll state)
        DrawOp bg;
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            if (style.boxSizing == "border-box") {
                bg.setBordered(sx, sy, sw, sh, rad, style.bgColor, bw, style.borderColor);
            } else {
                bg.setBordered(sx - bw, sy - bw, sw + 2.0f * bw, sh + 2.0f * bw,
                               rad, style.bgColor, bw, style.borderColor);
            }
        } else
#endif
#ifdef MORPH_FEATURE_RADIUS
        if (rad > 0.0f) {
            bg.setRounded(sx, sy, sw, sh, rad, style.bgColor);
        } else
#endif
        {
            bg.setRect(sx, sy, sw, sh, style.bgColor);
        }
        m_displayList.push_back(bg);
    }

    void executeDisplayList(Renderer& r) override {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);

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
        bool needRadiusClip = rad > 0.0f;
        bool scrolling = scrollEnabled && contentH > sh;

        if (needClip || needRadiusClip) {
            if (needClip) r.beginClip(sx, sy, sw, sh);
            if (needRadiusClip) r.beginRoundedClip(sx, sy, sw, sh, rad);
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
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            if (style.boxSizing == "border-box") {
                r.drawBorderedRoundedRect(sx, sy, sw, sh, rad,
                                          style.bgColor, bw, style.borderColor);
            } else {
                r.drawBorderedRoundedRect(sx - bw, sy - bw,
                                          sw + 2.0f * bw, sh + 2.0f * bw,
                                          rad,
                                          style.bgColor, bw, style.borderColor);
            }
        } else
#endif
#ifdef MORPH_FEATURE_RADIUS
        if (rad > 0.0f) {
            r.drawRoundedRect(sx, sy, sw, sh, rad, style.bgColor);
        } else
#endif
            r.drawRect(sx, sy, sw, sh, style.bgColor);

        // ── 2. Children (clipped when overflow is non-visible) ────
        bool overflowClipped = (style.overflow == "hidden" ||
                                style.overflow == "scroll" ||
                                style.overflow == "auto");
        bool needRectClip = overflowClipped;
        bool needRadiusClip = rad > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollEnabled && contentH > sh;
#else
        bool scrolling = false;
#endif

        if (needRectClip || needRadiusClip) {
            if (needRectClip) r.beginClip(sx, sy, sw, sh);
            if (needRadiusClip) r.beginRoundedClip(sx, sy, sw, sh, rad);

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
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float sbw = m_isTransitioning ? style.scrollbarWidth : snapBorderWidth(style.scrollbarWidth);
        float trackX = sx + sw - sbw;
        r.drawRect(trackX, sy, sbw, sh, style.scrollbarTrackColor);
        float thumbH = sc((sh / contentH) * sh);
        float thumbY = sy + sc((scrollY / (contentH - sh)) * (sh - thumbH));
        if (thumbY < sy) thumbY = sy;
        if (thumbY + thumbH > sy + sh) thumbY = sy + sh - thumbH;
        float radius = m_isTransitioning ? style.scrollbarBorderRadius : snapRadius(style.scrollbarBorderRadius);
        if (radius > thumbH * 0.5f) radius = thumbH * 0.5f;
        if (radius < 0.5f) radius = 0.5f;
        r.drawRoundedRect(trackX, thumbY, sbw, thumbH, radius, style.scrollbarThumbColor);
    }
#endif
};
