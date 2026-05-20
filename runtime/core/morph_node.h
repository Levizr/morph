#pragma once
#include <vector>
#include <functional>
#include <string>
#include "event.h"

struct MorphStyle {
    float bgColor[4] = {1,1,1,1};
    float color[4]   = {0,0,0,1};
    float borderRadius = 0.0f;
    float fontSize     = 16.0f;
    float padding[4]   = {0,0,0,0};
    float margin[4]    = {0,0,0,0};
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
    virtual void onEvent(MorphEvent& e) {}
    virtual void onHover(bool state) {}

    void addChild(MorphNode* child) { children.push_back(child); }
    virtual ~MorphNode() {}
};
