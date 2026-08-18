# FLASH / FORGE — Dual-Renderer Architecture & Implementation Plan

> Status: Approved plan (not yet implemented)
> Scope: Two user-selectable renderers — `flash` (Lightweight Direct Renderer) and `forge` (Hybrid Retained Tile Compositor)
> Related: `arch/hybrid-renderer.md` (tile-architecture deep-dive for the forge side), `arch/compositing-thread.md` (compositor thread model, reused)

---

## 1. Product concept

Two named, user-selectable renderers — the same philosophy as an engine picking a backend:

| Renderer | One-liner | Target |
|---|---|---|
| **flash** | Lightweight Direct Renderer — current full-clear path, minimal size/RAM | small apps, static UIs, low-footprint/embedded-ish constraints |
| **forge** | Hybrid Retained Tile Compositor — content-keyed tiles + damage-region repaint + per-node layers + scroll-shift | large stable desktop apps, editors, dashboards |

Both share the **same model layer** (node tree, layout, style, dirty flags, reactivity). The renderer is only the **paint + present** back half. Selection is **per-app**; the default is safe and backwards compatible.

**Config example:**
```jsonc
// project config (mproj / morph config)
{ "renderer": "forge" }   // "flash" (default) | "forge"
```

---

## 2. Design principles

1. **Single model, dual paint.** Layout/style/dirty logic is renderer-agnostic. Only `commitFrame`/`renderFrame` and the compositor branch by mode.
2. **Zero ABI risk.** All forge structures (`TilePool`, `DamageSet`, `RetainedLayer`) live in runtime-only headers under `renderers/forge/` (see §3), **never in `node.h`**. `logic.so` never includes them, so `MORPH_RENDERER_FORGE` is **not** needed in `compiler.py` — cleaner than the `MORPH_FEATURE_DEV` case.
3. **Production = build-time only; dev = runtime toggle.** In production the renderer is resolved at compile time (`constexpr` + `if constexpr`) so the unselected renderer is **fully eliminated** — zero runtime branch, zero dead code, smallest binary. In dev, **both** renderers are compiled and a runtime toggle (`g_renderMode`) switches between them for A/B experimentation: dev binary size is irrelevant, and the per-frame dispatch is one relaxed atomic read (~2 ns, once per frame). The runtime toggle is **dev-only** (`MORPH_FEATURE_DEV_RENDERER_SWITCH`) and absent from production builds.
4. **Backwards compatible.** `flash` = today's behavior, verbatim. Existing apps run unchanged.
5. **Bounded RAM.** Forge has a hard budget cap on the tile pool (configurable). Hello-world floor ≈ 30–31 MB @1080p.

---

## 3. Code organization

Renderers are isolated under a dedicated folder so **flash** and **forge** never share
implementation files; common seam code lives at the folder root.

```
runtime/renderers/                  # common renderer code (the seam)
├── renderer.h                      # RenderMode, activeRenderMode(), RendererBackend interface
├── renderer.cpp                    # g_renderMode (dev toggle), backend entry helpers
├── flash/
│   ├── flash.h                     # FlashBackend
│   └── flash.cpp                   # flashCommit / flashPresent (current full-clear path, moved from window.cpp)
└── forge/
    ├── forge.h                     # ForgeBackend
    ├── forge.cpp                   # forgeCommit / forgePresent
    ├── damage.h                    # DamageRect, DamageSet
    ├── tile.h                      # TileKey, Tile
    ├── tile_pool.h / tile_pool.cpp # TilePool (LRU + budget + epoch)
    ├── layer.h                     # RetainedLayer
    └── scroll_shift.cpp            # scroll-shift tile remap + exposed-strip invalidation
```

Layering with existing folders:

| Folder | Role | Shared? |
|---|---|---|
| `runtime/render/` | low-level GPU primitives (`GLRenderer`: rects/text/borders, batching + flush, clips) | shared by both renderers |
| `runtime/core/render_frame.h` | the shared flat-frame format (`RenderFrame`, `FlatRenderNode`) | shared; forge extends it |
| `runtime/renderers/` | high-level UI render backends (flash, forge) + seam | per-renderer isolated |
| `runtime/core/window.cpp` | dispatch host: calls `activeRenderMode()` then the backend's commit/present | shared host |

- `flash` owns today's full-clear path **verbatim** (moved out of `window.cpp`/`renderFrame`).
- `forge` owns all tile/damage/layer/scroll-shift logic; nothing leaks into `node.h`.
- New sources are added to the dev CMake; production CMake compiles **only the chosen renderer's** folder.

---

## 4. Renderer seam (abstraction)

```cpp
// runtime/renderers/renderer.h (runtime-only, common)
enum class RenderMode : uint8_t { Flash = 0, Forge = 1 };

// ── Renderer mode resolution ─────────────────────────────
// Production: build-time only. kRenderMode is constexpr, so the `if` folds at
// compile time and the unselected renderer is eliminated — zero runtime branch,
// zero dead code, smallest binary.
//
// Dev: both renderers are compiled and a runtime toggle (g_renderMode) switches
// between them for experimentation. Dev binary size is irrelevant and the
// per-frame dispatch is one relaxed atomic read. The switch is dev-only.
#ifdef MORPH_FEATURE_DEV_RENDERER_SWITCH
extern std::atomic<RenderMode> g_renderMode;                     // dev: runtime toggle
inline RenderMode activeRenderMode() {
    return g_renderMode.load(std::memory_order_relaxed);
}
#else
#ifdef MORPH_RENDERER_FORGE
constexpr RenderMode kRenderMode = RenderMode::Forge;
#else
constexpr RenderMode kRenderMode = RenderMode::Flash;
#endif
constexpr RenderMode activeRenderMode() { return kRenderMode; }  // production: compile-time
#endif

// runtime/core/window.cpp — dispatch (calls into renderers/)
void MorphWindow::commitFrame() {
    layoutIfNeeded(...);                       // shared
    if (activeRenderMode() == RenderMode::Forge)
        forge::accumulateDamage();             // renderers/forge/damage.h (Phase 2); flash: no-op
    recordPaintTree(...);                      // shared (per-node display-list skip)
    if (activeRenderMode() == RenderMode::Forge)
        forge::forgeCommit();                  // renderers/forge/forge.cpp
    else
        flash::flashCommit();                  // renderers/flash/flash.cpp (unchanged)
}

void MorphWindow::renderFrame(...) {
    if (activeRenderMode() == RenderMode::Forge)
        forge::forgePresent();                 // tile re-raster, layer blits, damage-limited present
    else
        flash::flashPresent();                 // current full clear + redraw (unchanged)
}
```

The compositor thread (`compositor.cpp`) branches the same way: forge owns the tile pool + tile blits + scroll-shift; flash keeps the current pass. GL stays exclusively on the compositor thread.

---

## 5. Feature flags & configuration plumbing

| Layer | Change |
|---|---|
| Config load | new `renderer` key → `RenderMode` (default flash; dev can override at runtime) |
| `codegen/feature_set.py` | `renderer:"forge"` → emit `MORPH_RENDERER_FORGE` for production |
| Production runtime CMake | defines the chosen renderer only → compile-time dispatch, one renderer in the binary |
| Dev CMake (`runtime/dev/CMakeLists.txt`) | defines `MORPH_FEATURE_DEV_RENDERER_SWITCH` (+ default `MORPH_RENDERER_FORGE` as the starting mode) → both renderers compiled, runtime toggle |
| `build/compiler.py` | **unchanged** — no `node.h` impact → no ABI concern |
| DevTools (`runtime/dev/inspector.h`, `main.cpp`) | renderer switch + active-renderer badge + damage overlay (dev only) |

Production ships exactly one renderer (build-time, zero dead code); dev ships both and can hot-switch for A/B testing.

ABI rule (same discipline as `MORPH_FEATURE_DEV`): any change that alters `MorphNode` layout must be guarded identically in the dev binary **and** `logic.so`. Forge's structures are intentionally excluded from `node.h`, so the flag stays out of `compiler.py` entirely.

---

## 6. Forge data structures (runtime-only, under `renderers/forge/`)

All forge structures are runtime-only — they live in `renderers/forge/*.h`, **never** in `node.h`, so `logic.so` stays untouched and no ABI risk is created. See §3 for the folder mapping.

```cpp
// renderers/forge/damage.h
struct DamageRect { int x, y, w, h; };           // screen space, integer
struct DamageSet {
    std::vector<DamageRect> rects;               // unioned, clipped to viewport
    bool fullScreen = false;                     // forced when untrackable
    bool intersects(const DamageRect&) const;
    void add(const DamageRect&);
    void add(MorphNode*);                        // conservative (style bounds + clip expansion)
};

// renderers/forge/tile.h
struct TileKey { int parentLayerId; int x, y, w, h; };  // keyed to node rects, not a grid
struct Tile {
    TileKey key;
    GLuint texture;                              // RGBA8
    uint64_t epoch;                              // version; invalidation from per-node dirty flags
    bool opaque = false;                         // opaque fast-path hint
    bool valid = false;
};

// renderers/forge/tile_pool.h
class TilePool {
    uint64_t m_epoch = 0;
    size_t m_budgetBytes;                        // hard cap (configurable)
    std::unordered_map<TileKey, Tile> m_tiles;
    std::deque<TileKey> m_lru;
    GLuint acquire(const TileKey&);              // get or allocate (evict if over budget)
    void invalidate(const TileKey&);             // bump epoch / drop
};

// renderers/forge/layer.h
struct RetainedLayer {
    int nodeId;                                  // flat frame index
    GLuint texture;
    int w, h;
    bool active;                                 // promoted while transitioning/animating
};

// renderers/forge/forge.h — frame payload (extended RenderFrame)
struct RenderFrame {
    std::vector<FlatRenderNode> nodes;           // existing
    std::vector<DrawOp> drawOps;                 // existing
    std::vector<FlatTextOp> textOps;             // existing
    std::vector<AnimationState> animations;      // existing
    DamageSet damage;                            // NEW
    std::vector<TileKey> invalidTiles;           // NEW
    std::vector<RetainedLayer> layers;           // NEW
    bool scrollShift;                            // NEW
    uint64_t frameId;
    double timestamp;
};
```

---

## 7. Thread model

Reuses the existing compositor thread — no new threads.

| Thread | flash | forge |
|---|---|---|
| Main | layout → record → flatten → swap (today) | + damage accumulation, tile invalidation (epoch), layer promotion, scroll-shift exposure strip |
| Compositor | full draw → swap (today) | tile pool LRU, tile re-raster (scissored), layer blits, damaged-tile composition, scroll-shift offset remap, damage-limited present |

Sync stays lock-free (frame pointer swap + SPSC queues). Tiles/layers are owned by the compositor thread only (GL textures never touched by the main thread).

---

## 8. RAM sizing (forge)

Floor: one window's worth of retained pixels is mandatory (old pixels must persist to repaint partially).

| Window | Minimum added (tight/lazy budget) | Generous pool | Hello-world total (≈22 MB baseline) |
|---|---|---|---|
| 1280×720 | ~4–5 MB | +5–12 MB | ~27–34 MB |
| 1920×1080 | ~8.3 MB | +8–25 MB | **~30–47 MB** |

- Lazy pool: a static hello-world caches ≈ one full-window opaque tile → **+8.3 MB @1080p**.
- Hard budget cap (e.g., 16 MB) bounds worst case; LRU eviction re-rasters more under thrash.

---

## 9. Implementation phases (each with Definition of Done)

### Phase 0 — Naming & flag plumbing
- Add `RenderMode`, `activeRenderMode()`, dev `g_renderMode` toggle; config key; CMake defines (`MORPH_FEATURE_DEV_RENDERER_SWITCH` dev-only, `MORPH_RENDERER_FORGE`); `feature_set.py` mapping; DevTools badge + switch.
- **DoD:** building with either config runs; flash pixel-identical to today; dev toggle switches renderers at runtime; production binary contains only the chosen renderer (verify via `nm`/size).

### Phase 1 — Renderer seam refactor
- Extract `flashCommit`/`forgeCommit` (forge stub = flash), dispatch in `commitFrame`/`renderFrame`; compositor branch point.
- **DoD:** zero behavior change; smoke tests pass on flash.

### Phase 2 — Damage tracking (shared)
- `DamageSet` from geometry-diff + paint-dirty nodes; viewport clip; conservative expansion at rounded-clip boundaries; DevTools damage overlay + stats.
- **DoD:** correct damage on text change / hover / scroll clamp / drag / animation; unit tests for union/clip/expansion.

### Phase 3 — Forge core: retained FBO + damage-limited present
- Persistent surface, scissored clear of damage only, `glBlitFramebuffer` of damage rects; idle → no present.
- **DoD:** scrub present BW drops ~18× vs flash; pixels identical.

### Phase 4 — Content-keyed tile pool
- `TileKey`/`Tile`/`TilePool` with epoch + LRU + budget; compositor re-rasters only invalidated tiles.
- **DoD:** unchanged tiles skipped (CPU + GPU); LRU respects budget; stress test leak-free.

### Phase 5 — Per-node retained layers (animated leaves)
- Auto-promote nodes with running animations/transitions to small surfaces; layer blits each vsync.
- **DoD:** playhead/drag/hover transitions run on layers with static content untouched.

### Phase 6 — Scroll-shift
- For `overflow: auto/scroll` containers: remap tile blit offsets on scroll, re-raster only exposed strip.
- **DoD:** scrolling a 10k-row list re-rasters only the newly exposed band; pixel-identical to flash.

### Phase 7 — Correctness hardening
- Rounded clips, overflow, borders, overlap at tile boundaries; opaque fast-path; full-screen-animation degradation guard.
- **DoD:** forge === flash pixels on the stress app (scroll + scrub + hover + anims + borders).

### Phase 8 — Production integration
- Config → `feature_set.py` → single-renderer production build; ABI verification; size deltas measured.
- **DoD:** production binary contains exactly one renderer; runs identically to dev.

### Phase 9 — Benchmarks & verification matrix
- flash vs forge on 100 / 5,000 / 20,000-node scenes × {scrub, scroll, static, full-screen anim}; report frame time, present BW, RAM (RSS + VRAM), size.
- **DoD:** documented in `arch/`; targets below.

### Phase 10 — Docs & examples
- Update `arch/` docs; add `examples/flash` + `examples/forge`; renderer selection guide.

---

## 10. Verification matrix (targets)

| Metric | flash | forge | Target |
|---|---|---|---|
| Hello-world RAM @1080p | ~22 MB | ~30–31 MB | measured |
| Scrub present BW (5k nodes) | ~498 MB/s | <1 MB/s | measured |
| Raster skipped on scrub | 0% | ~6–16× less | measured |
| Binary size | smallest | +50–150 KB | measured |
| Pixel-identical to flash | — | ✔ | stress app |

---

## 11. Risks & mitigations

| Risk | Mitigation |
|---|---|
| Clip/rounded correctness in tiles | Phase 7 + conservative damage expansion (never wrong pixels, occasionally repaint more) |
| Tile-pool thrash under full-screen change | Budget cap + LRU; degrade gracefully to ≈ flash |
| VRAM growth | Hard cap, lazy allocation, eviction; measured per platform |
| Scroll-shift with overlapping/rounded content | Apply only to provably scroll-local, clip-safe tiles |
| Scope creep | flash path frozen; forge lands behind flag; each phase independently shippable |
| ABI drift if retained structures touch `MorphNode` | Keep forge structures out of `node.h`; same guard discipline as `MORPH_FEATURE_DEV` |

---

## 12. Rollout

1. Land Phases 0–3 behind the flag (forge opt-in; default flash).
2. Ship Phases 4–7 as forge matures; flash stays untouched.
3. Phases 8–10: production opt-in per app, benchmarks, docs.

---

## 13. Open decisions (defaults proposed)

| Decision | Default proposal |
|---|---|
| Production default renderer | `flash` (until forge is battle-tested); opt-in per app |
| Tile pool budget @1080p | 16 MB cap (≈ 2× window), configurable |
| Layer promotion threshold | any running animation / layout transition on the leaf |
| Scroll-shift scope | `overflow: auto/scroll` containers only |
| Renderer selection | dev: runtime toggle (`g_renderMode`, dev-only); production: build-time only (`constexpr` + `if constexpr`, one renderer) |

---

*Companion docs:* `arch/hybrid-renderer.md` (tile-architecture deep-dive, now the forge design reference) · `arch/compositing-thread.md` (compositor thread model, reused).
