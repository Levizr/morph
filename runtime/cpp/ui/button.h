#pragma once
#include "../core/node.h"
#include "../core/event.h"
#include "radius.h"
#include "border_paint.h"

class ButtonNode : public MorphNode {
public:
    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float rad = m_isTransitioning ? (style.borderRadius > 0.0f ? style.borderRadius : 6.0f) : snapRadius(style.borderRadius > 0.0f ? style.borderRadius : 6.0f);
#ifdef MORPH_FEATURE_BORDER
        recordBoxOps(m_displayList, style, sx, sy, sw, sh, rad,
                     !m_isTransitioning, true);
#else
        DrawOp bg;
        {
            bg.setRounded(sx, sy, sw, sh, rad, style.bgColor);
        }
        m_displayList.push_back(bg);
#endif
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
                case DrawOp::Rect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,0.0f,&op.r,&style.bgGradient,0.0f,nullptr,nullptr);
                        break;
                    }
#endif
                    r.drawRect(op.x,op.y,op.w,op.h,&op.r); break;
                case DrawOp::RoundedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,op.data[0],&op.r,&style.bgGradient,0.0f,nullptr,nullptr);
                        break;
                    }
#endif
                    r.drawRoundedRect(op.x,op.y,op.w,op.h,op.data[0],&op.r); break;
                case DrawOp::BorderedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,0.0f,&op.r,&style.bgGradient,op.data[1],&op.br,nullptr);
                        break;
                    }
#endif
                    r.drawBorderedRect(op.x,op.y,op.w,op.h,&op.r,op.data[1],&op.br); break;
                case DrawOp::BorderedRoundedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,op.data[0],&op.r,&style.bgGradient,op.data[1],&op.br,nullptr);
                        break;
                    }
#endif
                    r.drawBorderedRoundedRect(op.x,op.y,op.w,op.h,op.data[0],&op.r,op.data[1],&op.br); break;
                case DrawOp::BorderRing:
#ifdef MORPH_FEATURE_BORDER
                    execBorderRingOp(r, op, style); break;
#else
                    break;
#endif
                default: break;
            }
        }

        // 2. Clip + scroll + children
        bool needRadiusClip = rad > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollEnabled && contentH > sh;
        bool needRectClip = scrolling || style.overflow == CSS::Overflow::Hidden || style.overflow == CSS::Overflow::Auto;
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
        paintBoxDirect(r, style, sx, sy, sw, sh, rad, !m_isTransitioning,
                       true);
#else
#ifdef MORPH_FEATURE_GRADIENT
        if (style.bgGradient.enabled) {
            r.drawGradRect(sx, sy, sw, sh, rad,
                           style.bgColor, &style.bgGradient, 0.0f, nullptr,
                           nullptr);
        } else {
#endif
            r.drawRoundedRect(sx, sy, sw, sh, rad, style.bgColor);
#ifdef MORPH_FEATURE_GRADIENT
        }
#endif
#endif
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
