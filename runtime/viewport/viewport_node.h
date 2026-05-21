#pragma once
#include "../core/morph_node.h"
#include "viewport_driver.h"

class ViewportNode : public MorphNode {
    MorphViewportDriver* driver;
public:
    ViewportNode(MorphViewportDriver* d) : driver(d) {}
    void draw(Renderer& r) override { if (driver) driver->onDraw(r); }
};
