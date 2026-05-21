#pragma once
#include <vector>
#include <functional>
#include <string>
#include "event.h"

struct MorphStyle {
    float bgColor[4] = {0,0,0,0};
    float color[4]   = {0,0,0,1};
    float borderRadius = 0.0f;
    float fontSize     = 16.0f;
    float padding[4]   = {0,0,0,0};
    float margin[4]    = {0,0,0,0};
    float gap          = 0.0f;
    float explicitWidth  = -1.0f;
    float explicitHeight = -1.0f;
    std::string fontWeight = "normal";
};

class Renderer;

class MorphNode {
public:
    float x = 0, y = 0, w = 0, h = 0;
    MorphStyle style;
    std::vector<MorphNode*> children;
    bool focused = false;

    virtual void layout(float px, float py, float parentW, float parentH) {
        float ml = style.margin[3], mr = style.margin[1];
        float mt = style.margin[0], mb = style.margin[2];

        x = px + ml;
        y = py + mt;

        if (style.explicitWidth >= 0.0f)
            w = style.explicitWidth;
        else
            w = parentW - ml - mr;

        if (style.explicitHeight >= 0.0f)
            h = style.explicitHeight;
        else
            h = 0.0f;

        float pl = style.padding[3], pr = style.padding[1];
        float pt = style.padding[0], pb = style.padding[2];

        float cw = w - pl - pr;
        if (cw < 0) cw = 0;
        float cx = x + pl;
        float cy = y + pt;

        for (auto* child : children) {
            child->layout(cx, cy, cw, 0.0f);
            cy += child->h + style.gap;
        }

        // Auto-height: expand to contain children if height not explicitly set
        if (style.explicitHeight < 0.0f && !children.empty()) {
            float lastBottom = cy - style.gap;
            float contentH = lastBottom - y + pb;
            if (contentH > h)
                h = contentH;
        }
    }

    virtual void draw(Renderer& r) = 0;
    virtual bool onEvent(MorphEvent& e) { return false; }
    virtual void onHover(bool state) {}

    void addChild(MorphNode* child) { children.push_back(child); }

    // Dispatch event to deepest child containing (ex, ey); bubble up
    // Returns true if event was handled (stops bubble)
    bool dispatchEvent(MorphEvent& e, float ex, float ey) {
        for (auto it = children.rbegin(); it != children.rend(); ++it) {
            auto* c = *it;
            if (ex >= c->x && ex <= c->x + c->w &&
                ey >= c->y && ey <= c->y + c->h) {
                if (c->dispatchEvent(e, ex, ey))
                    return true;
            }
        }
        return onEvent(e);
    }

    virtual ~MorphNode() {}
};
