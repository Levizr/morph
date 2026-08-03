#include "../node.h"
#include "../renderer.h"
#include <cmath>
#include <cstring>
#include <chrono>

MorphNode* MorphNode::s_lastHoveredNode = nullptr;

void MorphNode::markDirty(DirtyFlag f) {
    if (f == Clean) return;
    m_dirtyFlags |= f;
    // Layout/subtree dirtiness genuinely affects ancestors (flex sizing,
    // auto-height, margin collapse), so propagate a SubtreeDirty marker up.
    // PaintDirty does NOT propagate: display lists are recorded in absolute
    // coordinates and a node's own ops never depend on its descendants, so a
    // child repaint must not force every ancestor to re-record its display list.
    if ((f == LayoutDirty || f == SubtreeDirty) && parent && !parent->isDirty(SubtreeDirty)) {
        parent->markDirty(SubtreeDirty);
    }
}

void MorphNode::layoutIfNeeded(float px, float py, float parentW, float parentH,
                                Renderer* r, DirtyStats* stats, bool force) {
    bool selfDirty = isDirty(LayoutDirty) || isDirty(StyleDirty);
    bool subtreeDirty = isDirty(SubtreeDirty);
    bool needsLayout = selfDirty || subtreeDirty || force;

    if (!needsLayout && stats) stats->skippedCount++;

    if (needsLayout) {
        if (stats) stats->layoutCount++;
        layout(px, py, parentW, parentH, r);
        clearDirty(LayoutDirty);
        clearDirty(StyleDirty);
#ifdef MORPH_FEATURE_DEV
        // Dev: paint dirtiness is decided by the geometry diff that runs after
        // this layout pass (window.cpp syncPaintDirtyTree) so unchanged nodes
        // that merely re-ran layout are not repainted.
#else
        markDirty(PaintDirty);
#endif
    }

    for (auto* c : children) {
        // layout() re-applies the child's own margins to px/py, so pass the
        // parent-assigned PRE-margin position and a margin-inclusive parent
        // width; otherwise re-layout double-applies the margins (frame 1).
        float cw = c->w > 0 ? (c->w + c->style.margin[3] + c->style.margin[1])
                            : (parentW - c->x + px);
        float ch = c->h > 0 ? c->h : (parentH - c->y + py);
        c->layoutIfNeeded(c->x - c->style.margin[3],
                          c->y - c->style.margin[0],
                          cw, ch, r, stats, force || subtreeDirty);
    }
    if (needsLayout) clearDirty(SubtreeDirty);
}

#ifdef MORPH_FEATURE_DEV
void MorphNode::syncPaintDirtyAfterLayout() {
    // First pass (or freshly (re)attached node): never recorded this box, so
    // force a fresh display list and snapshot the geometry.
    if (!m_hasPaintedOnce) {
        markDirty(PaintDirty);
        m_hasPaintedOnce = true;
        m_lastPaintX = x; m_lastPaintY = y;
        m_lastPaintW = w; m_lastPaintH = h;
        m_lastPaintContentH = contentH;
        m_lastPaintScrollY = scrollY;
        m_lastPaintScrollEnabled = scrollEnabled;
        return;
    }
    // A node whose absolute box (or scrollport) moved during layout must
    // re-record its display list — flatten() bakes absolute coordinates into
    // its ops, so a stale list would render at the old position. Geometry that
    // didn't change needs no repaint even though it may have re-run layout.
    if (x != m_lastPaintX || y != m_lastPaintY ||
        w != m_lastPaintW || h != m_lastPaintH ||
        contentH != m_lastPaintContentH ||
        scrollEnabled != m_lastPaintScrollEnabled ||
        scrollY != m_lastPaintScrollY) {
        markDirty(PaintDirty);
        m_lastPaintX = x; m_lastPaintY = y;
        m_lastPaintW = w; m_lastPaintH = h;
        m_lastPaintContentH = contentH;
        m_lastPaintScrollY = scrollY;
        m_lastPaintScrollEnabled = scrollEnabled;
    }
}
#endif

float MorphNode::contentWidth(Renderer* r) {
    float pl = style.padding[3], pr = style.padding[1];
#ifdef MORPH_FEATURE_BORDER
    float bw = style.borderWidth;
#else
    float bw = 0.0f;
#endif

    if (style.explicitWidth >= 0.0f) {
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == "border-box") {
            return style.explicitWidth;
        }
#endif
        return style.explicitWidth + pl + pr + bw * 2.0f;
    }

#ifdef MORPH_FEATURE_FLEX
    if (style.display == "flex" && style.flexDirection == "row") {
        float total = 0.0f;
        int count = 0;
        for (auto* c : children) {
            float cw = c->contentWidth(r);
            if (cw < 0.0f) return -1.0f;
            float cml = c->style.margin[3], cmr = c->style.margin[1];
            total += cw + cml + cmr;
            count++;
        }
        if (count > 1) total += (count - 1) * style.gap;
        return total + pl + pr + bw * 2.0f;
    }
#endif

#ifdef MORPH_FEATURE_INLINE
    {
        float totalInline = 0.0f;
        for (auto* c : children) {
            if (c->style.display == "inline" || c->style.display == "inline-block"
                || c->type == "__text__") {
                float cw = c->contentWidth(r);
                if (cw > 0.0f)
                    totalInline += cw + c->style.margin[3] + c->style.margin[1];
            }
        }
        if (totalInline > 0.0f) {
            return totalInline + pl + pr + bw * 2.0f;
        }
    }
#endif

    float maxCW = -1.0f;
    for (auto* c : children) {
#ifdef MORPH_FEATURE_DISPLAY_NONE
        if (c->style.display == "none") continue;
#endif
        float cw = c->contentWidth(r);
        if (cw > maxCW) maxCW = cw;
    }
    if (maxCW > -0.5f) {
        return maxCW + pl + pr + bw * 2.0f;
    }
    return -1.0f;
}
