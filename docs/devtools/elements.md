# Elements

The Elements tab inspects what's actually on screen — the native node tree, not a DOM.

## Inspect Mode

Enter inspect mode two ways:

- Press **`F2`**
- Click **Inspect Element** in the panel header

While inspecting, hovering over the window highlights the element under the cursor with a box-model overlay, and the panel fills with that element's details.

## Box-Model Overlay

The overlay draws four colored rings around the hovered element. The rings are non-overlapping — each box fills only its exclusive area, so translucent colors never bleed into each other:

| Color | Layer | Description |
|---|---|---|
| Orange | Margin | Area outside the border |
| Yellow | Border | Border area (if any) |
| Green | Padding | Area inside the border |
| Blue | Content | Content area (innermost) |

## Info Panel

The hovered element's details appear as cards in the panel:

- **Tag badge + breadcrumb trail** — the element type (`div`, `button`, ...) and its ancestor chain
- **LAYOUT card** — size, position, margin (T/R/B/L), padding (T/R/B/L)
- **DISPLAY card** — display, overflow, box-sizing
- **STYLE card** — color and background with hex swatches, font-size, font-weight, text-align

Color values show as hex for opaque colors (`#334155`) and `rgba(R,G,B,A)` for semi-transparent ones, each with a small swatch next to the value.

> The margin values shown are the computed values resolved by layout — useful when you use `margin: auto` or flex spacing, where the declared CSS differs from what actually renders.
