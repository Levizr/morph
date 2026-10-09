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
        float radii[4];
        resolvedControlRadii(radii, !m_isTransitioning);
#ifdef MORPH_FEATURE_BORDER
        recordBoxOps(m_displayList, style, unitEnv(w, nullptr), sx, sy, sw, sh,
                     !m_isTransitioning, true);
#else
        DrawOp bg;
        {
            bg.setRounded(sx, sy, sw, sh, radii, style.bgColor);
        }
        m_displayList.push_back(bg);
#endif
    }

    void executeDisplayList(Renderer& r) override {
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float radii[4];
        resolvedRadii(radii, !m_isTransitioning);

#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif

        // 1. Render self (background)
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::Rect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,kSharpRadii,&op.r,&style.bgGradient,0.0f,nullptr,nullptr);
                        break;
                    }
#endif
                    r.drawRect(op.x,op.y,op.w,op.h,&op.r); break;
                case DrawOp::RoundedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,op.radii,&op.r,&style.bgGradient,0.0f,nullptr,nullptr);
                        break;
                    }
#endif
                    r.drawRoundedRect(op.x,op.y,op.w,op.h,op.radii,&op.r); break;
                case DrawOp::BorderedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,kSharpRadii,&op.r,&style.bgGradient,op.data[1],&op.br,nullptr);
                        break;
                    }
#endif
                    r.drawBorderedRect(op.x,op.y,op.w,op.h,&op.r,op.data[1],&op.br); break;
                case DrawOp::BorderedRoundedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x,op.y,op.w,op.h,op.radii,&op.r,&style.bgGradient,op.data[1],&op.br,nullptr);
                        break;
                    }
#endif
                    r.drawBorderedRoundedRect(op.x,op.y,op.w,op.h,op.radii,&op.r,op.data[1],&op.br); break;
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
        bool needRadiusClip = maxRadius4(radii) > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollsVertically(sh) || scrollsHorizontally(sw);
        bool needRectClip = scrolling || clipsOverflowBox();
#else
        bool needRectClip = false;
#endif

        if (needRectClip || needRadiusClip) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (needRectClip) {
                if (pushedSelf)
                    r.beginRoundedClip(sx, sy, sw, sh, kSharpRadii);
                else
                    r.beginClip(sx, sy, sw, sh);
            }
#else
            if (needRectClip) r.beginClip(sx, sy, sw, sh);
#endif
            if (needRadiusClip) r.beginRoundedClip(sx, sy, sw, sh, radii);
#ifdef MORPH_FEATURE_SCROLL
            if (scrolling) r.pushScrollOffset(-effScrollX(sw), -effScrollY(sh));
#endif
            for (auto* child : paintOrder())
                child->executeDisplayList(r);
#ifdef MORPH_FEATURE_SCROLL
            if (scrolling) r.popScrollOffset(-effScrollX(sw), -effScrollY(sh));
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
        float radii[4];
        resolvedControlRadii(radii, !m_isTransitioning);
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif
#ifdef MORPH_FEATURE_BORDER
        paintBoxDirect(r, style, unitEnv(w, nullptr), sx, sy, sw, sh, !m_isTransitioning,
                       true);
#else
#ifdef MORPH_FEATURE_GRADIENT
        if (style.bgGradient.enabled) {
            r.drawGradRect(sx, sy, sw, sh, radii,
                           style.bgColor, &style.bgGradient, 0.0f, nullptr,
                           nullptr);
        } else {
#endif
            r.drawRoundedRect(sx, sy, sw, sh, radii, style.bgColor);
#ifdef MORPH_FEATURE_GRADIENT
        }
#endif
#endif
#ifdef MORPH_FEATURE_SCROLL
        if (scrollsVertically(sh) || scrollsHorizontally(sw)) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (pushedSelf)
                r.beginRoundedClip(sx, sy, sw, sh, kSharpRadii);
            else
                r.beginClip(sx, sy, sw, sh);
#else
            r.beginClip(sx, sy, sw, sh);
#endif
            r.pushScrollOffset(-effScrollX(sw), -effScrollY(sh));
            for (auto* child : paintOrder()) {
                float childVisX = child->x - effScrollX(sw);
                float childVisY = child->y - effScrollY(sh);
                if (childVisX + child->w > sx && childVisX < sx + sw &&
                    childVisY + child->h > sy && childVisY < sy + sh)
                    child->draw(r);
            }
            r.popScrollOffset(-effScrollX(sw), -effScrollY(sh));
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
        float radius = m_isTransitioning ? style.scrollbarBorderRadius : snapRadius(style.scrollbarBorderRadius);
        float sbRadii[4];
        bool vBar = scrollsVertically(sh);
        if (vBar) {
            float trackX = sx + sw - sbw;
            r.drawRect(trackX, sy, sbw, sh, style.scrollbarTrackColor);
            float tp, ts;
            vScrollThumb(sh, &tp, &ts);
            float thumbH = sc(ts);
            float thumbY = sy + sc(tp);
            if (thumbY < sy) thumbY = sy;
            if (thumbY + thumbH > sy + sh) thumbY = sy + sh - thumbH;
            float tr = radius;
            if (tr > thumbH * 0.5f) tr = thumbH * 0.5f;
            if (tr < 0.5f) tr = 0.5f;
            fillRadii(sbRadii, tr);
            r.drawRoundedRect(trackX, thumbY, sbw, thumbH, sbRadii, style.scrollbarThumbColor);
        }
        if (scrollsHorizontally(sw)) {
            float trackW = sw - (vBar ? sbw : 0.0f);
            float trackY = sy + sh - sbw;
            r.drawRect(sx, trackY, trackW, sbw, style.scrollbarTrackColor);
            float tp, ts;
            hScrollThumb(trackW, &tp, &ts);
            float thumbW = sc(ts);
            float thumbX = sx + sc(tp);
            if (thumbX < sx) thumbX = sx;
            if (thumbX + thumbW > sx + trackW) thumbX = sx + trackW - thumbW;
            float tr = radius;
            if (tr > thumbW * 0.5f) tr = thumbW * 0.5f;
            if (tr < 0.5f) tr = 0.5f;
            fillRadii(sbRadii, tr);
            r.drawRoundedRect(thumbX, trackY, thumbW, sbw, sbRadii, style.scrollbarThumbColor);
        }
    }
#endif
};
