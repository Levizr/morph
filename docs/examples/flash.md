# Flash Example

A minimal static app on the `flash` renderer — Morph's lightweight direct rasterizer.

## Files

| File | Description |
|---|---|
| `src/App.mx` | Static layout: title, cards, hover button |
| `src/style.css` | Dark theme, flex row, `:hover` transition |
| `morph.config.json` | Window size (480×360), `renderer: "flash"` |

## Features Demonstrated

- `renderer: "flash"` — compile-time backend pick, smallest binary
- Static text layout with flexbox
- `:hover` transition on the button

## Run

```bash
morph run
```

See the [full README](../../examples/flash/README.md) for more details.
