#include "../node.h"
#include "../renderer.h"
#include <cmath>
#include <cstring>
#include <chrono>

void MorphNode::markDirty(DirtyFlag f) {
    if (f == Clean) return;
    m_dirtyFlags |= f;
    if ((f == LayoutDirty || f == SubtreeDirty) && parent && !parent->isDirty(SubtreeDirty)) {
        parent->markDirty(SubtreeDirty);
    }
    if (f == PaintDirty && parent && !parent->isDirty(PaintDirty)) {
        parent->markDirty(PaintDirty);
    }
}

void MorphNode::layoutIfNeeded(float px, float py, float parentW, float parentH,
                                Renderer* r, DirtyStats* stats, bool force) {
    bool selfDirty = isDirty(LayoutDirty) || isDirty(StyleDirty);
    bool subtreeDirty = isDirty(SubtreeDirty);
    bool needsLayout = selfDirty || subtreeDirty || force;

    if (!needsLayout && stats) stats->skippedCount++;

    if (needsLayout) {
        if (stats) {
            stats->layoutCount++;
            if (selfDirty && !subtreeDirty)
                stats->paintCount++;
        }
        layout(px, py, parentW, parentH, r);
        clearDirty(LayoutDirty);
        clearDirty(StyleDirty);
        markDirty(PaintDirty);
    }

    for (auto* c : children) {
        float cw = c->w > 0 ? c->w : (parentW - c->x + px);
        float ch = c->h > 0 ? c->h : (parentH - c->y + py);
        c->layoutIfNeeded(c->x, c->y, cw, ch, r, stats, force || subtreeDirty);
    }
    if (needsLayout) clearDirty(SubtreeDirty);
}

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
            if (c->style.display == "inline" || c->type == "__text__") {
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
