# Forge Example

A scrollable list on the `forge` renderer — Morph's hybrid retained
compositor. Scrolling the list exercises scroll-shift tile reuse (only the
exposed strip re-rasters); hovering rows exercises retained-layer promotion
for animated leaves.

## What it shows

- **`renderer: "forge"`** in `morph.config.json` — compile-time backend pick
- **Scrollable list** (`overflow: auto`) — GPU scroll-shift instead of
  full re-raster
- **Hover transitions** — animated leaves promote to retained layers while
  static content stays untouched
- **When to use forge** — large stable UIs: editors, dashboards, timelines
  (see [Choosing a Renderer](../../docs/rendering/index.md))

## Run

```bash
cd examples/forge
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```

Open DevTools → Rendering to watch damage area, present bytes, tile
residency, and the damage overlay live.
