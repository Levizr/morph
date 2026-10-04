#pragma once

#include <atomic>
#include <cstdint>

enum class RenderMode : uint8_t
{
    Flash = 0,
    Forge = 1
};

// ── Renderer mode resolution ─────────────────────────────
// Production: build-time only. kRenderMode is constexpr, so `if constexpr`
// folds at compile time and the unselected renderer is eliminated — zero
// runtime branch, zero dead code, smallest binary.
//
// Dev: both renderers are compiled and a runtime toggle (g_renderMode)
// switches between them for experimentation. Dev binary size is irrelevant
// and the per-frame dispatch is one relaxed atomic read. The switch is
// dev-only.

// Production defines (see crates/morph-codegen/src/feature_set.rs)
// MORPH_RENDERER_FORGE in feature_set.rs (dev: also MORPH_FEATURE_DEV_RENDERER_SWITCH)

#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
RenderMode activeRenderMode();
void setRenderMode(RenderMode m);
#else
#ifdef MORPH_RENDERER_FORGE
inline constexpr RenderMode kRenderMode = RenderMode::Forge;
#else
inline constexpr RenderMode kRenderMode = RenderMode::Flash;
#endif
inline constexpr RenderMode activeRenderMode()
{
    return kRenderMode;
}
#endif
