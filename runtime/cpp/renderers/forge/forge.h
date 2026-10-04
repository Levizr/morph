// renderers/forge/forge.h
#pragma once

#include <functional>
#include "../core/window.h"
#include "forge/damage.h"
#include "forge/layer.h"

namespace forge
{
    void forgeCommit(MorphWindow& win);
    // overlayFn is the devtools overlay draw callback (drawn on the default FB
    // after the damage-limited blit). Null unless the dev tools panel is active.
    void forgePresent(MorphWindow& win,
                      std::function<void(GLRenderer&, DirtyStats&)> overlayFn = {});
    // Force the next commit on every window to repaint fullscreen. Used when
    // the dev renderer toggle flips back to forge: the retained surface went
    // stale while flash presented straight to the backbuffer.
    void forceFullscreen();
    // Release per-window retained surfaces. Called while the window's GL
    // context is current (see MorphWindow::~MorphWindow).
    void forgetWindow(MorphWindow& win);
    // Single-mover layer fast path (v1): exactly one position-only leaf
    // becomes a retained layer instead of a re-raster. Pure frame walk,
    // no GL; pixel-tested. Returns the mover index + gates + base/new
    // screen boxes when eligible.
    bool tryMoverLayer(const RenderFrame* frame, int& outIdx, MoverLayerGates& gates,
                       DamageRect& baseBox, DamageRect& newBox);
    // Draw the current damage set as translucent rects on the caller's
    // target (devtools damage overlay). No-op when the window has no
    // committed damage. Dev-only; production binaries never call it.
    void drawDamageOverlay(MorphWindow& win, GLRenderer& r);
}