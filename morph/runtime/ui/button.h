#pragma once
#include "../core/node.h"
#include "../core/event.h"
#include "radius.h"

class ButtonNode : public MorphNode {
public:
    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? (style.borderRadius > 0.0f ? style.borderRadius : 6.0f) : snapRadius(style.borderRadius > 0.0f ? style.borderRadius : 6.0f);
        DrawOp bg;
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            bool inner = (style.boxSizing == "border-box");
            if (inner)
                bg.setBordered(sx, sy, sw, sh, rad, style.bgColor, bw, style.borderColor);
            else
                bg.setBordered(sx - bw, sy - bw, sw + 2.0f * bw, sh + 2.0f * bw,
                               rad, style.bgColor, bw, style.borderColor);
        } else
#endif
        {
            bg.setRounded(sx, sy, sw, sh, rad, style.bgColor);
        }
        m_displayList.push_back(bg);
    }

    void executeDisplayList(Renderer& r) override {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);

#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif

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
        bool needRadiusClip = rad > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollEnabled && contentH > sh;
        bool needRectClip = scrolling || style.overflow == "hidden" || style.overflow == "auto";
#else
        bool needRectClip = false;
#endif

        if (needRectClip || needRadiusClip) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (needRectClip) {
                if (pushedSelf)
                    r.beginRoundedClip(sx, sy, sw, sh, 0.0f);
                else
                    r.beginClip(sx, sy, sw, sh);
            }
#else
            if (needRectClip) r.beginClip(sx, sy, sw, sh);
#endif
            if (needRadiusClip) r.beginRoundedClip(sx, sy, sw, sh, rad);
#ifdef MORPH_FEATURE_SCROLL
            if (scrolling) r.pushScrollOffset(0, -scrollY);
#endif
            for (auto* child : paintOrder())
                child->executeDisplayList(r);
#ifdef MORPH_FEATURE_SCROLL
            if (scrolling) r.popScrollOffset(0, -scrollY);
#endif
            if (needRadiusClip) r.endRoundedClip();
            if (needRectClip) r.endClip();
        } else {
            for (auto* child : paintOrder())
                child->executeDisplayList(r);
        }

#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) drawScrollbar(r);
#endif

#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

    void draw(Renderer& r) override {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? (style.borderRadius > 0.0f ? style.borderRadius : 6.0f) : snapRadius(style.borderRadius > 0.0f ? style.borderRadius : 6.0f);
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif
#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            bool inner = (style.boxSizing == "border-box");
            if (inner)
                r.drawBorderedRoundedRect(sx, sy, sw, sh, rad, style.bgColor,
                                          bw, style.borderColor);
            else
                r.drawBorderedRoundedRect(sx - bw, sy - bw,
                                          sw + 2.0f * bw, sh + 2.0f * bw,
                                          rad, style.bgColor,
                                          bw, style.borderColor);
        } else
#endif
            r.drawRoundedRect(sx, sy, sw, sh, rad, style.bgColor);
#ifdef MORPH_FEATURE_SCROLL
        if (scrollEnabled && contentH > sh) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (pushedSelf)
                r.beginRoundedClip(sx, sy, sw, sh, 0.0f);
            else
                r.beginClip(sx, sy, sw, sh);
#else
            r.beginClip(sx, sy, sw, sh);
#endif
            r.pushScrollOffset(0, -scrollY);
            for (auto* child : paintOrder()) {
                float childVisY = child->y - scrollY;
                if (childVisY + child->h > sy && childVisY < sy + sh)
                    child->draw(r);
            }
            r.popScrollOffset(0, -scrollY);
            r.endClip();
            drawScrollbar(r);
        } else
#endif
        {
            for (auto* child : paintOrder())
                child->draw(r);
        }

#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
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
