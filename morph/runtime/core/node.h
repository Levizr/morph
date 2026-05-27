#pragma once
#include <vector>
#include <string>
#include "../style/style.h"
#include "event.h"

class Renderer;

class MorphNode {
public:
    float x = 0, y = 0, w = 0, h = 0;
    MorphStyle style;
    MorphNode* parent = nullptr;
    std::vector<MorphNode*> children;
    bool focused = false;
    std::string type = "div";

    // Scroll state (always present — zero overhead when unused)
    float scrollY = 0;
    float contentH = 0;
    bool scrollEnabled = false;
    bool scrollThumbHover = false;
    bool scrollDragging = false;
    float scrollDragStartY = 0;
    float scrollDragStartVal = 0;

    virtual void layout(float px, float py, float parentW, float parentH,
                        Renderer* r = nullptr);
    virtual void draw(Renderer& r) = 0;
    virtual bool onEvent(MorphEvent& e) { return false; }
    virtual void onHover(bool state) {}

    virtual float contentWidth(Renderer* r);
    MorphNode* hitTest(float ex, float ey);
    void addChild(MorphNode* child) { children.push_back(child); child->parent = this; }
    bool dispatchEvent(MorphEvent& e, float ex, float ey);

    virtual ~MorphNode() {}
};
