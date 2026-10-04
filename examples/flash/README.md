# Flash Example

A minimal static app on the `flash` renderer — Morph's lightweight direct
rasterizer. Small window, static text, one hover transition: the case where
full-frame redraw is cheapest.

## What it shows

- **`renderer: "flash"`** in `morph.config.json` — compile-time backend pick
- **Static layout** — text, cards, and a button with `:hover` feedback
- **When to use flash** — small apps and static UIs where simplicity beats
  retained surfaces (see [Choosing a Renderer](../../docs/rendering/index.md))

## Run

```bash
cd examples/flash
morph dev          # live window with hot reload
# or
morph run          # build + run optimized binary
```
