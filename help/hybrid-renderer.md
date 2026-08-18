# Hybrid Retained Renderer — Tier 3 Architecture (forge deep-dive)

> Status: Design reference for the **forge** renderer (see `arch/renderer-flash-forge.md` for the dual-renderer implementation plan)
> Scope: Browser-grade damage rendering — paint only damaged regions, skip unchanged work entirely
> Selection: User-configurable renderer (per-app) — **dev: runtime toggle; production: build-time only** (`constexpr` + `if constexpr`)

---

## 1. Goals

1. **Damage-region rendering:** repaint only the pixels that actually changed, not the whole window every dirty frame.
2. **Skip unchanged work entirely:** unchanged nodes are neither re-rasterized nor re-presented (retained tiles/layers).
3. **Large stable desktop UIs** (5,000+ nodes, video-editor-style surfaces) stay fluid while scrubbing/scrolling with localized mutations at 60 Hz.
4. **User-selectable renderer:** each app picks its renderer; dev build ships both for experimentation, production compiles only the chosen one (no dead code).
5. **Bounded RAM:** hard budget cap on the tile pool; ~30 MB floor for a hello-world app at 1080p.

---

## 2. Problem with the current approach

| # | Problem | Where |
|---|---|---|
| 1 | Full `glClear` + full scene re-raster **every dirty frame**; cost scales with total scene size, not change size | `window.cpp:480-505` (`renderFrame`), `gl_renderer.cpp:664-672` (`clear`) |
| 2 | Whole-tree `flatten()` every dirty frame — every node's `FlatRenderNode` is rebuilt even when nothing changed | `flatten.cpp:43-127` |
| 3 | Damage information is computed and thrown away: `recordPaintTree` limits *display-list re-recording* but nothing maps "what changed" → *which pixels* | `window.cpp:596-609` |
| 4 | Draw-by-type ordering (all rects → text → images → borders) prevents cheap per-node partial redraw without a retained surface | `gl_renderer.cpp:747-875` |
| 5 | A 1-px playhead tick re-presents the full 8.3 MB window (at 1920×1080) | present path |

**Net effect on a video-editor-style workload:** the GPU runs at continuous full-screen fill and the CPU does a full-tree walk every frame, while ≥99% of pixels are static.

---

## 3. Tier comparison (1920×1080 @ 60 fps, 5,000-node video-editor UI, scrub workload)

Assumptions: 1920×1080 = 2,073,600 px, RGBA8 = 4 B/px → 8.3 MB/frame. Scrub/playhead strip ≈ 60 px × full width ≈ 0.46 MB. Figures are order-of-magnitude estimates, not benchmarks.

| | Present BW | Raster work | RAM | Binary size | Video-editor fit |
|---|---|---|---|---|---|
| **Baseline (today)** | ~498 MB/s | full 100% | ~0 extra | tiny | ✗ power/jank on scrub |
| **Tier 1 — damage tracking only** | ~498 MB/s | full 100% | ~KB | +1–3 KB | ✗ metering only |
| **Tier 2 — retained FBO + damage-limited present** | ~27–30 MB/s | ~100% (full draw into FBO) | +9 MB | +10–40 KB | ~ partial (pixel present only) |
| **Tier 3 / Hybrid — retained tiles + layers + scroll-shift** | <1 MB/s | ~6–16× less (scrub), more with node count | +30–150 MB (budget-capped) | +50–150 KB | ✓ best fit, most RAM |

**Scaling laws (important):**
- Present savings ∝ `damaged_area / full_area` (independent of node count).
- Raster savings ∝ number of *static* nodes that get skipped (grows with scene size: ~1.5–3× at 100 nodes, ~6–16× at 5,000, ~20–50× at 20,000+).
- RAM is resolution + budget-cap driven, **not** node-count driven.
- If the whole screen animates, all tiers ≈ baseline (no partial win — expected; the video surface itself is a texture, out of scope for this system).

---

## 4. Chosen architecture: Hybrid retained compositor

Four techniques combined, each mapped onto the framework's existing per-node dirty model:

### 4.1 Content-keyed tiles (not a fixed pixel grid)

Instead of a fixed 256 px screen grid, tiles are keyed to **node rects**:

- A large static panel → one big tile.
- A clip bar / list row → small tiles aligned to its rect.
- Damage collapse is exact: a dirty rect invalidates only the tiles it overlaps.
- Stable tiles stay cached across frames; only invalidated tiles re-rasterize.

### 4.2 Per-node retained layers for animated leaves

- Nodes with active animations (playhead, dragging clip, hover transitions) are promoted to their own small retained GPU surface (`layer.enabled`-style, like Qt Quick).
- Each vsync, only the layer re-blits; static content behind it stays cached.
- Promotion is automatic from `m_isTransitioning` / `m_animations` (existing flags).

### 4.3 Scroll-shift reuse

- On scroll of a timeline/media list, **do not re-raster**: shift the cached tile content by the scroll delta on the GPU (remap tile blit coords) and re-rasterize **only the newly exposed strip**.
- This is the largest win for editors and reuses existing `scrollY`/`scrollEnabled` state.

### 4.4 Damage tracking & dirty-epoch

- **Damage accumulation:** screen-space integer rects accumulated during the dirty frame from:
  - the geometry-diff (already built, `syncPaintDirtyAfterLayout`),
  - explicit `PaintDirty` nodes (text change, style, hover, scroll clamp),
  - scroll-shift exposed strips.
- Rect union + clip to viewport; if a node crosses a rounded clip boundary → conservative rect expansion (correctness first).
- **Tile epoch/version:** each tile stores a version; invalidation is derived directly from the per-node dirty flags. No separate generic invalidation subsystem — this is the "unique" tie-in with the existing dirty model.

### 4.5 Present model

- Present = `glBlitFramebuffer` of the union of damaged tiles only.
- When nothing is dirty → no commit, no present, GPU idle (existing idle early-out preserved).

### 4.6 Pipeline overview

```
MAIN THREAD (CPU)                      COMPOSITOR THREAD (GL)
events → style → layout                 wait for frame
  └─ geometry-diff → damage rects        ├─ interpolate animations
  └─ recordDisplayList (dirty nodes)     ├─ invalidate tiles from epoch
  └─ flatten (unchanged nodes cheap)     ├─ re-raster invalid tiles
  └─ commit: frame + damage list         ├─ compose: blit dirty tiles only
                                         └─ glfwSwapBuffers
```

---

## 5. Renderer selection (user-configurable)

### Mechanism

New feature flag: `MORPH_RENDERER_FORGE` (Tier-3/forge) vs. `flash` (full-redraw path, default, no flag).

| Build | Defines | Behavior |
|---|---|---|
| Dev binary (`runtime/dev/CMakeLists.txt`) | `MORPH_FEATURE_DEV_RENDERER_SWITCH` + default `MORPH_RENDERER_FORGE` | Both renderers compiled; runtime toggle (`g_renderMode`) in DevTools for experimentation |
| Dev `logic.so` (`compiler.py::_DEV_FEATURES`) | **unchanged** | Forge structures never touch `node.h`, so no ABI impact — no define needed |
| Production (`codegen/feature_set.py` + production runtime CMake) | `MORPH_RENDERER_FORGE` (app-chosen only) | Single renderer compiled, zero dead code |
| Production `logic.so` | **unchanged** | ABI parity (shared model unchanged) |

The renderer is a `window.cpp`/`gl_renderer.cpp`/`renderers/` concern; `FlatRenderNode`/`RenderFrame` and all forge structs live only in the runtime, **not** in `logic.so`, and `MorphNode` layout doesn't change between renderers → zero ABI risk.

---

## 6. Data structures (new)

```cpp
// renderers/forge/damage.h
// ── Damage ─────────────────────────────────────────────
struct DamageRect { int x, y, w, h; };           // screen space, integer
struct DamageSet {
    std::vector<DamageRect> rects;               // unioned, clipped to viewport
    bool fullScreen = false;                     // forced when untrackable
    bool intersects(const DamageRect&) const;
    void add(const DamageRect&);
    void add(MorphNode*);                        // conservative (style bounds)
};

// renderers/forge/tile.h
// ── Tiles ─────────────────────────────────────────────
struct TileKey { int parentLayerId; int x, y, w, h; };   // content key, not grid index
struct Tile {
    TileKey key;
    GLuint texture;                              // RGBA8
    uint64_t epoch;                              // last-painted version
    bool opaque = false;                         // opaque fast-path hint
    bool valid = false;
};

// renderers/forge/tile_pool.h
// ── Tile pool (budget-capped, LRU) ────────────────────
class TilePool {
    uint64_t m_epoch = 0;
    size_t m_budgetBytes;                        // hard cap (config)
    std::unordered_map<TileKey, Tile> m_tiles;
    std::deque<TileKey> m_lru;                   // eviction order
    GLuint acquire(const TileKey&);              // get or allocate (evict if over budget)
    void invalidate(const TileKey&);             // bump epoch / drop
};

// renderers/forge/layer.h
// ── Retained layer (animated leaf) ────────────────────
struct RetainedLayer {
    int nodeId;                                  // flat frame index
    GLuint texture;                              // small RGBA8 surface
    int w, h;
    bool active;                                 // promoted while transitioning/animating
};

// runtime/core/render_frame.h (frame payload, extended by forge)
// ── Frame payload (extended RenderFrame) ──────────────
struct RenderFrame {
    std::vector<FlatRenderNode> nodes;           // existing
    std::vector<DrawOp> drawOps;                 // existing
    std::vector<FlatTextOp> textOps;             // existing
    std::vector<AnimationState> animations;      // existing
    DamageSet damage;                            // NEW: damage for this frame
    std::vector<TileKey> invalidTiles;           // NEW: tiles needing re-raster
    std::vector<RetainedLayer> layers;           // NEW: active animated leaves
    bool scrollShift;                            // NEW: apply scroll-shift path
    uint64_t frameId;
    double timestamp;
};
```

---

## 7. Thread model

Reuses the existing compositor thread (`compositor.cpp`) — no new threads:

| Thread | Adds |
|---|---|
| Main | damage accumulation, tile invalidation decisions (epoch), layer promotion, scroll-shift exposure strip computation |
| Compositor | tile pool LRU, tile re-raster (scissored to tile), layer blits, damaged-tile composition, scroll-shift offset remap, damage-limited present |

Synchronization stays lock-free (frame pointer swap + SPSC queues); tiles are owned by the compositor thread only (GL textures must not be touched by the main thread).

---

## 8. RAM sizing

Floor: **one window's worth of retained pixels is mandatory** (old pixels must persist somewhere to repaint partially).

| Window | Minimum added (tight/lazy budget) | Generous pool (scroll headroom + layers) | Hello-world total (≈22 MB baseline) |
|---|---|---|---|
| 1280×720 | ~4–5 MB | +5–12 MB | ~27–34 MB |
| 1920×1080 | ~8.3 MB | +8–25 MB | **~30–47 MB** |

- Tile pool is **lazy**: static hello-world ends up with ~1 full-window opaque tile → ~+8.3 MB @1080p.
- Hard budget cap (e.g., 16 MB) bounds worst case; eviction via LRU re-rasters more under thrash.
- RAM beyond the pool is negligible (few KB of rects/LRU, display lists already exist today).

---

## 9. Implementation phases

| Phase | Deliverable | Files | Depends on |
|---|---|---|---|
| **0** | Feature flag plumbing: `MORPH_RENDERER_FORGE` + `MORPH_FEATURE_DEV_RENDERER_SWITCH` in dev CMake; renderer enum + selection in dispatch host | `runtime/renderers/renderer.[h,cpp]`, `runtime/dev/CMakeLists.txt` (no `compiler.py` change), `codegen/feature_set.py`, `runtime/core/window.cpp` | — |
| **1** | Damage tracking: `DamageSet` accumulation from geometry-diff + paint-dirty nodes; expose stats (DevTools overlay) | `runtime/renderers/forge/damage.h`, `runtime/core/node/node.cpp`, `runtime/core/window.cpp`, `runtime/dev/inspector.h` | 0 |
| **2** | Retained FBO + damage-limited present (Tier 2 baseline): persistent surface, scissored clear, `glBlitFramebuffer` of damage rects | `runtime/renderers/forge/forge.cpp`, `runtime/render/gl_renderer.cpp` | 1 |
| **3** | Content-keyed tile pool: `TileKey`/`Tile`/`TilePool` with LRU + epoch; compositor-side raster of invalid tiles | `runtime/renderers/forge/tile_pool.[h,cpp]`, `runtime/core/render_frame.h` | 2 |
| **4** | Per-node retained layers for animated leaves; layer blit path; budget accounting | `runtime/renderers/forge/layer.h` + `runtime/core/compositor.cpp` | 3 |
| **5** | Scroll-shift: offset remap of cached tiles + exposed-strip invalidation; integrate `scrollY` changes | `runtime/renderers/forge/scroll_shift.cpp`, `runtime/core/window.cpp`, `runtime/core/compositor.cpp` | 3 |
| **6** | Rounded-clip / overflow correctness: conservative damage expansion at clip boundaries; opaque fast-path | `runtime/renderers/forge/forge.cpp` + `runtime/core/window.cpp` | 4 |
| **7** | Production integration: config → `feature_set.py` → define; single-renderer production build; ABI verification | `codegen/feature_set.py`, production CMake | 0 |
| **8** | Verification & tuning: budget knob, LRU behavior, benchmarks (below) | — | all |

---

## 10. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Rounded clips / overflow / shadows baked into tile content invalidate incorrectly | Conservative damage expansion; re-raster full tile on clip-boundary crossing; opaque fast-path only when provable |
| Tile pool thrashing under full-screen animation | Budget cap + LRU; expect degradation to ≈ baseline in full-screen-change cases (acceptable) |
| Texture memory growth (VRAM) | Hard cap config, lazy allocation, eviction; document per-platform budgets |
| Scroll-shift breaks with rounded/overlap content | Apply scroll-shift only to tiles whose content is scroll-container-local and clip-safe |
| ABI mismatch if retained structures touch `MorphNode` | Keep all retained structures out of `node.h` or guard identically in both binary and `logic.so` (same rule as `MORPH_FEATURE_DEV`) |
| Dev-only measurements mislead production expectations | Tier 1 stats first; production numbers validated on the same workload in phase 8 |

---

## 11. Verification

1. **Unit:** damage union correctness; tile epoch invalidation; LRU eviction under budget; scroll-shift offset math.
2. **Visual:** DevTools Rendering overlay shows per-node repaint (existing `g_repaintHook`); new overlay shows damage rects + tile invalidations; verify identical pixels vs basic renderer on a stress app (scroll + scrub + hover + animations).
3. **Perf:** frame-time & present-bandwidth counters; compare basic vs retained on: 100 / 5,000 / 20,000-node scenes, scrub workload, scroll workload.
4. **RAM:** RSS + VRAM measurement at 1080p; verify hello-world ≈ 30–47 MB range; verify cap enforcement.

---

## 12. Open decisions (defaults proposed)

| Decision | Default proposal |
|---|---|
| Renderer selection mechanism | dev: runtime toggle (`g_renderMode`, dev-only); production: build-time only (`constexpr` + `if constexpr`, one renderer) |
| Tile pool budget @1080p | 16 MB cap (≈ 2× window), configurable |
| Layer promotion threshold | any running animation / layout transition on the leaf |
| Scroll-shift scope | enabled for `overflow: auto/scroll` containers only |
| Production default renderer | basic (until retained is battle-tested); opt-in per app |

---

*Related:* `arch/renderer-flash-forge.md` (dual-renderer plan — this design is the forge side) · `arch/compositing-thread.md` (existing compositor thread architecture — reused, extended).
