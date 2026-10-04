// renderers/forge/tile_pool.h
#pragma once

#include <cstddef>
#include <cstdint>
#include <deque>
#include <unordered_map>
#include "tile.h"

inline constexpr size_t DEFAULT_TILE_BUDGET_BYTES = 16 * 1024 * 1024;

class TilePool {
public:
    explicit TilePool(size_t budgetBytes = DEFAULT_TILE_BUDGET_BYTES);
    ~TilePool();

    TilePool(const TilePool&) = delete;
    TilePool& operator=(const TilePool&) = delete;

    GLuint acquire(const TileKey& key);
    void invalidate(const TileKey& key);
    void invalidateOverlapping(int x, int y, int w, int h);
    void clear();

    bool contains(const TileKey& key) const;
    size_t bytesUsed() const { return m_bytesUsed; }
    size_t tileCount() const { return m_tiles.size(); }
    uint64_t epoch() const { return m_epoch; }
private:
    void evictOldest();
    void forget(const TileKey& key);

    uint64_t m_epoch = 0;
    size_t m_budgetBytes;
    size_t m_bytesUsed = 0;
    std::unordered_map<TileKey, Tile, TileKeyHash> m_tiles;
    std::deque<TileKey> m_lru;
};
