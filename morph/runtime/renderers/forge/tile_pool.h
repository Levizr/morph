// renderers/forge/tile_pool.h
#pragma once

#include <unordered_map>
#include <deque>
#include "tile.h"

class TilePool {
public:
    TilePool(size_t budgetBytes = 16 * 1024 * 1024);
    GLuint acquire(const TileKey& key);
    void invalidate(const TileKey& key);
    void clear() { m_tiles.clear(); m_lru.clear(); }
private:
    uint64_t m_epoch = 0;
    size_t m_budgetBytes;
    std::unordered_map<TileKey, Tile> m_tiles;
    std::deque<TileKey> m_lru;
};
