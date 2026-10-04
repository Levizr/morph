// renderers/forge/tile.h
#pragma once

#include <cstdint>
#include <functional>
#include "vendor/glad/glad.h"

struct TileKey {
    int parentLayerId;
    int x, y, w, h;

    bool operator==(const TileKey& o) const
    {
        return parentLayerId == o.parentLayerId && x == o.x && y == o.y && w == o.w &&
               h == o.h;
    }

    bool overlaps(int ox, int oy, int ow, int oh) const
    {
        return x < ox + ow && ox < x + w && y < oy + oh && oy < y + h;
    }
};

struct TileKeyHash {
    size_t operator()(const TileKey& k) const noexcept
    {
        size_t h = static_cast<size_t>(k.parentLayerId);
        h = h * 31 + static_cast<size_t>(k.x);
        h = h * 31 + static_cast<size_t>(k.y);
        h = h * 31 + static_cast<size_t>(k.w);
        h = h * 31 + static_cast<size_t>(k.h);
        return h;
    }
};

struct Tile {
    TileKey key;
    GLuint texture = 0;
    uint64_t epoch = 0;
    bool opaque = false;
    bool valid = false;
};

inline size_t tileBytes(const TileKey& k)
{
    if (k.w <= 0 || k.h <= 0)
        return 0;
    return static_cast<size_t>(k.w) * static_cast<size_t>(k.h) * 4;
}
