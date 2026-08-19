# Forge Renderer — Tile Pool, Retained Layers, Scroll-Shift

**Status:** beta → future · **Priority:** medium

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

The `forge` renderer (retained FBO + damage tracking) is shipped but beta. This page documents the remaining phases that turn it into a production renderer: **content-keyed tile pool**, **per-node retained layers**, and **scroll-shift tile reuse**. Full design: `help/renderer-flash-forge.md` and `help/hybrid-renderer.md`.

> **Current status:** damage tracking + retained FBO work and are toggleable in dev (DevTools → Rendering). Known bugs: damage-rect edges, scroll-shift, some compositor-animation paths. **`flash` is the recommended production renderer until forge matures.**

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
| 9 | Benchmarks: flash vs forge on 100 / 5,000 / 20,000-node scenes × {scrub, scroll, static, full-screen anim}; report frame time, present bandwidth, RAM, binary size |
| 10 | Docs + `examples/flash` / `examples/forge` |

## Design constraints (already decided)

- **Zero ABI risk** — forge structs live in `runtime/renderers/forge/`, never in `node.h`; `logic.so` never includes them
- **Production = compile-time only** — `constexpr` dispatch, unselected renderer fully eliminated; dev = runtime atomic toggle (~2 ns/frame)
- **Bounded RAM** — hard budget cap (16 MB default); hello-world floor ≈ 30 MB @1080p
- **Scroll-shift scope** — only provably scroll-local, clip-safe tiles; conservative damage expansion at rounded-clip boundaries

## Current state

| Piece | State |
|---|---|
| Flash/Forge seam + dev toggle | ✅ Shipped |
| `DamageSet` + retained FBO + damage-limited present | ✅ Shipped (beta, known bugs) |
| Tile pool (`TileKey`/`Tile`/`TilePool` + LRU) | ❌ Phase 4 |
| Retained layers (`RetainedLayer`) | ❌ Phase 5 |
| Scroll-shift (`scroll_shift.cpp`) | ❌ Phase 6 |
| Production single-renderer builds | ❌ Phase 8 |

## Verification plan

- Unit tests: damage union, tile epoch invalidation, LRU eviction, scroll-shift offset math
- Visual checks: DevTools damage/tile-invalidation overlay (extends the existing repaint-highlight hook)
- Perf counters on 100 / 5,000 / 20,000-node scenes; RSS + VRAM measurement at 1080p