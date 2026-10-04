#include "renderer.h"

#include <atomic>

// ── Dev-only runtime toggle ────────────────────────────────
#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
std::atomic<RenderMode> g_renderMode{RenderMode::Flash};
RenderMode activeRenderMode()
{
    return g_renderMode.load(std::memory_order_relaxed);
}
void setRenderMode(RenderMode m)
{
    g_renderMode.store(m, std::memory_order_relaxed);
}
#endif
