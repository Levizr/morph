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
        float br = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);

        // Clip self for borderRadius (each node's flat render handles children separately)
        if (br > 0.0f) {
            DrawOp cl; cl.setClip(sx, sy, sw, sh, true, br);
            m_displayList.push_back(cl);
        }

        DrawOp tex;
        tex.type = DrawOp::TextureQuad;
        tex.x = sx; tex.y = sy; tex.w = sw; tex.h = sh;
        tex.texId = textureId;
        tex.r = tex.g = tex.b = tex.a = 1.0f;
        m_displayList.push_back(tex);

#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
            DrawOp brr;
            brr.setBordered(sx, sy, sw, sh, br, style.bgColor,
                            bw, style.borderColor);
            brr.type = DrawOp::BorderRing;
            m_displayList.push_back(brr);
        }
#endif

        if (br > 0.0f) {
            DrawOp ec; ec.setEndClip(true);
            m_displayList.push_back(ec);
        }
    }

    void executeDisplayList(Renderer& r) override {
        for (auto& op : m_displayList) {
            switch (op.type) {
                case DrawOp::BeginClip: r.beginClip(op.x,op.y,op.w,op.h); break;
                case DrawOp::EndClip: r.endClip(); break;
                case DrawOp::BeginRoundedClip: r.beginRoundedClip(op.x,op.y,op.w,op.h,op.data[0]); break;
                case DrawOp::EndRoundedClip: r.endRoundedClip(); break;
                case DrawOp::BorderRing: r.drawBorderRing(op.x,op.y,op.w,op.h,op.data[0],op.data[1],&op.br); break;
                case DrawOp::TextureQuad: r.drawTexture(op.texId, op.x, op.y, op.w, op.h); break;
                default: break;
            }
        }
        for (auto* c : paintOrder()) c->executeDisplayList(r);
    }

    void draw(Renderer& r) override {
        ensureLoaded(r);

        if (textureId && imgW > 0 && imgH > 0) {
            auto sc = [&](float v) { return m_hasLayoutTransition ? v : std::round(v); };
            float sx = sc(x), sy = sc(y);
            float sw = sc(w), sh = sc(h);
            float br = m_isTransitioning ? style.borderRadius : snapRadius(style.borderRadius);
            if (br > 0.0f) {
                r.beginRoundedClip(sx, sy, sw, sh, br);
            }

            r.drawTexture(textureId, sx, sy, sw, sh);

#ifdef MORPH_FEATURE_BORDER
            if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
                float bw = m_isTransitioning ? style.borderWidth : snapBorderWidth(style.borderWidth);
                r.drawBorderRing(sx, sy, sw, sh, br,
                                 bw, style.borderColor);
            }
#endif

            if (br > 0.0f) {
                r.endRoundedClip();
            }
        }

        // Draw children on top (e.g., overlay text)
        for (auto* c : paintOrder()) c->draw(r);
    }

    void layout(float px, float py, float parentW, float parentH,
                Renderer* r = nullptr) override {
        MorphNode::layout(px, py, parentW, parentH, r);

        if (r) ensureLoaded(*r);
        if (imgW > 0 && imgH > 0) {
            float aspect = (float)imgW / (float)imgH;
            bool hasExplicitW = style.explicitWidth > 0;
            bool hasExplicitH = style.explicitHeight > 0;
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
