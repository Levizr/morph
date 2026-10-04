// renderers/forge/damage.cpp
#include "damage.h"

DamageRect DamageRect::intersection(const DamageRect& o) const
{
    int ix = std::max(x, o.x);
    int iy = std::max(y, o.y);
    int ix2 = std::min(right(), o.right());
    int iy2 = std::min(bottom(), o.bottom());
    if (ix2 <= ix || iy2 <= iy)
        return {0, 0, 0, 0};
    return {ix, iy, ix2 - ix, iy2 - iy};
}

int DamageSet::totalArea() const
{
    int a = 0;
    for (const auto& r : rects)
        a += r.w * r.h;
    return a;
}

bool DamageSet::intersects(const DamageRect& r) const
{
    if (fullScreen)
        return true;
    for (const auto& o : rects)
        if (o.intersects(r))
            return true;
    return false;
}

// Merge r into the set. If it overlaps an existing rect, absorb it into a
// bounding union (greedy but keeps the set compact); otherwise append.
void DamageSet::add(const DamageRect& r)
{
    if (fullScreen)
        return;
    if (r.w <= 0 || r.h <= 0)
        return;
    for (auto& o : rects)
    {
        if (o.intersects(r))
        {
            int x0 = std::min(o.x, r.x);
            int y0 = std::min(o.y, r.y);
            o.w = std::max(o.right(), r.right()) - x0;
            o.h = std::max(o.bottom(), r.bottom()) - y0;
            o.x = x0;
            o.y = y0;
            return;
        }
    }
    rects.push_back(r);
}

void DamageSet::merge(const DamageSet& o)
{
    if (o.fullScreen)
    {
        setFullScreen();
        return;
    }
    if (fullScreen)
        return;
    for (const auto& r : o.rects)
        add(r);
}

void DamageSet::clipTo(int vw, int vh)
{
    if (fullScreen)
    {
        rects.clear();
        rects.push_back({0, 0, vw, vh});
        return;
    }
    for (auto& r : rects)
    {
        int rx0 = std::max(0, r.x);
        int ry0 = std::max(0, r.y);
        int rx1 = std::min(vw, r.right());
        int ry1 = std::min(vh, r.bottom());
        r.x = rx0;
        r.y = ry0;
        r.w = rx1 - rx0;
        r.h = ry1 - ry0;
        if (r.w <= 0 || r.h <= 0)
            r.w = r.h = 0;
    }
    rects.erase(std::remove_if(rects.begin(), rects.end(),
                               [](const DamageRect& r) { return r.w <= 0 || r.h <= 0; }),
                rects.end());
}