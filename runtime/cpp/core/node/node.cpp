#include "../node.h"
#include "../renderer.h"
#include <cmath>
#include <cstring>
#include <chrono>

MorphNode* MorphNode::s_lastHoveredNode = nullptr;
MorphNode* MorphNode::s_focusedNode = nullptr;
MorphNode* MorphNode::s_mouseCapture = nullptr;
MorphNode* MorphNode::s_activePressNode = nullptr;
MorphNode* MorphNode::s_lastClickNode = nullptr;

// ── Font-relative unit support ─────────────────────────────────────
// Element font for `em/ex/ch`: the parent's computed size (16 at the
// root). Chains resolve on demand so pre-layout readers (intrinsic
// sizing, paint before first layout) agree with layout.
float MorphNode::parentFontSize() const {
    if (!parent) return 16.0f;
    if (parent->m_computedFontSize >= 0.0f) return parent->m_computedFontSize;
    return parent->computeFontSize(nullptr);
}

// Root font for `rem`: the outermost node's computed size (html/body
// `font-size` sets it); 16 until the root itself resolves.
float MorphNode::rootFontSize() const {
    const MorphNode* top = this;
    while (top->parent) top = top->parent;
    if (top != this && top->m_computedFontSize >= 0.0f) return top->m_computedFontSize;
    return 16.0f;
}

float MorphNode::computeFontSize(Renderer* r) const {
    float parentF = parentFontSize();
    // Unspecified font inherits the parent's computed size (the codegen
    // bakes this in for static trees; dynamic/hand-built trees need it
    // at resolve time). Mirrors the old _effFontSize rule exactly.
    if (style.fontSize == pxLen(16.0f) && parentF != 16.0f) {
        m_computedFontSize = parentF;
        return parentF;
    }
    UnitEnv e;
    e.pctBase = parentF; // `%` font-size resolves against the parent font
    e.fontSize = parentF;
    e.rootFont = rootFontSize();
#ifdef MORPH_FEATURE_POSITION
    e.vw = m_winW;
    e.vh = m_winH;
#endif
    if (r && parentF > 0.0f) {
        e.chW = r->measureTextWidth("0", parentF, style.fontWeight);
        e.exW = r->measureTextWidth("x", parentF, style.fontWeight);
    }
    m_computedFontSize = resolveUnits(style.fontSize, e);
    return m_computedFontSize;
}

UnitEnv MorphNode::unitEnv(float pctBase, Renderer* r) const {
    if (m_computedFontSize < 0.0f) computeFontSize(r);
    UnitEnv e;
    e.pctBase = pctBase;
    e.fontSize = m_computedFontSize < 0.0f ? 16.0f : m_computedFontSize;
    e.rootFont = rootFontSize();
#ifdef MORPH_FEATURE_POSITION
    e.vw = m_winW;
    e.vh = m_winH;
#endif
    if (r && e.fontSize > 0.0f) {
        e.chW = r->measureTextWidth("0", e.fontSize, style.fontWeight);
        e.exW = r->measureTextWidth("x", e.fontSize, style.fontWeight);
    }
    return e;
}

#ifdef MORPH_FEATURE_BORDER
float MorphNode::resolvedBorderWidth() const {
    return resolveUnits(style.borderWidth, unitEnv(0.0f, nullptr));
}
#endif

void MorphNode::resolvedRadii(float out[4], bool snap) const {
    resolveBorderRadii(style, unitEnv(w, nullptr), snap, out);
}

void MorphNode::resolvedControlRadii(float out[4], bool snap) const {
    resolveControlRadii(style, unitEnv(w, nullptr), snap, out);
}

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
        // Relative offsets double-apply the same way, so back out the
        // stored offset and pass the flow slot instead.
        //
        // Only propagate force when this node did NOT just lay out its
        // subtree itself: layout() lays out every child directly, clearing
        // their flags. Forcing them to re-run would redundantly recompute —
        // and a child's own layout() resets auto sizes (h = 0 with no
        // explicit height), wiping parent-assigned flex grow/stretch sizes
        // that the re-layout cannot reconstruct (own height ignores the
        // passed parentH). Children that are genuinely dirty still run via
        // their own flags.
        bool propagateForce = (force || subtreeDirty) && !needsLayout;
        // Margins resolve through the child's own environment (`%` against
        // the width passed down, like layout's own base).
        UnitEnv cEnv = c->unitEnv(parentW, r);
        float cml = resolveUnits(c->style.margin[3], cEnv);
        float cmt = resolveUnits(c->style.margin[0], cEnv);
        float cpx = c->x - cml;
        float cpy = c->y - cmt;
#ifdef MORPH_FEATURE_POSITION
        if (c->style.position == CSS::Position::Relative) {
            cpx -= c->m_relOffX;
            cpy -= c->m_relOffY;
        }
#endif
        float cw = c->w > 0 ? (c->w + cml + resolveUnits(c->style.margin[1], cEnv))
                            : (parentW - c->x + px);
        float ch = c->h > 0 ? c->h : (parentH - c->y + py);
        c->layoutIfNeeded(cpx, cpy, cw, ch, r, stats, propagateForce);
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
    // Intrinsic sizing has no containing block yet: `%` has no base and
    // resolves to 0 here (final layout re-resolves with the real base);
    // every other unit is exact. Font units use this node's own font.
    UnitEnv env = unitEnv(0.0f, r);
    float pl = resolveUnits(style.padding[3], env);
    float pr = resolveUnits(style.padding[1], env);
#ifdef MORPH_FEATURE_BORDER
    float bwH = borderOuterH(style, env);
#else
    float bwH = 0.0f;
#endif

    if (style.explicitWidth.isSet() && !style.explicitWidth.isPercent()) {
#ifdef MORPH_FEATURE_BORDER_BOX
        if (style.boxSizing == CSS::BoxSizing::BorderBox) {
            return resolveUnits(style.explicitWidth, env);
        }
#endif
        return resolveUnits(style.explicitWidth, env) + pl + pr + bwH;
    }

#ifdef MORPH_FEATURE_FLEX
    if (style.display == CSS::Display::Flex && style.flexDirection == CSS::FlexDirection::Row) {
        float total = 0.0f;
        int count = 0;
        for (auto* c : children) {
            float cw = c->contentWidth(r);
            if (cw < 0.0f) return -1.0f;
            UnitEnv cEnv = c->unitEnv(0.0f, r);
            float cml = resolveUnits(c->style.margin[3], cEnv);
            float cmr = resolveUnits(c->style.margin[1], cEnv);
            total += cw + cml + cmr;
            count++;
        }
        if (count > 1) total += (count - 1) * resolveUnits(style.gap, env);
        return total + pl + pr + bwH;
    }
#endif

#ifdef MORPH_FEATURE_INLINE
    {
        float totalInline = 0.0f;
        for (auto* c : children) {
            if (c->style.display == CSS::Display::Inline || c->style.display == CSS::Display::InlineBlock
                || c->type == NodeType::Text || c->type == NodeType::Expr) {
                float cw = c->contentWidth(r);
                if (cw > 0.0f) {
                    UnitEnv cEnv = c->unitEnv(0.0f, r);
                    totalInline += cw + resolveUnits(c->style.margin[3], cEnv)
                        + resolveUnits(c->style.margin[1], cEnv);
                }
            }
        }
        if (totalInline > 0.0f) {
            return totalInline + pl + pr + bwH;
        }
    }
#endif

    float maxCW = -1.0f;
    for (auto* c : children) {
#ifdef MORPH_FEATURE_DISPLAY_NONE
        if (c->style.display == CSS::Display::None) continue;
#endif
        float cw = c->contentWidth(r);
        if (cw > maxCW) maxCW = cw;
    }
    if (maxCW > -0.5f) {
        return maxCW + pl + pr + bwH;
    }
    return -1.0f;
}
