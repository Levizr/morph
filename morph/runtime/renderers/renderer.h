#pragma once

#include <atomic>

enum class RenderMode : uint8_t {
    Flash = 0,
    Forge = 1
};

// ── Renderer mode resolution ─────────────────────────────
// Production: build-time only. kRenderMode is constexpr, so the `if` folds at
// compile time and the unselected renderer is eliminated — zero runtime branch,
// zero dead code, smallest binary.
//
// Dev: both renderers are compiled and a runtime toggle (g_renderMode) switches
// between them for experimentation. Dev binary size is irrelevant and the
// per-frame dispatch is one relaxed atomic read. The switch is dev-only.

// Production defines (see renderer.cpp)
// MORPH_RENDERER_FORGE in feature_set.py (dev: also MORPH_FEATURE_DEV_RENDERER_SWITCH)

RenderMode activeRenderMode();

#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
void setRenderMode(RenderMode m);
#endif
