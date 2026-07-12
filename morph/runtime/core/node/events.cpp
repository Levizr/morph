#include "../node.h"
#include <cmath>

MorphNode* MorphNode::hitTest(float ex, float ey) {
    if (ex < x || ex > x + w || ey < y || ey > y + h) return nullptr;
    for (auto it = children.rbegin(); it != children.rend(); ++it) {
        auto* c = *it;
        float cy = c->y - (scrollEnabled ? scrollY : 0);
        if (ex >= c->x && ex <= c->x + c->w &&
            ey >= cy && ey <= cy + c->h) {
            auto* found = c->hitTest(ex, ey + (scrollEnabled ? scrollY : 0));
            if (found) return found;
        }
    }
    return this;
}

bool MorphNode::dispatchEvent(MorphEvent& e, float ex, float ey) {
    bool inBounds = (ex >= x && ex <= x + w && ey >= y && ey <= y + h);

#ifdef MORPH_FEATURE_SCROLL
    if (scrollEnabled && e.type == EventType::Scroll) {
        if (inBounds) {
            float oldScrollY = scrollY;
            scrollY -= e.scroll * 40.0f;
            if (scrollY < 0) scrollY = 0;
            if (scrollY > contentH - h) scrollY = contentH - h;
            if (scrollY != oldScrollY) {
                markDirty(PaintDirty);
                for (auto* c : children) c->markDirty(PaintDirty);
            }
            return true;
        }
    }

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
                float oldScrollY = scrollY;
                scrollY += (ey < thumbY) ? -page : page;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > contentH - h) scrollY = contentH - h;
                if (scrollY != oldScrollY) {
                    markDirty(PaintDirty);
                    for (auto* c : children) c->markDirty(PaintDirty);
                }
                return true;
            }
        }
        if (e.type == EventType::MouseUp) {
            scrollDragging = false;
        }
        if (e.type == EventType::MouseMove && scrollDragging) {
            float oldScrollY = scrollY;
            float thumbH = (h / contentH) * h;
            float dy = ey - scrollDragStartY;
            float range = contentH - h;
            float thumbRange = h - thumbH;
            if (thumbRange > 0) {
                scrollY = scrollDragStartVal + (dy / thumbRange) * range;
                if (scrollY < 0) scrollY = 0;
                if (scrollY > range) scrollY = range;
            }
            if (scrollY != oldScrollY) {
                markDirty(PaintDirty);
                for (auto* c : children) c->markDirty(PaintDirty);
            }
            return true;
        }
    }
#endif

    for (auto it = children.rbegin(); it != children.rend(); ++it) {
        auto* c = *it;
        float cy = c->y - (scrollEnabled ? scrollY : 0);
        if (ex >= c->x && ex <= c->x + c->w &&
            ey >= cy && ey <= cy + c->h) {
#ifdef MORPH_FEATURE_SCROLL
            if (scrollEnabled && (cy + c->h <= y || cy >= y + h))
                continue;
#endif
            if (c->dispatchEvent(e, ex, ey + (scrollEnabled ? scrollY : 0)))
                return true;
        }
    }
    return onEvent(e);
}
