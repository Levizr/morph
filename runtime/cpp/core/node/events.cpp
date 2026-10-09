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
#ifdef MORPH_FEATURE_POSITION
    // Viewport-locked fixed boxes don't move with ancestor scroll, but the
    // dispatch path added every scrolling ancestor's scroll into the point
    // on the way down: back it out for this subtree (locked by a transformed
    // ancestor → keep today's behavior).
    if (!accInv && style.position == CSS::Position::Fixed && !hasTransformedAncestor()) {
        for (MorphNode* p = parent; p; p = p->parent) {
            if (p->scrollXEnabled) ex -= p->scrollX;
            if (p->scrollYEnabled) ey -= p->scrollY;
        }
    }
#endif
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
            // A(child) = A(this) × T(-scrollX,-scrollY) × T(rel) × T(o) × M(child) ×
            // T(-o), so A(child)^-1 = T(o) × M^-1 × T(-o) × T(-rel) ×
            // T(+scrollX,+scrollY) × A(this)^-1.  o is the child's transform-origin
            // in its own box space (default center).
            float invScroll[16], invRel[16], invM[16], invO[16], posO[16],
                  t1[16], t2[16], t3[16];
            float sx = (scrollXEnabled && contentW > w) ? scrollX : 0.0f;
            float s = (scrollYEnabled && contentH > h) ? scrollY : 0.0f;
            morph::mat4Identity(invScroll);
            invScroll[12] = sx;
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

#ifdef MORPH_FEATURE_SCROLL
void MorphNode::scrollDragTo(float ex, float ey)
{
    if (!scrollDragging)
    {
        return;
    }
    if (scrollDragX)
    {
        if (!scrollXEnabled)
            return;
        float oldScrollX = scrollX;
        float thumbW = (w / contentW) * w;
        float dx = ex - scrollDragStartX;
        float range = contentW - w;
        float thumbRange = w - thumbW;
        if (thumbRange > 0)
        {
            scrollX = scrollDragStartVal + (dx / thumbRange) * range;
            if (scrollX < 0) scrollX = 0;
            if (scrollX > range) scrollX = range;
        }
        if (scrollX != oldScrollX)
        {
            markDirty(PaintDirty);
            for (auto* c : children) c->markDirty(PaintDirty);
        }
        return;
    }
    if (!scrollYEnabled)
    {
        return;
    }
    float oldScrollY = scrollY;
    float thumbH = (h / contentH) * h;
    float dy = ey - scrollDragStartY;
    float range = contentH - h;
    float thumbRange = h - thumbH;
    if (thumbRange > 0)
    {
        scrollY = scrollDragStartVal + (dy / thumbRange) * range;
        if (scrollY < 0) scrollY = 0;
        if (scrollY > range) scrollY = range;
    }
    if (scrollY != oldScrollY)
    {
        markDirty(PaintDirty);
        for (auto* c : children) c->markDirty(PaintDirty);
#ifdef MORPH_FEATURE_POSITION
        updateStickySubtree();
#endif
    }
}
#endif

#ifdef MORPH_FEATURE_SCROLL
// Wheel scroll for one level: consumes each axis independently into the
// event (zeroed when this box actually moved) so the remainder chains to
// the parent, like browsers. Axes already at their limit — or axes this
// box cannot scroll — keep their remainder for an ancestor to try.
void MorphNode::scrollWheel(MorphEvent& e)
{
    float dx = e.scrollX, dy = e.scroll;
    if ((e.mods & 0x01) != 0 && dy != 0.0f && dx == 0.0f && scrollXEnabled) {
        // Shift+wheel scrolls horizontally like browsers, but only a box
        // that can take horizontal scroll converts the delta; otherwise
        // the vertical delta travels on untouched for an ancestor to map.
        // The converted vertical part is consumed here — only the
        // horizontal remainder chains outward.
        dx = dy;
        dy = 0.0f;
        e.scroll = 0.0f;
    }
    bool moved = false;
    if (dy != 0.0f && scrollYEnabled && contentH > h) {
        float v = scrollY - dy * 40.0f;
        if (v < 0.0f) v = 0.0f;
        if (v > contentH - h) v = contentH - h;
        if (v != scrollY) {
            scrollY = v;
            e.scroll = 0.0f;
            moved = true;
        }
    }
    if (dx != 0.0f && scrollXEnabled && contentW > w) {
        float v = scrollX - dx * 40.0f;
        if (v < 0.0f) v = 0.0f;
        if (v > contentW - w) v = contentW - w;
        if (v != scrollX) {
            scrollX = v;
            e.scrollX = 0.0f;
            moved = true;
        }
    }
    if (moved) {
        markDirty(PaintDirty);
        for (auto* c : children) c->markDirty(PaintDirty);
#ifdef MORPH_FEATURE_POSITION
        updateStickySubtree();
#endif
    }
}
#endif

bool MorphNode::dispatchEvent(MorphEvent& e, float ex, float ey) {
    bool inBounds = (ex >= x && ex <= x + w && ey >= y && ey <= y + h);

#ifdef MORPH_FEATURE_SCROLL
    // Wheel scrolling itself is routed deepest-first below (with chaining),
    // so this block only handles direct scrollbar manipulation. Both axes
    // get a thumb drag and track paging; moves flow through the capture
    // path while dragging.
    if ((scrollYEnabled || scrollXEnabled) && inBounds) {
        float sw = style.scrollbarWidth;
        if (scrollYEnabled) {
            float trackX = x + w - sw;
            bool onScrollbar = (ex >= trackX && ex <= trackX + sw);
            if (onScrollbar && e.type == EventType::MouseDown) {
                float thumbH = (contentH > 0.0f) ? (h / contentH) * h : h;
                if (thumbH > h) thumbH = h;
                float thumbY = y;
                if (contentH > h)
                    thumbY = y + (scrollY / (contentH - h)) * (h - thumbH);
                if (ey >= thumbY && ey <= thumbY + thumbH) {
                    scrollDragging = true;
                    scrollDragX = false;
                    scrollDragStartY = ey;
                    scrollDragStartVal = scrollY;
                    // Capture like <input> drag-selection: moves keep flowing
                    // past the box edges, and the release always finds this
                    // node — otherwise a release outside the box leaves the
                    // flag set and the thumb follows the cursor forever.
                    s_mouseCapture = this;
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
        }
        if (scrollXEnabled) {
            // The corner square belongs to the vertical bar; the horizontal
            // track stops short of it when the vertical bar shows.
            bool vBar = scrollYEnabled && contentH > h;
            float trackY = y + h - sw;
            float trackW = w - (vBar ? sw : 0.0f);
            bool onHBar = (ey >= trackY && ey <= trackY + sw &&
                           ex >= x && ex <= x + trackW);
            if (onHBar && e.type == EventType::MouseDown) {
                float thumbW = (contentW > 0.0f) ? (trackW / contentW) * trackW : trackW;
                if (thumbW > trackW) thumbW = trackW;
                float thumbX = x;
                if (contentW > w)
                    thumbX = x + (scrollX / (contentW - w)) * (trackW - thumbW);
                if (ex >= thumbX && ex <= thumbX + thumbW) {
                    scrollDragging = true;
                    scrollDragX = true;
                    scrollDragStartX = ex;
                    scrollDragStartVal = scrollX;
                    s_mouseCapture = this;
                    return true;
                } else {
                    float page = w * 0.7f;
                    float oldScrollX = scrollX;
                    scrollX += (ex < thumbX) ? -page : page;
                    if (scrollX < 0) scrollX = 0;
                    if (scrollX > contentW - w) scrollX = contentW - w;
                    if (scrollX != oldScrollX) {
                        markDirty(PaintDirty);
                        for (auto* c : children) c->markDirty(PaintDirty);
                    }
                    return true;
                }
            }
        }
        if (e.type == EventType::MouseUp) {
            scrollDragging = false;
        }
        if (e.type == EventType::MouseMove && scrollDragging) {
            scrollDragTo(ex, ey);
            return true;
        }
    }
#endif

    // Child routing shared by the wheel path (deepest-first, below) and
    // the general path (after scrollbar handling). Points travel in each
    // child's unscrolled space — this level's offsets added back — matching
    // hitTestImpl, which expects root-space coords plus ancestor scroll.
    auto routeToChildren = [&]() -> bool {
        const auto& po = paintOrder();
        for (auto it = po.rbegin(); it != po.rend(); ++it) {
            auto* c = *it;
            float exAdj = ex + (scrollXEnabled ? scrollX : 0.0f);
            float eyAdj = ey + (scrollYEnabled ? scrollY : 0.0f);
            // Route by subtree hit, not own-box bounds: absolutely-positioned
            // content and zero-size conditional wrappers can render (and must
            // receive events) outside this child's box.
            if (!c->hitTest(exAdj, eyAdj))
                continue;
#ifdef MORPH_FEATURE_SCROLL
            // A viewport-locked fixed subtree is positioned in viewport coords
            // and can be visible outside this scroller's box: never cull it by
            // scrolled-out bounds (locked by transform → cull normally).
            bool fixedBelow = false;
#ifdef MORPH_FEATURE_POSITION
            if (c->m_subtreeHasFixed && !c->hasTransformedAncestor()) fixedBelow = true;
#endif
            if (!fixedBelow) {
                if (scrollYEnabled) {
                    float cy = c->y - scrollY;
                    if (cy + c->h <= y || cy >= y + h)
                        continue;   // fully scrolled out of view
                }
                if (scrollXEnabled) {
                    float cx = c->x - scrollX;
                    if (cx + c->w <= x || cx >= x + w)
                        continue;   // fully scrolled out of view
                }
            }
#endif
            if (c->dispatchEvent(e, exAdj, eyAdj))
                return true;
        }
        return false;
    };

    if (e.type == EventType::Scroll) {
#ifdef MORPH_FEATURE_SCROLL
        // Wheel routes deepest-first with chaining: each level consumes
        // what it can (zeroing its axes in the event) and the remainder
        // bubbles outward, like browsers. A fully-consumed wheel stops;
        // leftovers reach onEvent (pageX/Y and deltaX/Y carry the remainder
        // for JS wheel listeners) and keep bubbling past silent levels.
        if (routeToChildren())
            return true;
        if (inBounds)
            scrollWheel(e);
        if (e.scroll == 0.0f && e.scrollX == 0.0f)
            return true;
#endif
        return onEvent(e);
    }

    if (routeToChildren())
        return true;
    return onEvent(e);
}
