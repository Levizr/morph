#pragma once
#include <string>
#include <vector>
#include <cctype>
#include "../core/node.h"

#ifdef MORPH_FEATURE_TEXT_SHADOW
// Approximate CSS text-shadow blur with a 7x7 Gaussian-weighted text pass.
// CSS blur-radius is 2*sigma; samples sit 0.6*sigma apart and span
// +/-1.8*sigma. FreeType has no cheap per-glyph blur, so this soft cluster
// is the contract's approximation. The small alpha boost keeps the shadow
// core from washing out under source-over compositing.
inline void paintTextShadows(Renderer& r,
                             const std::vector<FlatTextShadow>& shadows,
                             const std::string& text, float x, float y,
                             const float baseColor[4], TextAlign align,
                             float fontSize, CSS::FontWeight fontWeight,
                             bool centerInk, float letterSpacing)
{
    static constexpr float kWeights[7] = {
        0.0489866f, 0.1204881f, 0.2067575f, 0.2475357f,
        0.2067575f, 0.1204881f, 0.0489866f,
    };
    static constexpr float kAlphaBoost = 1.2f;
    for (const auto& sh : shadows)
    {
        float color[4];
        if (sh.hasColor)
        {
            color[0] = sh.color[0]; color[1] = sh.color[1];
            color[2] = sh.color[2]; color[3] = sh.color[3];
        }
        else
        {
            color[0] = baseColor[0]; color[1] = baseColor[1];
            color[2] = baseColor[2]; color[3] = baseColor[3];
        }
        if (sh.blur <= 0.75f)
        {
            r.drawText(text, x + sh.offsetX, y + sh.offsetY, color, align,
                       fontSize, fontWeight, centerInk, letterSpacing);
            continue;
        }
        const float step = sh.blur * 0.3f;
        for (int gy = -3; gy <= 3; ++gy)
        {
            for (int gx = -3; gx <= 3; ++gx)
            {
                float alpha = color[3] * kWeights[gy + 3] * kWeights[gx + 3] *
                              kAlphaBoost;
                if (alpha > 1.0f)
                    alpha = 1.0f;
                if (alpha < 0.002f)
                    continue;
                float pass[4] = {color[0], color[1], color[2], alpha};
                r.drawText(text, x + sh.offsetX + (float)gx * step,
                           y + sh.offsetY + (float)gy * step, pass, align,
                           fontSize, fontWeight, centerInk, letterSpacing);
            }
        }
    }
}
#endif

class TextNode : public MorphNode {
public:
    std::string text;
    std::vector<std::string> lines;

    // Display list helpers
    struct TextOp {
        std::string text;
        float x, y;
        float color[4];
        TextAlign align;
        float fontSize;
        CSS::FontWeight fontWeight = CSS::FontWeight::Normal;
        float letterSpacing = 0.0f;
        bool centerInk = true;
#ifdef MORPH_FEATURE_TEXT_SHADOW
        std::vector<FlatTextShadow> shadows;
#endif
    };
    std::vector<TextOp> m_textOps;

    // Renderer alignment → style alignment at the FlatTextOp boundary
    // (values coincide; Justify never originates here).
    static CSS::TextAlign cssAlign(TextAlign a)
    {
        if (a == TextAlign::Center)
            return CSS::TextAlign::Center;
        if (a == TextAlign::Right)
            return CSS::TextAlign::Right;
        return CSS::TextAlign::Left;
    }

    TextNode(const std::string& text) : text(text) {}

    void setText(const std::string& newText) {
        if (text == newText) return;
        text = newText;
        m_frags.clear();
        markDirty(LayoutDirty);
        markDirty(PaintDirty);
    }

    bool isWhitespaceOnly() const override {
        for (size_t i = 0; i < text.size(); i++)
            if (!std::isspace((unsigned char)text[i])) return false;
        return true;
    }

    bool isTextRun() const override { return true; }
    bool isEmptyText() const override { return text.empty(); }
    std::string textContent() const override { return text; }

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        m_textOps.clear();
        float spacing = _effLetterSpacing(&r);
#ifdef MORPH_FEATURE_TEXT_SHADOW
        std::vector<FlatTextShadow> shadows;
        _collectShadows(&r, shadows);
#endif
        if (!m_frags.empty()) {
            // Word fragments placed by the line breaker: paint each slice
            // at its own box instead of the whole wrapped lines.
            for (auto& frag : m_frags) {
                TextOp top;
                top.text = frag.text;
                top.x = frag.x;
                top.y = frag.y;
                top.color[0] = style.color[0]; top.color[1] = style.color[1];
                top.color[2] = style.color[2]; top.color[3] = style.color[3];
                top.align = TextAlign::Left;
                top.fontSize = _effFontSize();
                top.fontWeight = _effFontWeight();
                top.letterSpacing = spacing;
                top.centerInk = m_centerInk;
#ifdef MORPH_FEATURE_TEXT_SHADOW
                top.shadows = shadows;
#endif
                m_textOps.push_back(top);
            }
            return;
        }
        float lh = _effFontSize() * 1.4f;
        float py = y;
        for (auto& line : lines) {
            float lx = x;
            if (_effTextAlign() == CSS::TextAlign::Center) {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight(), spacing);
                if (tw < w) lx = x + (w - tw) * 0.5f;
            } else if (_effTextAlign() == CSS::TextAlign::Right) {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight(), spacing);
                lx = x + w - tw;
            }
            TextOp top;
            top.text = line;
            top.x = lx;
            top.y = py;
            top.color[0] = style.color[0]; top.color[1] = style.color[1];
            top.color[2] = style.color[2]; top.color[3] = style.color[3];
            top.align = TextAlign::Left;
            top.fontSize = _effFontSize();
            top.fontWeight = _effFontWeight();
            top.letterSpacing = spacing;
            top.centerInk = m_centerInk;
#ifdef MORPH_FEATURE_TEXT_SHADOW
            top.shadows = shadows;
#endif
            m_textOps.push_back(top);
            py += lh;
        }
    }

    void executeDisplayList(Renderer& r) override {
        for (auto& top : m_textOps) {
            float* effectiveColor = top.color;
            if (parent && !m_colorOverridden) {
                MorphNode* src = parent;
                while (src) {
                    float* c = src->style.color;
                    bool nonDefault = c[0] != 0.0f || c[1] != 0.0f || c[2] != 0.0f || c[3] != 1.0f;
                    if (nonDefault && !src->m_colorInherited)
                        break;
                    src = src->parent;
                }
                effectiveColor = src ? src->style.color : parent->style.color;
            }
#ifdef MORPH_FEATURE_TEXT_SHADOW
            paintTextShadows(r, top.shadows, top.text, top.x, top.y,
                             effectiveColor, top.align, top.fontSize,
                             top.fontWeight, top.centerInk, top.letterSpacing);
#endif
            r.drawText(top.text, top.x, top.y, effectiveColor, top.align,
                       top.fontSize, top.fontWeight, top.centerInk,
                       top.letterSpacing);
        }
        for (auto* child : paintOrder())
            child->executeDisplayList(r);
    }

    void layout(float px, float py, float parentW, float parentH,
                Renderer* r = nullptr) override {
        MorphNode::layout(px, py, parentW, parentH, r);

        if (r && w > 0.0f) {
            // Constrain wrap width by maxWidth (`%` vs the CB width).
            float wrapW = w;
            if (style.maxWidth.isSet()) {
                float mw = resolveUnits(style.maxWidth, unitEnv(parentW, r));
                if (mw > 0.0f && wrapW > mw) wrapW = mw;
            }

            // Split by newlines first, then word-wrap each paragraph
            lines.clear();
            size_t paraStart = 0;
            while (paraStart < text.size()) {
                size_t paraEnd = text.find('\n', paraStart);
                if (paraEnd == std::string::npos) paraEnd = text.size();
                std::string para = text.substr(paraStart, paraEnd - paraStart);
                wrapParagraph(para, r, wrapW);
                paraStart = paraEnd + 1;
            }
            if (lines.empty() && !text.empty())
                wrapParagraph(text, r, wrapW);
            if (lines.empty())
                lines.push_back(text);

            float lh = _effFontSize() * 1.4f;
            h = (float)lines.size() * lh;
        } else {
            if (h == 0.0f)
                h = _effFontSize() * 1.4f;
        }
    }

    void wrapParagraph(const std::string& para, Renderer* r, float lineW) {
        float epsilon = 0.5f;
        float spacing = _effLetterSpacing(r);
        float tw = r->measureTextWidth(para, _effFontSize(), _effFontWeight(), spacing);
        if (tw <= lineW + epsilon) {
            lines.push_back(para);
            return;
        }
        size_t start = 0;
        while (start < para.size()) {
            // Find the next space (or end)
            size_t end = para.find(' ', start);
            if (end == std::string::npos) {
                // Last word (or whole remaining text)
                lines.push_back(para.substr(start));
                break;
            }
            // Try adding more words while they fit
            size_t fitEnd = end;
            while (fitEnd < para.size()) {
                size_t nextSpace = para.find(' ', fitEnd + 1);
                if (nextSpace == std::string::npos) nextSpace = para.size();
                std::string candidate = para.substr(start, nextSpace - start);
                if (r->measureTextWidth(candidate, _effFontSize(), _effFontWeight(), spacing)
                    <= lineW + epsilon) {
                    fitEnd = nextSpace;
                } else {
                    break;
                }
            }
            // If the first word itself doesn't fit, force it
            if (fitEnd == end && start == 0) {
                std::string word = para.substr(start, end - start);
                lines.push_back(word);
                start = end + 1;
                continue;
            }
            if (fitEnd == start) fitEnd = para.size();
            lines.push_back(para.substr(start, fitEnd - start));
            start = fitEnd + 1;
        }
    }

    int flattenExtra(RenderFrame& frame, FlatRenderNode& fn) override {
        (void)fn;
        // Compute effective color (same parent-chain logic as executeDisplayList)
        float* effectiveColor = nullptr;
        if (!m_textOps.empty()) {
            effectiveColor = m_textOps[0].color;
        }
        if (parent && !m_colorOverridden) {
            MorphNode* src = parent;
            while (src) {
                float* c = src->style.color;
                bool nonDefault = c[0] != 0.0f || c[1] != 0.0f || c[2] != 0.0f || c[3] != 1.0f;
                if (nonDefault && !src->m_colorInherited)
                    break;
                src = src->parent;
            }
            effectiveColor = src ? src->style.color : parent->style.color;
        }
        int count = 0;
        for (auto& top : m_textOps) {
            FlatTextOp fto;
            fto.nodeId = fn.id;
            fto.text = top.text;
            fto.x = top.x;
            fto.y = top.y;
            if (effectiveColor) {
                fto.color[0] = effectiveColor[0]; fto.color[1] = effectiveColor[1];
                fto.color[2] = effectiveColor[2]; fto.color[3] = effectiveColor[3];
            } else {
                fto.color[0] = top.color[0]; fto.color[1] = top.color[1];
                fto.color[2] = top.color[2]; fto.color[3] = top.color[3];
            }
            // Group opacity: the flat node's `opacity` already holds the
            // product of every ancestor's opacity (own opacity is 1 on text).
            fto.color[3] *= fn.opacity;
            fto.align = cssAlign(top.align);
            fto.centerInk = top.centerInk ? (uint8_t)1 : (uint8_t)0;
            fto.fontSize = top.fontSize;
            fto.fontWeight = top.fontWeight;
            fto.letterSpacing = top.letterSpacing;
#ifdef MORPH_FEATURE_TEXT_SHADOW
            fto.shadows = top.shadows;
            for (auto& sh : fto.shadows) {
                sh.color[3] *= fn.opacity;
            }
#endif
            frame.textOps.push_back(fto);
            count++;
        }
        return count;
    }

    void draw(Renderer& r) override {
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, x, y);
#endif
        float lh = _effFontSize() * 1.4f;
        float py = y;
        float spacing = _effLetterSpacing(&r);
#ifdef MORPH_FEATURE_TEXT_SHADOW
        std::vector<FlatTextShadow> shadows;
        _collectShadows(&r, shadows);
#endif
        for (auto& line : lines) {
            float lx = x;
            if (_effTextAlign() == CSS::TextAlign::Center) {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight(), spacing);
                if (tw < w) lx = x + (w - tw) * 0.5f;
            } else if (_effTextAlign() == CSS::TextAlign::Right) {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight(), spacing);
                lx = x + w - tw;
            }
            float* effectiveColor = style.color;
            if (parent && !m_colorOverridden) {
                MorphNode* src = parent;
                while (src) {
                    float* c = src->style.color;
                    bool nonDefault = c[0] != 0.0f || c[1] != 0.0f || c[2] != 0.0f || c[3] != 1.0f;
                    if (nonDefault && !src->m_colorInherited)
                        break;
                    src = src->parent;
                }
                effectiveColor = src ? src->style.color : parent->style.color;
            }
#ifdef MORPH_FEATURE_TEXT_SHADOW
            paintTextShadows(r, shadows, line, lx, py, effectiveColor,
                             TextAlign::Left, _effFontSize(), _effFontWeight(),
                             m_centerInk, spacing);
#endif
            r.drawText(line, lx, py, effectiveColor, TextAlign::Left,
                       _effFontSize(), _effFontWeight(), m_centerInk, spacing);
            py += lh;
        }
        for (auto* child : paintOrder())
            child->draw(r);
#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }

    float contentWidth(Renderer* r) override {
        if (style.explicitWidth.isSet() && !style.explicitWidth.isPercent())
            return resolveUnits(style.explicitWidth, unitEnv(0.0f, r));
        if (r) {
            float tw = 0.0f;
            size_t start = 0;
            while (start <= text.size()) {
                size_t end = text.find('\n', start);
                if (end == std::string::npos) end = text.size();
                float lw = r->measureTextWidth(text.substr(start, end - start),
                                               _effFontSize(), _effFontWeight(),
                                               _effLetterSpacing(r));
                if (lw > tw) tw = lw;
                if (end == text.size()) break;
                start = end + 1;
            }
            UnitEnv twEnv = unitEnv(0.0f, r);
            float pl = resolveUnits(style.padding[3], twEnv);
            float pr = resolveUnits(style.padding[1], twEnv);
            return tw + pl + pr;
        }
        return MorphNode::contentWidth(r);
    }

private:
    float _effLetterSpacing(Renderer* r) const {
        return resolvedLetterSpacing(r);
    }

#ifdef MORPH_FEATURE_TEXT_SHADOW
    void _collectShadows(Renderer* r, std::vector<FlatTextShadow>& out) const {
        const MorphNode* srcNode = this;
        while (srcNode && !srcNode->style.textShadowSet)
            srcNode = srcNode->parent;
        if (!srcNode) return;
        int count = srcNode->style.textShadowCount;
        if (count > 4) count = 4;
        // Shadow lengths resolve against the element that declared them.
        UnitEnv env = srcNode->unitEnv(w, r);
        for (int i = 0; i < count; i++) {
            const TextShadow& src = srcNode->style.textShadows[i];
            FlatTextShadow dst;
            dst.offsetX = resolveUnits(src.offsetX, env);
            dst.offsetY = resolveUnits(src.offsetY, env);
            dst.blur = resolveUnits(src.blur, env);
            dst.hasColor = src.hasColor;
            dst.color[0] = src.color[0]; dst.color[1] = src.color[1];
            dst.color[2] = src.color[2]; dst.color[3] = src.color[3];
            out.push_back(dst);
        }
    }
#endif

    float _effFontSize() const {
        return resolvedFontSize(nullptr);
    }
    CSS::FontWeight _effFontWeight() const {
        return resolvedFontWeight();
    }
    CSS::TextAlign _effTextAlign() const {
        if (style.textAlign != CSS::TextAlign::Left) return style.textAlign;
        for (const MorphNode* src = parent; src; src = src->parent) {
            if (src->style.textAlign != CSS::TextAlign::Left)
                return src->style.textAlign;
        }
        return style.textAlign;
    }
};
