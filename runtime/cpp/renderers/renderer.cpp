#include "renderer.h"

#include <atomic>

// ── Dev-only runtime toggle ────────────────────────────────
#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
std::atomic<RenderMode> g_renderMode;
RenderMode activeRenderMode() {
    return g_renderMode.load(std::memory_order_relaxed);
}
#else
// Production: resolved at compile time (no runtime overhead)
RenderMode activeRenderMode() {
    #ifdef MORPH_RENDERER_FORGE
    return RenderMode::Forge;
    #else
    return RenderMode::Flash;
    #endif
}
#endif

#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
void setRenderMode(RenderMode m) {
    g_renderMode.store(m, std::memory_order_relaxed);
}
#endif
