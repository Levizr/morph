#pragma once

struct ViewportContext {
    unsigned int fbo;
    int x, y, w, h;
    float deltaTime;
    float mouseX, mouseY;
    bool focused;
};

class MorphViewportDriver {
public:
    virtual void onInit(ViewportContext& ctx)                          {}
    virtual void onDraw(ViewportContext& ctx)                          = 0;
    virtual void onResize(int w, int h, ViewportContext& ctx)          {}
    virtual void onMouseMove(float x, float y, ViewportContext& ctx)   {}
    virtual void onMouseDown(int btn, ViewportContext& ctx)            {}
    virtual void onScroll(float delta, ViewportContext& ctx)           {}
    virtual void onKeyDown(int key, ViewportContext& ctx)              {}
    virtual ~MorphViewportDriver() {}
};
