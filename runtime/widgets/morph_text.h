#pragma once
#include <string>
#include <vector>
#include "../core/node.h"
#include "../core/renderer.h"

class TextNode : public MorphNode {
public:
    std::string text;
    std::vector<std::string> lines;

    TextNode(const std::string& text) : text(text) {}

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

            float lh = style.fontSize * 1.4f;
            h = (float)lines.size() * lh;
        } else {
            if (h == 0.0f)
                h = style.fontSize * 1.4f;
        }
    }

    void wrapParagraph(const std::string& para, Renderer* r, float lineW) {
        float tw = r->measureTextWidth(para, style.fontSize, style.fontWeight);
        if (tw <= lineW) {
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
                if (r->measureTextWidth(candidate, style.fontSize, style.fontWeight) <= lineW) {
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

    void draw(Renderer& r) override {
        float lh = style.fontSize * 1.4f;
        float py = y;
        for (auto& line : lines) {
            float lx = x;
            if (style.textAlign == "center") {
                float tw = r.measureTextWidth(line, style.fontSize, style.fontWeight);
                if (tw < w) lx = x + (w - tw) * 0.5f;
            } else if (style.textAlign == "right") {
                float tw = r.measureTextWidth(line, style.fontSize, style.fontWeight);
                lx = x + w - tw;
            }
            r.drawText(line, lx, py, style.color, TextAlign::Left,
                       style.fontSize, style.fontWeight);
            py += lh;
        }
        for (auto* child : children)
            child->draw(r);
    }

    float contentWidth(Renderer* r) override {
        if (style.explicitWidth >= 0.0f) return style.explicitWidth;
        if (r) {
            float tw = r->measureTextWidth(text, style.fontSize, style.fontWeight);
            float pl = style.padding[3], pr = style.padding[1];
            return tw + pl + pr;
        }
        return MorphNode::contentWidth(r);
    }
};
