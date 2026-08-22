# Forge

`forge` is Morph's hybrid retained compositor — a renderer that keeps pixels on the GPU between frames and repaints only what changed. It is **beta**: shipped and toggleable in dev, but flash remains the recommended production renderer until forge matures.

## How It Works

Instead of clearing every frame, forge keeps a persistent FBO surface of the window and computes a **damage set** — the rectangles that actually changed:

1. **Damage accumulation** — a `DamageSet` is built each frame from:
   - a live prev-frame geometry map (old + new positions of moved nodes)
   - pre-layout paint dirt from the shared dirty-flag system
   - running non-geometry compositor animations
   - scroll and content-height changes
2. **Fullscreen fallback** — damage is forced to fullscreen on the first frame, while X/Y compositor animations run, or when the node count changes
3. **Damage-limited raster** — scissored color clears + depth/stencil reset per damage rect; only nodes touching damage are re-rastered into the retained surface
4. **Present** — the whole surface is blitted with `glBlitFramebuffer`; idle frames only blit

Conservative 1px expansion past rounded-clip boundaries guarantees no stale edges; stale prev-rects are pruned as nodes disappear.

## Status

| Piece | State |
|---|---|
| Flash/Forge seam + dev toggle | Shipped |
| `DamageSet` + retained FBO + damage-limited present | Shipped (beta) |
| Content-keyed tile pool | Planned |
| Per-node retained layers | Planned |
| Scroll-shift tile remap | Planned |

Known bugs: damage-rect edges, scroll-shift paths, some compositor-animation paths. Test it from DevTools → Rendering → RENDERER (`Flash | Forge` toggle).

## Why Retained Rendering

The goal is large stable desktop UIs — 5,000–20,000+ node editors, dashboards, timelines — where full-clear redraws waste most of their bandwidth repainting identical pixels. Forge targets repainting only changed regions, with design targets like scrub present bandwidth under 1 MB/s where flash measures ~498 MB/s at 5k nodes.

## RAM Cost

Retention isn't free: one window's worth of pixels must persist to repaint partially (~8.3 MB @1080p). Hello-world floor is ≈30 MB @1080p vs ~22 MB for [flash](flash.md). A hard budget cap bounds worst case.

## What's Next

The remaining phases turn forge into the production renderer: a content-keyed tile pool with LRU + budget, per-node retained layers for animated leaves, and scroll-shift tile reuse. See [Forge Renderer roadmap](../future/forge-renderer.md).
