#pragma once
#include <vector>
#include <string>
#include <algorithm>
#include <functional>
#include "../style/style.h"
#include "event.h"
#include "../types/js_value.h"
#include "../render/gl_renderer.h"
#include "render_frame.h"

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

// ── Hover transition state (heap-allocated only when active) ──
struct HoverTransition {
    MorphStyle startStyle;
    MorphStyle targetStyle;
    MorphStyle preHoverStyle; // snapshot before hover, for restore on exit
    float elapsed = 0.0f;
    bool active = false;
};

// ── Ancestor-hover rule (built at codegen / deserialization) ──
struct AncestorHoverRule {
    std::string ancestorTag;
    MorphStyle style;
};

// ── Ancestor-hover transition state ──
struct AncestorHoverTransition {
    MorphStyle revertStyle;  // style before ANY ancestor-hover was applied
    MorphStyle targetStyle;  // the rule's style
    float elapsed = 0.0f;
    bool active = false;
    bool applying = true;    // true = animating to target, false = animating back
    int applyCount = 0;      // number of active ancestor-hover rules
};

class MorphNode {
public:
    float x = 0, y = 0, w = 0, h = 0;
    MorphStyle style;
    float m_computedMargin[4] = {0}; // resolved margins after latest layout()
    MorphStyle* hoverStyle = nullptr; // allocated only when :hover rules exist

    // Transition config (0 duration = no transition, instant snap)
    float m_transitionDuration = 0.0f;
    Easing m_transitionEasing = Easing::EaseInOut;

    std::string nodeId;
    MorphNode* parent = nullptr;
    std::vector<MorphNode*> children;
    bool focused = false;
    bool m_colorInherited = false;
    std::string type = "div";

    // Scroll state (always present — zero overhead when unused)
    float scrollY = 0;
    float contentH = 0;
    bool scrollEnabled = false;
    bool scrollThumbHover = false;
    bool scrollDragging = false;
    float scrollDragStartY = 0;
    float scrollDragStartVal = 0;

    // True while this node has an active hover/programmatic transition (no propagation)
    bool m_isTransitioning = false;
    // True when active transition changes layout-affecting properties (size, margin, padding, etc.)
    bool m_hasLayoutTransition = false;

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
    virtual void setText(const std::string&) {}

    // Runtime class name tracking (for dev-mode reactive className)
    std::string className;
    void setClassName(const std::string& c) {
        if (className == c) return;
        className = c;
        markDirty(PaintDirty);
    }

    std::function<void(JsObject)> onClick;
    std::function<void(JsObject)> onDoubleClick;
    std::function<void(JsObject)> onMouseDown;
    std::function<void(JsObject)> onMouseUp;
    std::function<void(JsObject)> onMouseEnter;
    std::function<void(JsObject)> onMouseLeave;
    std::function<void(JsObject)> onKeyDown;
    std::function<void(JsObject)> onKeyUp;

    virtual bool onEvent(MorphEvent& e) {
        JsObject evt;
        evt.set("x", JsNumber(e.x));
        evt.set("y", JsNumber(e.y));
        evt.set("button", JsNumber(e.button));
        evt.set("key", JsNumber(e.key));
        evt.set("scroll", JsNumber(e.scroll));
        {
            const char* tn = "unknown";
            switch (e.type) {
                case EventType::Click: tn = "click"; break;
                case EventType::DoubleClick: tn = "dblclick"; break;
                case EventType::MouseMove: tn = "mousemove"; break;
                case EventType::MouseDown: tn = "mousedown"; break;
                case EventType::MouseUp: tn = "mouseup"; break;
                case EventType::KeyUp: tn = "keyup"; break;
                case EventType::KeyDown: tn = "keydown"; break;
                case EventType::Scroll: tn = "scroll"; break;
                case EventType::Resize: tn = "resize"; break;
                case EventType::Focus: tn = "focus"; break;
                case EventType::Blur: tn = "blur"; break;
            }
            evt.set("type", JsString(tn));
        }
        if (e.type == EventType::Click && onClick) {
            onClick(evt);
            return true;
        }
        if (e.type == EventType::DoubleClick && onDoubleClick) {
            onDoubleClick(evt);
            return true;
        }
        if (e.type == EventType::MouseDown && onMouseDown) {
            onMouseDown(evt);
            return true;
        }
        if (e.type == EventType::MouseUp && onMouseUp) {
            onMouseUp(evt);
            return true;
        }
        if (e.type == EventType::KeyDown && onKeyDown) {
            onKeyDown(evt);
            return true;
        }
        if (e.type == EventType::KeyUp && onKeyUp) {
            onKeyUp(evt);
            return true;
        }
        return false;
    }
    virtual void onHover(bool state);

    // ── Hover transitions ──
    HoverTransition* m_hoverTransition = nullptr;
    void updateHoverTransition(float dt);
    static void interpolateStyles(MorphStyle& out, const MorphStyle& a,
                                  const MorphStyle& b, float t);

    // ── Ancestor-hover rules + transitions ──
    std::vector<AncestorHoverRule> m_ancestorHoverRules;
    AncestorHoverTransition* m_ancestorHoverTransition = nullptr;
    void _applyAncestorHover(bool state);
    void updateAncestorHoverTransition(float dt);

    void startAnimation(AnimProperty prop, float to, float duration, Easing easing = Easing::Linear);

    virtual float contentWidth(Renderer* r);
    MorphNode* hitTest(float ex, float ey);
    void addChild(MorphNode* child) {
        children.push_back(child);
        child->parent = this;
        child->markDirty(LayoutDirty);
        child->markDirty(PaintDirty);
    }
    void removeChild(MorphNode* child) {
        auto it = std::find(children.begin(), children.end(), child);
        if (it != children.end()) {
            children.erase(it);
            child->parent = nullptr;
            markDirty(SubtreeDirty);
        }
    }
    void removeAllChildren() {
        for (auto* c : children) c->parent = nullptr;
        children.clear();
        markDirty(SubtreeDirty);
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

    // Flatten into a lock-free render frame for the compositor thread
    virtual int flatten(RenderFrame& frame, int parentId);
    virtual int flattenExtra(RenderFrame& frame, FlatRenderNode& fn);

    virtual ~MorphNode() { delete hoverStyle; delete m_hoverTransition; delete m_ancestorHoverTransition; for (auto* child : children) delete child; }
};
