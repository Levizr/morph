# Forge Example

A scrollable list on the `forge` renderer — Morph's hybrid retained compositor.

## Files

| File | Description |
|---|---|
| `src/App.mx` | Header plus 24-row scrollable list, hover button |
| `src/style.css` | Dark theme, `overflow: auto` list, row `:hover` transitions |
| `morph.config.json` | Window size (600×520), `renderer: "forge"` |

## Features Demonstrated

- `renderer: "forge"` — compile-time backend pick, retained surfaces
- Scrollable list exercising scroll-shift tile reuse (exposed strip only)
- Hover transitions exercising retained-layer promotion for animated leaves
- Open DevTools → Rendering to watch damage area, present bytes, tile
  residency, and the damage overlay live

## Run

```bash
morph run
```

See the [full README](../../examples/forge/README.md) for more details.
