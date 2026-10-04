// renderers/forge/layer.h
#pragma once

#include <cstdint>
#include <deque>
#include <unordered_map>
#include <vector>

#include "vendor/glad/glad.h"

namespace forge
{

struct RetainedLayer
{
    int m_nodeId = -1;
    GLuint m_texture = 0;
    GLuint m_fbo = 0;
    GLuint m_rbo = 0;
    int m_w = 0;
    int m_h = 0;
    bool m_active = false;
    uint64_t m_lastUsed = 0;
    // Captured appearance fingerprint (v1: single opaque Rect color).
    // Recapture when it drifts; otherwise the cached raster is reused
    // across moves without re-raster.
    float m_capR = 0.0f;
    float m_capG = 0.0f;
    float m_capB = 0.0f;
    float m_capA = 0.0f;
    bool m_captured = false;
};

inline constexpr size_t DEFAULT_LAYER_BUDGET_BYTES = 4 * 1024 * 1024;
inline constexpr size_t MAX_RETAINED_LAYERS = 8;

class LayerPool
{
public:
    explicit LayerPool(size_t budgetBytes = DEFAULT_LAYER_BUDGET_BYTES)
        : m_budgetBytes(budgetBytes)
    {
    }

    ~LayerPool()
    {
        clear();
    }

    LayerPool(const LayerPool&) = delete;
    LayerPool& operator=(const LayerPool&) = delete;

    bool shouldPromote(int nodeId, int w, int h, bool transitioning, bool colorAnim,
                       bool positionAnim = false) const
    {
        if (!transitioning && !colorAnim && !positionAnim)
        {
            return false;
        }
        if (w <= 0 || h <= 0 || w > 512 || h > 512)
        {
            return false;
        }
        if (nodeId < 0)
        {
            return false;
        }
        return true;
    }

    void update(int nodeId, int w, int h, uint64_t frameId)
    {
        size_t need = static_cast<size_t>(w) * static_cast<size_t>(h) * 4;
        if (need == 0 || need > m_budgetBytes)
        {
            return;
        }
        auto it = m_layers.find(nodeId);
        if (it != m_layers.end())
        {
            it->second.m_lastUsed = frameId;
            touch(nodeId);
            return;
        }
        while ((m_bytesUsed + need > m_budgetBytes || m_layers.size() >= MAX_RETAINED_LAYERS) &&
               !m_lru.empty())
        {
            evictOldest();
        }
        RetainedLayer layer;
        layer.m_nodeId = nodeId;
        layer.m_w = w;
        layer.m_h = h;
        layer.m_active = true;
        layer.m_lastUsed = frameId;
        m_layers.emplace(nodeId, layer);
        m_lru.push_back(nodeId);
        m_bytesUsed += need;
    }

    void prune(const std::vector<int>& liveIds, uint64_t frameId)
    {
        for (auto it = m_layers.begin(); it != m_layers.end();)
        {
            bool live = false;
            for (int id : liveIds)
            {
                if (id == it->first)
                {
                    live = true;
                    break;
                }
            }
            if (!live || frameId - it->second.m_lastUsed > 120)
            {
                m_bytesUsed -= static_cast<size_t>(it->second.m_w) *
                               static_cast<size_t>(it->second.m_h) * 4;
                if (it->second.m_texture != 0)
                {
                    glDeleteTextures(1, &it->second.m_texture);
                }
                if (it->second.m_rbo != 0)
                {
                    glDeleteRenderbuffers(1, &it->second.m_rbo);
                }
                if (it->second.m_fbo != 0)
                {
                    glDeleteFramebuffers(1, &it->second.m_fbo);
                }
                forget(it->first);
                it = m_layers.erase(it);
            }
            else
            {
                ++it;
            }
        }
    }

    void clear()
    {
        for (auto& [id, layer] : m_layers)
        {
            if (layer.m_texture != 0)
            {
                glDeleteTextures(1, &layer.m_texture);
                layer.m_texture = 0;
            }
            if (layer.m_rbo != 0)
            {
                glDeleteRenderbuffers(1, &layer.m_rbo);
                layer.m_rbo = 0;
            }
            if (layer.m_fbo != 0)
            {
                glDeleteFramebuffers(1, &layer.m_fbo);
                layer.m_fbo = 0;
            }
        }
        m_layers.clear();
        m_lru.clear();
        m_bytesUsed = 0;
    }

    bool contains(int nodeId) const
    {
        return m_layers.find(nodeId) != m_layers.end();
    }

    // Marks a present-side layer live for another frame without
    // resizing it (mover reuse across commits: commit prunes by its
    // own live set, so the saved mover key must be re-touched while
    // its position anim still runs).
    void refresh(int nodeId, uint64_t frameId)
    {
        auto it = m_layers.find(nodeId);
        if (it == m_layers.end())
        {
            return;
        }
        it->second.m_lastUsed = frameId;
        touch(nodeId);
    }

    // Drops one layer immediately (mover index changed: the old slot
    // can never be reused, so free its surface now, not in 120 frames).
    void drop(int nodeId)
    {
        auto it = m_layers.find(nodeId);
        if (it == m_layers.end())
        {
            return;
        }
        if (it->second.m_texture != 0)
        {
            glDeleteTextures(1, &it->second.m_texture);
        }
        if (it->second.m_rbo != 0)
        {
            glDeleteRenderbuffers(1, &it->second.m_rbo);
        }
        if (it->second.m_fbo != 0)
        {
            glDeleteFramebuffers(1, &it->second.m_fbo);
        }
        m_bytesUsed -= static_cast<size_t>(it->second.m_w) * static_cast<size_t>(it->second.m_h) * 4;
        forget(nodeId);
        m_layers.erase(it);
    }

    size_t realCount() const
    {
        size_t n = 0;
        for (const auto& [id, layer] : m_layers)
        {
            if (layer.m_fbo != 0)
            {
                n++;
            }
        }
        return n;
    }

    size_t realBytes() const
    {
        size_t n = 0;
        for (const auto& [id, layer] : m_layers)
        {
            if (layer.m_fbo != 0)
            {
                n += static_cast<size_t>(layer.m_w) * static_cast<size_t>(layer.m_h) * 4;
            }
        }
        return n;
    }

    RetainedLayer* find(int nodeId)
    {
        auto it = m_layers.find(nodeId);
        return it == m_layers.end() ? nullptr : &it->second;
    }

    const RetainedLayer* find(int nodeId) const
    {
        auto it = m_layers.find(nodeId);
        return it == m_layers.end() ? nullptr : &it->second;
    }

    // Ensures a GL surface (texture + FBO) for a layer. Returns the FBO,
    // or 0 when allocation fails. Reallocates when the size drifted.
    // No-op when w/h are invalid. Never called without a GL context
    // (present path only); pure tracking in update() stays GL-free so
    // the math suite links without a context.
    GLuint ensureSurface(int nodeId, int w, int h)
    {
        if (w <= 0 || h <= 0)
        {
            return 0;
        }
        auto it = m_layers.find(nodeId);
        if (it == m_layers.end())
        {
            return 0;
        }
        RetainedLayer& layer = it->second;
        if (layer.m_fbo != 0 && layer.m_w == w && layer.m_h == h)
        {
            return layer.m_fbo;
        }
        if (layer.m_texture != 0)
        {
            glDeleteTextures(1, &layer.m_texture);
            layer.m_texture = 0;
        }
        if (layer.m_rbo != 0)
        {
            glDeleteRenderbuffers(1, &layer.m_rbo);
            layer.m_rbo = 0;
        }
        if (layer.m_fbo != 0)
        {
            glDeleteFramebuffers(1, &layer.m_fbo);
            layer.m_fbo = 0;
        }
        layer.m_captured = false;
        glGenTextures(1, &layer.m_texture);
        glBindTexture(GL_TEXTURE_2D, layer.m_texture);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE,
                     nullptr);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
        glGenRenderbuffers(1, &layer.m_rbo);
        glBindRenderbuffer(GL_RENDERBUFFER, layer.m_rbo);
        glRenderbufferStorage(GL_RENDERBUFFER, GL_DEPTH24_STENCIL8, w, h);
        glGenFramebuffers(1, &layer.m_fbo);
        glBindFramebuffer(GL_FRAMEBUFFER, layer.m_fbo);
        glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D,
                               layer.m_texture, 0);
        glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_DEPTH_STENCIL_ATTACHMENT,
                                  GL_RENDERBUFFER, layer.m_rbo);
        bool ok = glCheckFramebufferStatus(GL_FRAMEBUFFER) == GL_FRAMEBUFFER_COMPLETE;
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
        if (!ok)
        {
            glDeleteTextures(1, &layer.m_texture);
            glDeleteRenderbuffers(1, &layer.m_rbo);
            glDeleteFramebuffers(1, &layer.m_fbo);
            layer.m_texture = 0;
            layer.m_rbo = 0;
            layer.m_fbo = 0;
            return 0;
        }
        layer.m_w = w;
        layer.m_h = h;
        return layer.m_fbo;
    }

    size_t layerCount() const
    {
        return m_layers.size();
    }

    size_t bytesUsed() const
    {
        return m_bytesUsed;
    }

private:
    void evictOldest()
    {
        if (m_lru.empty())
        {
            return;
        }
        int id = m_lru.front();
        m_lru.pop_front();
        auto it = m_layers.find(id);
        if (it == m_layers.end())
        {
            return;
        }
        if (it->second.m_texture != 0)
        {
            glDeleteTextures(1, &it->second.m_texture);
        }
        if (it->second.m_rbo != 0)
        {
            glDeleteRenderbuffers(1, &it->second.m_rbo);
        }
        if (it->second.m_fbo != 0)
        {
            glDeleteFramebuffers(1, &it->second.m_fbo);
        }
        m_bytesUsed -= static_cast<size_t>(it->second.m_w) * static_cast<size_t>(it->second.m_h) * 4;
        m_layers.erase(it);
    }

    void touch(int nodeId)
    {
        for (auto it = m_lru.begin(); it != m_lru.end(); ++it)
        {
            if (*it == nodeId)
            {
                m_lru.erase(it);
                m_lru.push_back(nodeId);
                return;
            }
        }
    }

    void forget(int nodeId)
    {
        for (auto it = m_lru.begin(); it != m_lru.end(); ++it)
        {
            if (*it == nodeId)
            {
                m_lru.erase(it);
                return;
            }
        }
    }

    size_t m_budgetBytes;
    size_t m_bytesUsed = 0;
    std::unordered_map<int, RetainedLayer> m_layers;
    std::deque<int> m_lru;
};

// Mover-layer eligibility (v1): position-only animated leaves cached in
// a small FBO and blitted instead of re-rastered. Every condition is a
// correctness gate — any failure falls back to the normal re-raster
// path, which is always correct. Pure CPU, no GL, unit-tested.
struct MoverLayerGates
{
    int m_w = 0;
    int m_h = 0;
    bool m_hasOffset = false;
    bool m_animOpacityOne = true;
    bool m_opacityOne = true;
    bool m_isLeaf = false;
    bool m_hasText = true;
    bool m_hasRadius = true;
    bool m_hasBorder = true;
    bool m_overflowVisible = false;
    bool m_scrollEnabled = true;
    bool m_hasLayoutTransition = true;
    bool m_transformed = true;
    bool m_singleRectOpaque = false;
    bool m_colorAnim = true;
    bool m_opacityAnim = true;
    bool m_radiusAnim = true;
    bool m_ancestorTransformed = true;
    bool m_ancestorRounded = true;
    bool m_ancestorScrolled = true;
    bool m_ancestorClipped = true;
    bool m_overlapsSibling = true;
    bool m_multipleMovers = true;
};

inline bool isMoverLayerEligible(const MoverLayerGates& g)
{
    if (!g.m_hasOffset)
    {
        return false;
    }
    if (!g.m_animOpacityOne || !g.m_opacityOne)
    {
        return false;
    }
    if (g.m_w <= 0 || g.m_h <= 0 || g.m_w > 512 || g.m_h > 512)
    {
        return false;
    }
    if (!g.m_isLeaf || g.m_hasText || g.m_hasRadius || g.m_hasBorder)
    {
        return false;
    }
    if (!g.m_overflowVisible || g.m_scrollEnabled || g.m_hasLayoutTransition ||
        g.m_transformed)
    {
        return false;
    }
    if (!g.m_singleRectOpaque)
    {
        return false;
    }
    if (g.m_colorAnim || g.m_opacityAnim || g.m_radiusAnim)
    {
        return false;
    }
    if (g.m_ancestorTransformed || g.m_ancestorRounded || g.m_ancestorScrolled ||
        g.m_ancestorClipped)
    {
        return false;
    }
    if (g.m_overlapsSibling || g.m_multipleMovers)
    {
        return false;
    }
    return true;
}

} // namespace forge
