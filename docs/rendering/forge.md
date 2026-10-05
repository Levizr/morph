# Forge

`forge` is Morph's hybrid retained compositor — a renderer that keeps pixels on the GPU between frames and repaints only what changed. It is **production opt-in**: per-window retained FBO + damage tracking, scroll-shift tile reuse, and retained layers for animated leaves are implemented; select it per app with `"renderer": "forge"`.

## How It Works

Instead of clearing every frame, forge keeps a persistent FBO surface of the window and computes a **damage set** — the rectangles that actually changed:

1. **Damage accumulation** — a `DamageSet` is built each frame from:
   - a live prev-frame geometry map (old + new positions of moved nodes)
   - pre-layout paint dirt from the shared dirty-flag system
   - running non-geometry compositor animations
   - scroll and content-height changes
2. **Fullscreen fallback** — damage is forced to fullscreen on the first frame, while X/Y compositor animations run, or when the node count changes
3. **Damage-limited raster** — scissored color clears + depth/stencil reset per damage rect; only nodes touching damage are re-rastered into the retained surface
4. **Present** — the whole surface is blitted with `glBlitFramebuffer`; idle frames skip present entirely (0 bytes, no swap)

Conservative 1px expansion past rounded-clip boundaries guarantees no stale edges; stale prev-rects are pruned as nodes disappear.

## Status

| Piece | State |
|---|---|
| Flash/Forge seam + dev toggle | Shipped (mode switch forces fullscreen) |
| `DamageSet` + retained FBO + damage-limited present | Shipped (per-window; idle skips present) |
| Content-keyed tile pool | Shipped (residency-only LRU + 16 MB budget) |
| Per-node retained layers | Shipped (auto-promotion, 8-layer / 4 MB budget) |
| Scroll-shift tile remap | Shipped (intra-FBO blit + exposed-strip damage) |
| Benchmarks (Phase 9) | Shipped (`help/forge-benchmarks.md` — scrub 158.8×, scroll 20.1×, idle present 0 B; +12 KB binary) |
| Examples (Phase 10) | Shipped (`examples/flash`, `examples/forge`) |
| DevTools overlay | Shipped (FORGE card: damage/present/tiles; Show-damage rect overlay; peak RSS) |

X/Y compositor animations move through the normal damage path now (old box damaged at commit, new box added present-side once offsets are known); only transformed movers escalate to fullscreen. Scrolled, transformed, or rounded-clip containers that fail the scroll-shift safety check fall back to full-container damage. Test it from DevTools → Rendering → RENDERER (`Flash | Forge` toggle).

Known limitations are tracked as fixable engineering work — logic gaps, not
design flaws — in the [Forge Renderer roadmap](../future/rendering/forge-tile-pool.md#limitations-logic-not-design).

## Why Retained Rendering

The goal is large stable desktop UIs — 5,000–20,000+ node editors, dashboards, timelines — where full-clear redraws waste most of their bandwidth repainting identical pixels. Forge targets repainting only changed regions, with design targets like scrub present bandwidth under 1 MB/s where flash measures ~498 MB/s at 5k nodes.

## RAM Cost

Retention isn't free: one window's worth of pixels must persist to repaint partially (~8.3 MB @1080p). Hello-world floor is ≈30 MB @1080p vs ~22 MB for [flash](flash.md). A hard budget cap bounds worst case.

## What's Next

Nothing structural remains: benchmarks, examples, and the DevTools overlay
are landed (see [Forge Renderer roadmap](../future/rendering/forge-tile-pool.md)).
`flash` stays the default; pick `forge` per app when the UI is large and
mostly stable.
