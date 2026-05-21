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
    std::string overflow = "visible";
    float scrollbarWidth  = 8.0f;
    float scrollbarTrackColor[4] = {0.85f, 0.85f, 0.85f, 0.4f};
    float scrollbarThumbColor[4] = {0.5f, 0.5f, 0.5f, 0.6f};
    float scrollbarBorderRadius = 4.0f;
};

class Renderer;

class MorphNode {
public:
    float x = 0, y = 0, w = 0, h = 0;
    MorphStyle style;
    std::vector<MorphNode*> children;
    bool focused = false;

    // Scroll state
    float scrollY = 0;
    float contentH = 0;
    bool scrollEnabled = false;
    bool scrollThumbHover = false;
    bool scrollDragging = false;
    float scrollDragStartY = 0;
    float scrollDragStartVal = 0;

    virtual void layout(float px, float py, float parentW, float parentH,
                        Renderer* r = nullptr) {
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

        // Track max bottom for content height
        float maxBottom = 0.0f;
        for (auto* child : children) {
            child->layout(cx, cy, cw, 0.0f, r);
            cy += child->h + style.gap;
            float cb = child->y + child->h;
            if (cb > maxBottom) maxBottom = cb;
        }

        // Auto-height: expand to contain children if height not explicitly set
        if (style.explicitHeight < 0.0f && !children.empty()) {
            float lastBottom = cy - style.gap;
            float autoH = lastBottom - y + pb;
            if (autoH > h) h = autoH;
        }

        // Clamp auto-height to parent viewport when overflow is auto/scroll
        if (style.explicitHeight < 0.0f &&
            (style.overflow == "auto" || style.overflow == "scroll") &&
            parentH > 0.0f && h > parentH) {
            h = parentH;
        }

        // Compute scroll state
        contentH = maxBottom - y + pb;
        if (contentH < h) contentH = h;
        scrollEnabled = (style.overflow == "scroll") ||
                        (style.overflow == "auto" && contentH > h);
        if (scrollEnabled) {
            if (scrollY > contentH - h) scrollY = contentH - h;
            if (scrollY < 0) scrollY = 0;
        }
    }

    virtual void draw(Renderer& r) = 0;
    virtual bool onEvent(MorphEvent& e) { return false; }
    virtual void onHover(bool state) {}

    void addChild(MorphNode* child) { children.push_back(child); }

    // Dispatch event to deepest child containing (ex, ey); bubble up
    // Returns true if event was handled (stops bubble)
    bool dispatchEvent(MorphEvent& e, float ex, float ey) {
        bool inBounds = (ex >= x && ex <= x + w && ey >= y && ey <= y + h);

        // Handle scroll wheel (only within bounds)
        if (scrollEnabled && e.type == EventType::Scroll) {
            if (inBounds) {
                scrollY -= e.scroll * 40.0f;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > contentH - h) scrollY = contentH - h;
                return true;
            }
            // Not in bounds — don't consume, let children handle
        }

        // Handle scrollbar (only within bounds)
        if (scrollEnabled && inBounds) {
            float sw = style.scrollbarWidth;
            float trackX = x + w - sw;
            bool onScrollbar = (ex >= trackX && ex <= trackX + sw);
            if (onScrollbar && (e.type == EventType::MouseDown || e.type == EventType::Click)) {
                float thumbH = (h / contentH) * h;
                float thumbY = y + (scrollY / (contentH - h)) * (h - thumbH);
                if (ey >= thumbY && ey <= thumbY + thumbH) {
                    scrollDragging = true;
                    scrollDragStartY = ey;
                    scrollDragStartVal = scrollY;
                    return true;
                } else {
                    float page = h * 0.7f;
                    scrollY += (ey < thumbY) ? -page : page;
                    if (scrollY < 0) scrollY = 0;
                    if (scrollY > contentH - h) scrollY = contentH - h;
                    return true;
                }
            }
            if (e.type == EventType::MouseUp) {
                scrollDragging = false;
            }
            if (e.type == EventType::MouseMove && scrollDragging) {
                float thumbH = (h / contentH) * h;
                float dy = ey - scrollDragStartY;
                float range = contentH - h;
                float thumbRange = h - thumbH;
                if (thumbRange > 0) {
                    scrollY = scrollDragStartVal + (dy / thumbRange) * range;
                    if (scrollY < 0) scrollY = 0;
                    if (scrollY > range) scrollY = range;
                }
                return true;
            }
        }

        for (auto it = children.rbegin(); it != children.rend(); ++it) {
            auto* c = *it;
            float cy = c->y - (scrollEnabled ? scrollY : 0);
            if (ex >= c->x && ex <= c->x + c->w &&
                ey >= cy && ey <= cy + c->h) {
                // Viewport culling: skip children scrolled completely out of view
                if (scrollEnabled && (cy + c->h <= y || cy >= y + h))
                    continue;
                if (c->dispatchEvent(e, ex, ey + (scrollEnabled ? scrollY : 0)))
                    return true;
            }
        }
        return onEvent(e);
    }

    virtual ~MorphNode() {}
};
