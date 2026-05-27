#pragma once
#include <vector>
#include <string>
#include "../style/style.h"
#include "event.h"
#include "../render/gl_renderer.h"

class Renderer;

enum DirtyFlag : uint8_t {
    Clean        = 0,
    StyleDirty   = 1 << 0,
    LayoutDirty  = 1 << 1,
    PaintDirty   = 1 << 2,
    ScrollDirty  = 1 << 3,
    SubtreeDirty = 1 << 4,
};

struct DirtyStats {
    int layoutCount = 0;
    int paintCount = 0;
    int fullTreeCount = 0;
    int skippedCount = 0;
    void reset() { layoutCount = 0; paintCount = 0; fullTreeCount = 0; skippedCount = 0; }
};

enum class Easing : uint8_t {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
};

enum class AnimProperty : uint8_t {
    X, Y, W, H,
    BgColorR, BgColorG, BgColorB, BgColorA,
    ColorR, ColorG, ColorB, ColorA,
    BorderRadius,
};

struct MorphAnimation {
    AnimProperty property;
    float from, to;
    float duration;
    float elapsed;
    Easing easing;
    bool running;
    bool finished;
};

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

    // Dirty rendering state
    uint8_t m_dirtyFlags = Clean | LayoutDirty | PaintDirty;

    void markDirty(DirtyFlag f);
    void clearDirty(DirtyFlag f) { m_dirtyFlags &= ~f; }
    bool isDirty(DirtyFlag f) const { return (m_dirtyFlags & f) != 0; }
    bool isFullyClean() const { return m_dirtyFlags == Clean; }

    virtual void layout(float px, float py, float parentW, float parentH,
                        Renderer* r = nullptr);
    virtual void draw(Renderer& r) = 0;
    virtual void update(float dt);
    virtual bool onEvent(MorphEvent& e) { return false; }
    virtual void onHover(bool state) {}

    void startAnimation(AnimProperty prop, float to, float duration, Easing easing = Easing::Linear);

    virtual float contentWidth(Renderer* r);
    MorphNode* hitTest(float ex, float ey);
    void addChild(MorphNode* child) {
        children.push_back(child);
        child->parent = this;
        child->markDirty(LayoutDirty);
        child->markDirty(PaintDirty);
    }
    bool dispatchEvent(MorphEvent& e, float ex, float ey);

    // Animation state
    std::vector<MorphAnimation> m_animations;
    void updateAnimations(float dt);

    // Dirty rendering
    std::vector<struct DrawOp> m_displayList;
    virtual void layoutIfNeeded(float px, float py, float parentW, float parentH,
                                Renderer* r = nullptr, DirtyStats* stats = nullptr,
                                bool force = false);
    virtual void recordDisplayList(Renderer& r);
    virtual void executeDisplayList(Renderer& r);

    virtual ~MorphNode() {}
};
