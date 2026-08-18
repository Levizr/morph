# Cull Test

Viewport culling stress test with 870+ nodes — 600 scrollable rows, a 120-item flex grid, and 150 off-screen side elements positioned at `left: 6000px`.

## What it tests

- Viewport culling skips off-screen children during draw and event dispatch
- Scroll container (`overflow: scroll`) with large child lists
- Nested layout with absolute positioning
- `MORPH_FEATURE_VIEWPORT_CULLING` feature gate

## Run

```bash
cd tests/runtime/culltest
morph dev
# or
morph run
```
