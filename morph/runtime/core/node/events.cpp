#include "../node.h"
#include <cmath>

MorphNode* MorphNode::hitTest(float ex, float ey) {
#ifdef MORPH_FEATURE_TRANSFORM
    float inv[16];
    morph::mat4Identity(inv);
    // Node x/y hold window-ABSOLUTE positions post-layout. Translate the
    // point into this node's local frame up front, so subtree-level calls
    // (dispatch routing, not just root queries) resolve correctly.
    inv[12] = -x;
    inv[13] = -y;
    return hitTestImpl(ex, ey, inv);
#else
    return hitTestImpl(ex, ey, nullptr);
#endif
}

// accInv is the inverse of the accumulated model transform of this node
// (identity matrix for the root). It maps screen coords into this node's
// local space, where its box is (0, 0, w, h) — so transformed subtrees are
// hit-tested against their actual (rotated/scaled) geometry.
//
// NOTE: a node's own box does NOT gate its subtree — absolutely-positioned
// descendants and zero-size conditional wrappers legitimately render (and
// receive hits) outside their parent's box. Children are checked first;
// the node itself is a candidate only for points inside its own box.
MorphNode* MorphNode::hitTestImpl(float ex, float ey, const float* accInv) {
    float lx = ex, ly = ey;
#ifdef MORPH_FEATURE_TRANSFORM
    if (accInv)
    {
        float ox, oy, oz;
        morph::mat4TransformPoint(accInv, ex, ey, 0.0f, ox, oy, oz);
        lx = ox;
        ly = oy;
    }
#endif
    const auto& po = paintOrder();
    for (auto it = po.rbegin(); it != po.rend(); ++it) {
        auto* c = *it;
        const float* childInvPtr;
        float childInv[16];
#ifdef MORPH_FEATURE_TRANSFORM
        if (accInv)
        {
            // A(child) = A(this) × T(0,-scrollY) × T(rel) × T(o) × M(child) ×
            // T(-o), so A(child)^-1 = T(o) × M^-1 × T(-o) × T(-rel) ×
            // T(0,+scrollY) × A(this)^-1.  o is the child's transform-origin
            // in its own box space (default center).
            float invScroll[16], invRel[16], invM[16], invO[16], posO[16],
                  t1[16], t2[16], t3[16];
            float s = (scrollEnabled && contentH > h) ? scrollY : 0.0f;
            morph::mat4Identity(invScroll);
            invScroll[13] = s;
            morph::mat4Identity(invRel);
            invRel[12] = x - c->x;
            invRel[13] = y - c->y;
            float ox = 0.0f, oy = 0.0f;
            if (c->style.transformSet)
            {
                morph::mat4Inverse(c->style.matrix, invM);
                ox = c->style.originX * c->w;
                oy = c->style.originY * c->h;
            }
            else
            {
                morph::mat4Identity(invM);
            }
            morph::mat4Identity(invO);
            invO[12] = -ox; invO[13] = -oy;
            morph::mat4Identity(posO);
            posO[12] = ox; posO[13] = oy;
            morph::mat4Multiply(t1, invScroll, accInv);
            morph::mat4Multiply(t2, invRel, t1);
            morph::mat4Multiply(t3, invO, t2);
            morph::mat4Multiply(t2, invM, t3);
            morph::mat4Multiply(childInv, posO, t2);
            childInvPtr = childInv;
        }
        else
#endif
        {
            childInvPtr = nullptr;
        }
        auto* found = c->hitTestImpl(ex, ey, childInvPtr);
        if (found) return found;
    }
    // Self hit: only for points inside THIS node's own box.
    bool inside;
#ifdef MORPH_FEATURE_TRANSFORM
    inside = accInv ? (lx >= 0.0f && lx <= w && ly >= 0.0f && ly <= h)
                    : (ex >= x && ex <= x + w && ey >= y && ey <= y + h);
#else
    inside = (ex >= x && ex <= x + w && ey >= y && ey <= y + h);
#endif
    return inside ? this : nullptr;
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
#ifdef MORPH_FEATURE_POSITION
                updateStickySubtree();
#endif
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
#ifdef MORPH_FEATURE_POSITION
                    updateStickySubtree();
#endif
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
#ifdef MORPH_FEATURE_POSITION
                updateStickySubtree();
#endif
            }
            return true;
        }
    }
#endif

    const auto& po = paintOrder();
    for (auto it = po.rbegin(); it != po.rend(); ++it) {
        auto* c = *it;
        float eyAdj = ey + (scrollEnabled ? scrollY : 0.0f);
        // Route by subtree hit, not own-box bounds: absolutely-positioned
        // content and zero-size conditional wrappers can render (and must
        // receive events) outside this child's box.
        if (!c->hitTest(ex, eyAdj))
            continue;
#ifdef MORPH_FEATURE_SCROLL
        float cy = c->y - (scrollEnabled ? scrollY : 0.0f);
        if (scrollEnabled && (cy + c->h <= y || cy >= y + h))
            continue;   // fully scrolled out of view
#endif
        if (c->dispatchEvent(e, ex, eyAdj))
            return true;
    }
    return onEvent(e);
}
