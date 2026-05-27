#pragma once
#include "../core/node.h"

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

        float br = style.borderRadius;
        if (br > 0.0f) {
            DrawOp cl; cl.setClip(x, y, w, h, true, br);
            m_displayList.push_back(cl);
        }

        DrawOp tex;
        tex.type = DrawOp::TextureQuad;
        tex.x = x; tex.y = y; tex.w = w; tex.h = h;
        tex.texId = textureId;
        tex.r = tex.g = tex.b = tex.a = 1.0f;
        m_displayList.push_back(tex);

#ifdef MORPH_FEATURE_BORDER
        if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
            DrawOp brr;
            brr.setBordered(x, y, w, h, br, style.bgColor, style.borderWidth, style.borderColor);
            brr.type = DrawOp::BorderRing; // draw border ring only
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
        for (auto* c : children) c->executeDisplayList(r);
    }

    void draw(Renderer& r) override {
        ensureLoaded(r);

        if (textureId && imgW > 0 && imgH > 0) {
            float br = style.borderRadius;
            if (br > 0.0f) {
                r.beginRoundedClip(x, y, w, h, br);
            }

            r.drawTexture(textureId, x, y, w, h);

#ifdef MORPH_FEATURE_BORDER
            if (style.borderWidth > 0.0f && style.borderStyle == "solid") {
                // Border drawn inside the stencil scope so it's not hidden
                // by nested GL_INCR stencil masking.
                r.drawBorderRing(x, y, w, h, style.borderRadius,
                                 style.borderWidth, style.borderColor);
            }
#endif

            if (br > 0.0f) {
                r.endRoundedClip();
            }
        }

        // Draw children on top (e.g., overlay text)
        for (auto* c : children) c->draw(r);
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
