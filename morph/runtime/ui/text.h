#pragma once
#include <string>
#include <vector>
#include "../core/node.h"

class TextNode : public MorphNode {
public:
    std::string text;
    std::vector<std::string> lines;
    bool m_colorOverridden = false;

    // Display list helpers
    struct TextOp {
        std::string text;
        float x, y;
        float color[4];
        TextAlign align;
        float fontSize;
        std::string fontWeight;
    };
    std::vector<TextOp> m_textOps;

    TextNode(const std::string& text) : text(text) {}

    void setText(const std::string& newText) {
        if (text == newText) return;
        text = newText;
        markDirty(LayoutDirty);
        markDirty(PaintDirty);
    }

    void recordDisplayList(Renderer& r) override {
        m_displayList.clear();
        m_textOps.clear();
        float lh = _effFontSize() * 1.4f;
        float py = y;
        for (auto& line : lines) {
            float lx = x;
            if (style.textAlign == "center") {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight());
                if (tw < w) lx = x + (w - tw) * 0.5f;
            } else if (style.textAlign == "right") {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight());
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
            r.drawText(top.text, top.x, top.y, effectiveColor, top.align,
                       top.fontSize, top.fontWeight);
        }
        for (auto* child : children)
            child->executeDisplayList(r);
    }

    void layout(float px, float py, float parentW, float parentH,
                Renderer* r = nullptr) override {
        MorphNode::layout(px, py, parentW, parentH, r);

        if (r && w > 0.0f) {
            // Constrain wrap width by maxWidth
            float wrapW = w;
            if (style.maxWidth > 0.0f && wrapW > style.maxWidth)
                wrapW = style.maxWidth;

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
        float tw = r->measureTextWidth(para, _effFontSize(), _effFontWeight());
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
                if (r->measureTextWidth(candidate, _effFontSize(), _effFontWeight()) <= lineW + epsilon) {
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
            fto.align = (uint8_t)top.align;
            fto.fontSize = top.fontSize;
            fto.fontWeight = (top.fontWeight == "bold" || top.fontWeight == "700" || top.fontWeight == "800" || top.fontWeight == "900") ? (uint8_t)1 : (uint8_t)0;
            frame.textOps.push_back(fto);
            count++;
        }
        return count;
    }

    void draw(Renderer& r) override {
        float lh = _effFontSize() * 1.4f;
        float py = y;
        for (auto& line : lines) {
            float lx = x;
            if (style.textAlign == "center") {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight());
                if (tw < w) lx = x + (w - tw) * 0.5f;
            } else if (style.textAlign == "right") {
                float tw = r.measureTextWidth(line, _effFontSize(), _effFontWeight());
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
            r.drawText(line, lx, py, effectiveColor, TextAlign::Left,
                       _effFontSize(), _effFontWeight());
            py += lh;
        }
        for (auto* child : children)
            child->draw(r);
    }

    float contentWidth(Renderer* r) override {
        if (style.explicitWidth >= 0.0f) return style.explicitWidth;
        if (r) {
            float tw = r->measureTextWidth(text, _effFontSize(), _effFontWeight());
            float pl = style.padding[3], pr = style.padding[1];
            return tw + pl + pr;
        }
        return MorphNode::contentWidth(r);
    }

private:
    float _effFontSize() const {
        if (style.fontSize != 16.0f || !parent) return style.fontSize;
        float pfs = parent->style.fontSize;
        return (pfs != 16.0f) ? pfs : style.fontSize;
    }
    const std::string& _effFontWeight() const {
        if (style.fontWeight != "normal" || !parent) return style.fontWeight;
        return (parent->style.fontWeight != "normal") ? parent->style.fontWeight : style.fontWeight;
    }
};
