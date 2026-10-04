# Forge Renderer — Tile Pool, Retained Layers, Scroll-Shift

**Status:** shipped · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

The `forge` renderer (retained FBO + damage tracking) is shipped and
opt-in per app. This page documents the phases that turned it into a
production renderer: **content-keyed tile pool**, **per-node retained
layers**, and **scroll-shift tile reuse**, plus the closing verification
(benchmarks, examples, DevTools overlay). Full design:
`help/renderer-flash-forge.md` and `help/hybrid-renderer.md`.

> **Current status:** damage tracking + retained FBO work and are toggleable in dev (DevTools → Rendering). Scroll-shift reuses retained pixels via intra-FBO blit; retained layers auto-promote from transitions/color anims under an 8-layer / 4 MB budget. Idle frames skip present entirely. **`flash` remains the default renderer; `forge` is opt-in per app via `renderer: "forge"`.**

## The goal

For large stable desktop UIs (5,000–20,000+ nodes: editors, dashboards, timelines) forge should repaint only changed pixels, keep static content cached on the GPU, and stay fluid at 60 Hz with bounded RAM.

## Phase 4 — Content-keyed tile pool

Tiles are keyed to **content**, not a fixed grid:

```cpp
struct TileKey { int parentLayerId; int x, y, w, h; };  // content key
struct Tile { TileKey key; GLuint texture; uint64_t epoch; bool opaque; bool valid; };

class TilePool {
    size_t m_budgetBytes;                    // hard cap, default ~16 MB @1080p
    std::unordered_map<TileKey, Tile> m_tiles;
    std::deque<TileKey> m_lru;               // eviction order
    GLuint acquire(const TileKey&);          // get or allocate (evict if over budget)
    void invalidate(const TileKey&);
};
```

- A large static panel → one big tile; dirty rects invalidate only overlapping tiles
- LRU + epoch eviction keeps VRAM under budget (full-screen animation degrades gracefully to baseline)
- The compositor re-rasters only invalidated tiles each frame

## Phase 5 — Per-node retained layers

Nodes with running animations (playhead scrub, dragging, hover transitions) are **promoted** to their own small retained GPU surface ("like Qt Quick"):

```cpp
struct RetainedLayer { int nodeId; GLuint texture; int w, h; bool active; };
```

- Promotion is automatic from existing flags (`m_isTransitioning`, `m_animations`)
- Layer blits each vsync while the leaf animates; static content beneath stays untouched

## Phase 6 — Scroll-shift

On scroll of an `overflow: auto/scroll` container, **don't re-raster** — shift cached tile content by the scroll delta on the GPU and re-raster only the newly exposed strip.

- Uses existing `scrollY` / `scrollEnabled` state
- A 10k-row list scroll re-rasters only the exposed band

## Phases 7–10 — Hardening, integration, verification

| Phase | Deliverable |
|---|---|
| 7 | Correctness: rounded clips, overflow, borders, opaque fast-path, full-screen-animation degradation guard |
| 8 | Production integration: `renderer: "forge"` in config → `MORPH_RENDERER_FORGE` → single-renderer binary (verified via `nm`/size) |
| 9 | Benchmarks: flash vs forge on 100 / 5,000 / 20,000-node scenes × {scrub, scroll, static, full-screen anim}; report frame time, present bandwidth, RAM, binary size — **done** (`help/forge-benchmarks.md`, `tests/runtime/bench-forge.sh`) |
| 10 | Docs + `examples/flash` / `examples/forge` — **done** |

## Design constraints (already decided)

- **Zero ABI risk** — forge structs live in `runtime/renderers/forge/`, never in `node.h`; `logic.so` never includes them
- **Production = compile-time only** — `constexpr` dispatch, unselected renderer fully eliminated; dev = runtime atomic toggle (~2 ns/frame)
- **Bounded RAM** — hard budget cap (16 MB default); hello-world floor ≈ 30 MB @1080p
- **Scroll-shift scope** — only provably scroll-local, clip-safe tiles; conservative damage expansion at rounded-clip boundaries

## Current state

| Piece | State |
|---|---|
| Flash/Forge seam + dev toggle | ✅ Shipped (dev toggle respects `activeRenderMode()`; mode switch forces fullscreen) |
| `DamageSet` + retained FBO + damage-limited present | ✅ Shipped (per-window damage/FBO; idle skips present; >32-anim fullscreen guard) |
| Tile pool (`TileKey`/`Tile`/`TilePool` + LRU) | ✅ Shipped (`runtime/cpp/renderers/forge/tile.h`, `tile_pool.h` — 16 MB default budget, epoch + LRU eviction; residency-only, zero VRAM until a per-tile composite path consumes it) |
| Retained layers (`RetainedLayer` + `LayerPool`) | ✅ Shipped (`runtime/cpp/renderers/forge/layer.h` — auto-promotion from transitions/color/position anims, 8-layer / 4 MB budget, LRU + prune; v1 mover layers allocate real FBOs with padded raw capture + blended composite, `mover_layer.cpp` eligibility) |
| Scroll-shift (`scroll_shift.cpp`) | ✅ Shipped (`runtime/cpp/renderers/forge/scroll_shift.h/.cpp` — scroll-only deltas reuse retained pixels via intra-FBO blit; exposed strip + scrollbar damage only; transformed/rounded containers fall back to full damage) |
| Production single-renderer builds | ✅ Shipped (`renderer: "forge"` → `MORPH_RENDERER_FORGE` → TU exclusion; `constexpr kRenderMode` dispatch; verified by build-failure with `#error` markers in `forge.cpp`/`mover_layer.cpp` plus `examples/forge` at 288,968 B vs flash 145,608 B) |
| Benchmarks (Phase 9) | ✅ Shipped (`help/forge-benchmarks.md` — scrub 158.8×, scroll 20.1× raster saved, idle present 0 B; `examples/flash` 145,608 B vs `examples/forge` 288,968 B with the real backend) |
| Examples (Phase 10) | ✅ Shipped (`examples/flash` static UI, `examples/forge` scrollable list; `docs/examples/flash.md`, `docs/examples/forge.md`) |
| DevTools overlay | ✅ Shipped (Rendering tab FORGE card + Show-damage rect overlay + peak RSS + Layers row; `tests/runtime/check-forge-math.sh` covers damage/tile/scroll-shift/layer math — 67 checks) |

## Limitations (logic, not design)

None of the items below is a flaw in the retained-pixel + damage model —
each is bounded engineering work on top of it:

| Limitation | Nature | Fix |
|---|---|---|
| Scroll-shift strip is cleared but incoming content is culled in unscrolled coords (damage cull vs screen-space strip) → background-colored band on scroll | ✅ Fixed — commit damage and present culling uniformly screen-space (`screenYOf`/`damageNodeScreen` in `forge.cpp`, screen-box cull in `window.cpp`); dead raw-space `DamageSet::add(MorphNode*)`/`addAll` removed |
| Container whole-box repaint destroys retained pixels of culled rows outside damage (scroll-shift strip, any sub-container damage) | ✅ Fixed — damage-stencil mask in `drawFrameNodes`: paint outside the damage set is impossible by construction (`GLRenderer::stencilDamageMask`, bit 0x80, clip-INCR-safe); culling stays pure optimization; proven by the pixel suite (T2/T3) |
| Scroll-shift guards check only the container's own transform/clip; ancestor transforms, rounded clips, nested scrollers slip through | ✅ Fixed — `shiftSafeAncestors()` rejects transformed / rounded / effectively-scrolled / non-containing-clip ancestors (fallback: full-container damage); nested `childVisY` visibility test corrected to accumulated scroll |
| Tile pool invalidates-then-acquires the same rect every frame, so it never hits; stats-only | ✅ Fixed (invalidation half) — present now invalidates by precise pre-merge boxes, so stable-region tiles persist across coarse merged damage (`residency=kept` in bench); per-tile composite consumption still future (pixel verification itself is now headless, see below) |
| `LayerPool` tracks promotion/budget but allocates no surfaces; animated leaves still re-raster | ✅ Fixed (v1: position-only movers) — single eligible leaf gets a real FBO (`ensureSurface`, +1px padding), is captured raw via `captureNodeLayer`, and composites as a blended textured quad after background restore (`drawFrameNodesExcluding`); span damage covers the travel; color/opacity/radius anims, text, overlap, and multi-movers fall back to re-raster; proven by the pixel suite (T6) |
| `morph.config.json` `"renderer": "forge"` never reached the IR (`builder.rs` hardcoded `"flash"`) so production app builds silently shipped flash — the whole retained backend was dev-only | ✅ Fixed — `IRBuilder::with_app_renderer` (main + route windows) wired from `config.renderer` in `morph build` with a warning on unknown values; `mover_layer.cpp` added to the app TU list; proven by build-failure with `#error` markers in `forge.cpp`/`mover_layer.cpp` and the forge example growing 157,896 → 288,968 B |
| X/Y compositor anims (and >32 anims) fall back to fullscreen | ✅ Fixed — per-tick paint dirt drives frames (fixes prod freeze), commit damages base, present extends damage to the interpolated box (`moverBox`) with transformed-escalation; >32 guard retained |
| Non-idle frames always full-blit; sub-pixel damage truncated; horizontal scroll unsupported | Partially fixed — 2px margin provably covers int/round mismatch to a sub-pixel sliver; X-shift blocked on horizontal-scroll product support (no scrollX producer; dead code otherwise); platform damage extensions unavailable (GLFW doesn't expose them) |

The one near-inherent item is full-surface present: the swapchain backbuffer
is undefined after swap. Idle-skip already captures most of that win.

## Verification plan

- [x] Unit tests: damage union, tile epoch invalidation, LRU eviction, scroll-shift offset math, mover boxes + spans, layer promotion + mover-layer gates (`tests/runtime/check-forge-math.sh` — 67 checks)
- [x] Pixel checks: headless EGL raster verification driving the real `renderNode` culling, `applyScrollShift`, subtree exclusion, and the mover-layer capture/composite path against synthetic frames — full-draw determinism, damage-raster identity, scroll-shift identity, exclusion with control, blit mapping, mover-layer capture + reuse at two positions with decline coverage (multi/color/overlap) (`tests/runtime/check-forge-pixels.sh` — 36 checks; RGB ±2 / alpha ±16 SDF-fringe gate, plus a documented two-zone gate for layer composites where interior + background stay strict and only the mover's 1px ring allows ±32 for capture-phase SDF corner noise; skips green without EGL)
- [x] Perf counters on 100 / 5,000 / 20,000-node scenes; RSS + VRAM measurement at 1080p (`tests/runtime/bench-forge.sh`, `help/forge-benchmarks.md`; live RSS in the FRAME card)