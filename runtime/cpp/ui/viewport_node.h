#pragma once
#include "../core/node.h"
#include "viewport_driver.h"

class ViewportNode : public MorphNode {
    MorphViewportDriver* driver;
public:
    ViewportNode(MorphViewportDriver* d) : driver(d) {}
    void draw(Renderer& r) override {
#ifdef MORPH_FEATURE_TRANSFORM
        bool pushedSelf = pushSelfTransform(r, x, y);
#endif
        if (driver) driver->onDraw(r);
#ifdef MORPH_FEATURE_TRANSFORM
        if (pushedSelf) r.popTransform();
#endif
    }
};
