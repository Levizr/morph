// renderers/forge/damage.h
#pragma once

#include <vector>
#include <algorithm>
#include <cstdint>

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

// Unioned damage set. Fullscreen flag forces everything (untrackable changes).
struct DamageSet {
    std::vector<DamageRect> rects;
    bool fullScreen = false;

    bool empty() const { return !fullScreen && rects.empty(); }
    int totalArea() const;
    bool intersects(const DamageRect& r) const;
    void add(const DamageRect& r);
    void merge(const DamageSet& o);
    void clipTo(int vw, int vh);                // viewport clip
    void setFullScreen() { fullScreen = true; rects.clear(); }
};

// New screen box of a compositor-offset mover: base box shifted by the
// interpolated offset, with the 2px safety margin baked in (present-added
// rects skip the commit margin loop, so they carry their own).
inline DamageRect moverBox(const DamageRect& base, float dx, float dy)
{
    return {base.x + static_cast<int>(dx) - 2, base.y + static_cast<int>(dy) - 2,
            base.w + 4, base.h + 4};
}

// Travel span of a mover: bounding box of the base and shifted boxes,
// with the same 2px margin. The layer fast path repaints the span so
// every intermediate position (previous retained pixels included) is
// covered even when offsets jump between frames; the plain re-raster
// path keeps moverBox (old comes from commit damage, steps are small).
inline DamageRect moverSpan(const DamageRect& base, float dx, float dy)
{
    int nx = base.x + static_cast<int>(dx);
    int ny = base.y + static_cast<int>(dy);
    int sx = base.x < nx ? base.x : nx;
    int sy = base.y < ny ? base.y : ny;
    int ex = base.x + base.w > nx + base.w ? base.x + base.w : nx + base.w;
    int ey = base.y + base.h > ny + base.h ? base.y + base.h : ny + base.h;
    return {sx - 2, sy - 2, ex - sx + 4, ey - sy + 4};
}