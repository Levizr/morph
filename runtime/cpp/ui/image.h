#pragma once
#include "../core/node.h"
#include "radius.h"

class ImageNode : public MorphNode {
public:
    std::string src;
    std::string alt;
    mutable unsigned int textureId = 0;
    mutable int imgW = 0, imgH = 0;
    mutable bool loaded = false;

    ImageNode(const std::string& src, const std::string& alt = "")
        : src(src), alt(alt) {}

    void ensureLoaded(Renderer& r) const {
        if (loaded) return;
        loaded = true;
        if (src.empty()) return;
        textureId = r.loadTexture(src, imgW, imgH);
    }

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        ensureLoaded(r);
        if (!textureId || imgW <= 0 || imgH <= 0) return;

        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
        float sw = sc(w), sh = sc(h);
        float imRadii[4];
        resolvedRadii(imRadii, !m_isTransitioning);

        // Clip self for borderRadius (each node's flat render handles children separately)
        if (maxRadius4(imRadii) > 0.0f) {
            DrawOp cl; cl.setClip(sx, sy, sw, sh, true, imRadii);
            m_displayList.push_back(cl);
        }

        DrawOp tex;
        tex.type = DrawOp::TextureQuad;
        tex.x = sx; tex.y = sy; tex.w = sw; tex.h = sh;
        tex.texId = textureId;
        tex.r = tex.g = tex.b = tex.a = 1.0f;
        m_displayList.push_back(tex);

#ifdef MORPH_FEATURE_BORDER
        if (resolvedBorderWidth() > 0.0f && style.borderStyle == CSS::BorderStyle::Solid) {
            float bw0 = resolvedBorderWidth();
            float bw = m_isTransitioning ? bw0 : snapBorderWidth(bw0);
            DrawOp brr;
            brr.setBordered(sx, sy, sw, sh, imRadii, style.bgColor,
                            bw, style.borderColor);
            brr.type = DrawOp::BorderRing;
            m_displayList.push_back(brr);
        }
#endif

        if (maxRadius4(imRadii) > 0.0f) {
            DrawOp ec; ec.setEndClip(true);
            m_displayList.push_back(ec);
        }
    }

    void executeDisplayList(Renderer& r) override {
#ifdef MORPH_FEATURE_TRANSFORM
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        bool pushedSelf = pushSelfTransform(r, sc(x), sc(y));
#endif
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::BeginClip: r.beginClip(op.x,op.y,op.w,op.h); break;
                case DrawOp::EndClip: r.endClip(); break;
                case DrawOp::BeginRoundedClip: r.beginRoundedClip(op.x,op.y,op.w,op.h,op.radii); break;
                case DrawOp::EndRoundedClip: r.endRoundedClip(); break;
                case DrawOp::BorderRing: r.drawBorderRing(op.x,op.y,op.w,op.h,op.radii,op.data[1],&op.br); break;
                case DrawOp::TextureQuad: r.drawTexture(op.texId, op.x, op.y, op.w, op.h); break;
                default: break;
            }
        }
        for (auto* c : paintOrder()) c->executeDisplayList(r);
#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

    void draw(Renderer& r) override {
        ensureLoaded(r);
        auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
        float sx = sc(x), sy = sc(y);
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, sx, sy);
#endif

        if (textureId && imgW > 0 && imgH > 0) {
            float sw = sc(w), sh = sc(h);
            float imRadii[4];
            resolvedRadii(imRadii, !m_isTransitioning);
            if (maxRadius4(imRadii) > 0.0f) {
                r.beginRoundedClip(sx, sy, sw, sh, imRadii);
            }

            r.drawTexture(textureId, sx, sy, sw, sh);

#ifdef MORPH_FEATURE_BORDER
            if (resolvedBorderWidth() > 0.0f && style.borderStyle == CSS::BorderStyle::Solid) {
                float bw0 = resolvedBorderWidth();
            float bw = m_isTransitioning ? bw0 : snapBorderWidth(bw0);
                r.drawBorderRing(sx, sy, sw, sh, imRadii,
                                 bw, style.borderColor);
            }
#endif

            if (maxRadius4(imRadii) > 0.0f) {
                r.endRoundedClip();
            }
        }

        // Draw children on top (e.g., overlay text)
        for (auto* c : paintOrder()) c->draw(r);

#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

    void layout(float px, float py, float parentW, float parentH,
                Renderer* r = nullptr) override {
        MorphNode::layout(px, py, parentW, parentH, r);

        if (r) ensureLoaded(*r);
        if (imgW > 0 && imgH > 0) {
            float aspect = (float)imgW / (float)imgH;
            bool hasExplicitW = style.explicitWidth.isSet();
            bool hasExplicitH = style.explicitHeight.isSet();
            if (hasExplicitW && !hasExplicitH) {
                h = w / aspect;
            } else if (!hasExplicitW && hasExplicitH) {
                w = h * aspect;
            } else if (!hasExplicitW && !hasExplicitH) {
                w = (float)imgW;
                h = (float)imgH;
            }
        }

        for (auto* c : children)
            c->layout(x, y, w, h, r);
    }
};
