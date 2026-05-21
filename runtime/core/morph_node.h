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
    std::string fontWeight = "normal";
};

class Renderer;

class MorphNode {
public:
    float x = 0, y = 0, w = 0, h = 0;
    MorphStyle style;
    std::vector<MorphNode*> children;
    bool focused = false;

    virtual void layout(float parentW, float parentH) {}
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
