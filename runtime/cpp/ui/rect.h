#pragma once
#include "../core/node.h"
#include "radius.h"
#include "border_paint.h"

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
        float radii[4];
        resolvedRadii(radii, !m_isTransitioning);

        // Background rect + border (only rendering ops — no clip/scroll state)
#ifdef MORPH_FEATURE_BORDER
        recordBoxOps(m_displayList, style, unitEnv(w, nullptr), sx, sy, sw, sh,
                     !m_isTransitioning, false);
#else
        DrawOp bg;
#ifdef MORPH_FEATURE_RADIUS
        if (maxRadius4(radii) > 0.0f) {
            bg.setRounded(sx, sy, sw, sh, radii, style.bgColor);
        } else
#endif
        {
            bg.setRect(sx, sy, sw, sh, style.bgColor);
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

        // 1. Render self (background rect — from display list)
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::Rect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x, op.y, op.w, op.h, kSharpRadii, &op.r,
                                       &style.bgGradient, 0.0f, nullptr, nullptr);
                        break;
                    }
#endif
                    r.drawRect(op.x, op.y, op.w, op.h, &op.r); break;
                case DrawOp::RoundedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x, op.y, op.w, op.h, op.radii, &op.r,
                                       &style.bgGradient, 0.0f, nullptr, nullptr);
                        break;
                    }
#endif
                    r.drawRoundedRect(op.x, op.y, op.w, op.h, op.radii, &op.r); break;
                case DrawOp::BorderedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x, op.y, op.w, op.h, kSharpRadii, &op.r,
                                       &style.bgGradient, op.data[1], &op.br, nullptr);
                        break;
                    }
#endif
                    r.drawBorderedRect(op.x, op.y, op.w, op.h, &op.r, op.data[1], &op.br); break;
                case DrawOp::BorderedRoundedRect:
#ifdef MORPH_FEATURE_GRADIENT
                    if (style.bgGradient.enabled) {
                        r.drawGradRect(op.x, op.y, op.w, op.h, op.radii, &op.r,
                                       &style.bgGradient, op.data[1], &op.br, nullptr);
                        break;
                    }
#endif
                    r.drawBorderedRoundedRect(op.x, op.y, op.w, op.h, op.radii, &op.r, op.data[1], &op.br); break;
                case DrawOp::BorderRing:
#ifdef MORPH_FEATURE_BORDER
                    execBorderRingOp(r, op, style); break;
#else
                    break;
#endif
                default: break;
            }
        }

        // 2. Clip setup (from node state — correct interleaving)
        bool needClip = clipsOverflowBox();
        bool needRadiusClip = maxRadius4(radii) > 0.0f;
        bool scrolling = scrollsVertically(sh) || scrollsHorizontally(sw);

        if (needClip || needRadiusClip) {
#ifdef MORPH_FEATURE_TRANSFORM
            if (needClip) {
                if (pushedSelf)
                    r.beginRoundedClip(sx, sy, sw, sh, kSharpRadii);
                else
                    r.beginClip(sx, sy, sw, sh);
            }
#else
            if (needClip) r.beginClip(sx, sy, sw, sh);
#endif
            if (needRadiusClip) r.beginRoundedClip(sx, sy, sw, sh, radii);
        }

        // 3. Scroll + children
        if (scrolling) r.pushScrollOffset(-effScrollX(sw), -effScrollY(sh));
        for (auto* child : paintOrder()) {
            if (scrolling) {
                float childVisX = child->x - effScrollX(sw);
                float childVisY = child->y - effScrollY(sh);
                if (childVisX + child->w > x && childVisX < x + w &&
                    childVisY + child->h > y && childVisY < y + h)
                    child->executeDisplayList(r);
            } else {
                child->executeDisplayList(r);
            }
        }
        if (scrolling) r.popScrollOffset(-effScrollX(sw), -effScrollY(sh));

        // 4. Clip teardown
        if (needClip || needRadiusClip) {
            if (needRadiusClip) r.endRoundedClip();
            if (needClip) r.endClip();
        }

        // 5. Scrollbar
#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) drawScrollbar(r);
#endif

#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

    void draw(Renderer& r) override {
        // ── 1. Draw self background + border ──────────────────────
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float radii[4];
        resolvedRadii(radii, !m_isTransitioning);
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif
#ifdef MORPH_FEATURE_BORDER
        paintBoxDirect(r, style, unitEnv(w, nullptr), sx, sy, sw, sh, !m_isTransitioning,
                       false);
#else
#ifdef MORPH_FEATURE_GRADIENT
        if (style.bgGradient.enabled) {
            r.drawGradRect(sx, sy, sw, sh, radii,
                           style.bgColor, &style.bgGradient, 0.0f, nullptr,
                           nullptr);
        } else {
#endif
#ifdef MORPH_FEATURE_RADIUS
        if (maxRadius4(radii) > 0.0f) {
            r.drawRoundedRect(sx, sy, sw, sh, radii, style.bgColor);
        } else
#endif
            r.drawRect(sx, sy, sw, sh, style.bgColor);
#ifdef MORPH_FEATURE_GRADIENT
        }
#endif
#endif

        // ── 2. Children (clipped when overflow is non-visible) ────
        bool overflowClipped = clipsOverflowBox();
        bool needRectClip = overflowClipped;
        bool needRadiusClip = maxRadius4(radii) > 0.0f;
#ifdef MORPH_FEATURE_SCROLL
        bool scrolling = scrollsVertically(sh) || scrollsHorizontally(sw);
#else
        bool scrolling = false;
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

            r.pushScrollOffset(-effScrollX(sw), -effScrollY(sh));
            for (auto* child : paintOrder()) {
                if (scrolling) {
                    float childVisX = child->x - effScrollX(sw);
                    float childVisY = child->y - effScrollY(sh);
                    if (childVisX + child->w > x && childVisX < x + w &&
                        childVisY + child->h > y && childVisY < y + h)
                        child->draw(r);
                } else {
                    child->draw(r);
                }
            }
            r.popScrollOffset(-effScrollX(sw), -effScrollY(sh));

            if (needRadiusClip) r.endRoundedClip();
            if (needRectClip) r.endClip();
        } else {
            for (auto* child : paintOrder())
                child->draw(r);
        }

        // ── 3. Scrollbar ──────────────────────────────────────────
#ifdef MORPH_FEATURE_SCROLL
        if (scrolling) {
            drawScrollbar(r);
        }
#endif

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
        // Overlay style: the track paints only with an explicit track
        // color (default transparent) — otherwise just the thumb floats.
        bool showTrack = style.scrollbarTrackColor[3] > 0.01f;
        if (vBar) {
            float trackX = sx + sw - sbw;
            if (showTrack)
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
            if (showTrack)
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
