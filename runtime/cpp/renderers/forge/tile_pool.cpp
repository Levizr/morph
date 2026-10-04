// renderers/forge/tile_pool.cpp
#include "tile_pool.h"

TilePool::TilePool(size_t budgetBytes)
    : m_budgetBytes(budgetBytes)
{
}

TilePool::~TilePool()
{
    clear();
}

GLuint TilePool::acquire(const TileKey& key)
{
    size_t need = tileBytes(key);
    if (need == 0 || need > m_budgetBytes)
    {
        return 0;
    }

    auto it = m_tiles.find(key);
    if (it != m_tiles.end())
    {
        forget(key);
        m_lru.push_back(key);
        return it->second.texture;
    }

    while (m_bytesUsed + need > m_budgetBytes && !m_lru.empty())
    {
        evictOldest();
    }

    // Residency accounting only: raster still flows through the single
    // retained FBO, so no GL texture is allocated here. The sentinel keeps
    // the pool a pure LRU/budget tracker with zero VRAM overhead until a
    // per-tile composite path consumes it.
    GLuint tex = 1;

    Tile tile;
    tile.key = key;
    tile.texture = tex;
    tile.epoch = m_epoch;
    tile.valid = true;
    m_tiles.emplace(key, tile);
    m_lru.push_back(key);
    m_bytesUsed += need;
    return tex;
}

void TilePool::invalidate(const TileKey& key)
{
    auto it = m_tiles.find(key);
    if (it == m_tiles.end())
    {
        return;
    }
    m_bytesUsed -= tileBytes(key);
    m_tiles.erase(it);
    forget(key);
    m_epoch++;
}

void TilePool::invalidateOverlapping(int x, int y, int w, int h)
{
    if (w <= 0 || h <= 0)
    {
        return;
    }
    for (auto it = m_tiles.begin(); it != m_tiles.end();)
    {
        if (it->first.overlaps(x, y, w, h))
        {
            m_bytesUsed -= tileBytes(it->first);
            forget(it->first);
            it = m_tiles.erase(it);
            m_epoch++;
        }
        else
        {
            ++it;
        }
    }
}

void TilePool::clear()
{
    m_tiles.clear();
    m_lru.clear();
    m_bytesUsed = 0;
    m_epoch++;
}

bool TilePool::contains(const TileKey& key) const
{
    return m_tiles.find(key) != m_tiles.end();
}

void TilePool::evictOldest()
{
    if (m_lru.empty())
    {
        return;
    }
    TileKey key = m_lru.front();
    m_lru.pop_front();
    auto it = m_tiles.find(key);
    if (it == m_tiles.end())
    {
        return;
    }
    m_bytesUsed -= tileBytes(key);
    m_tiles.erase(it);
}

void TilePool::forget(const TileKey& key)
{
    for (auto it = m_lru.begin(); it != m_lru.end(); ++it)
    {
        if (*it == key)
        {
            m_lru.erase(it);
            return;
        }
    }
}
