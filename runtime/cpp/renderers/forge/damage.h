// renderers/forge/damage.h
#pragma once

#include <vector>
#include <algorithm>
#include <cstdint>
#include "../core/node.h"

// Screen-space damage rectangle (integer pixel coords, top-left origin).
struct DamageRect {
    int x, y, w, h;

    int right() const { return x + w; }
    int bottom() const { return y + h; }
    bool intersects(const DamageRect& o) const
    {
        return x < o.right() && o.x < right() && y < o.bottom() && o.y < bottom();
    }
    DamageRect intersection(const DamageRect& o) const;
};

// Screen-space box of a node: layout coords exclude scroll, but damage
// rects are consumed as screen regions (scissor clears, cull tests), so
// subtract every actively-scrolling ancestor's offset — mirroring
// renderNode's accumulated scrollOffset (help/bug-report#1.4).
inline DamageRect screenBox(const MorphNode* n)
{
    float sy = n->y;
    for (const MorphNode* a = n->parent; a; a = a->parent)
    {
        if (a->scrollEnabled && a->contentH > a->h)
            sy -= a->scrollY;
    }
    return {(int)n->x, (int)sy, (int)n->w, (int)n->h};
};

// Unioned damage set. Fullscreen flag forces everything (untrackable changes).
struct DamageSet {
    std::vector<DamageRect> rects;
    bool fullScreen = false;

    bool empty() const { return !fullScreen && rects.empty(); }
    int totalArea() const;
    bool intersects(const DamageRect& r) const;
    void add(const DamageRect& r);
    void add(MorphNode* node);                  // conservative: node bounds + clip ancestors
    void addAll(MorphNode* root);               // from a dirty tree
    void merge(const DamageSet& o);
    void clipTo(int vw, int vh);                // viewport clip
    void setFullScreen() { fullScreen = true; rects.clear(); }
};