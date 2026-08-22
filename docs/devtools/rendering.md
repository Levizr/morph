# Rendering

The Rendering tab is a live profiler for Morph's render pipeline — per-frame counters, layout and paint diagnostics, and the Flash/Forge renderer switch.

## Cards

### RENDERER

Shows the active renderer badge — **Flash** or **Forge** — with a segmented `Flash | Forge` toggle to switch renderers live while your app runs.

- **flash** — lightweight direct renderer, full clear each frame. The production default.
- **forge** — hybrid retained-tile compositor with damage tracking. Beta; toggle it here for testing.

Production picks the renderer at compile time via the `"renderer"` [config key](../getting-started/configuration.md) (dead code eliminates the other one). Only the dev runtime compiles both, which is what makes the live switch possible.

See [Flash](../rendering/flash.md) and [Forge](../rendering/forge.md) for how each renderer works, and the [Forge roadmap](../future/forge-renderer.md) for what's planned.

### FRAME

- Frame number since launch
- Total nodes in the tree

### LAYOUT

- Layout count this frame
- Skipped count and percentage

A high skip percentage is good — it means dirty rendering skipped clean nodes instead of re-laying-out the whole tree.

### PAINT

- Repainted node count this frame
- Display-list cache hit rate

### SAVINGS

Layout and paint savings percentages, color-coded: green when high, red when low.

## Highlight Repaints

The **Highlight repaints** toggle flashes every node repainted in the current frame. Use it to spot unexpected invalidations — a healthy app mostly repaints only what changed.
