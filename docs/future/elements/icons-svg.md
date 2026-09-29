# Icons & SVG — Tiny Pictures, Surprisingly Deep Rabbit Hole

**Status:** future · **Priority:** medium · **Depends on:** [Packages](../tooling/packages.md) (distribution), [Graphics APIs](../rendering/graphics-apis.md) (GPU path choice)

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Every app needs icons; nobody wants to hand-roll them. Today a Morph dev's options are "draw it with divs" or "ship PNGs at 6 resolutions and pray." Meanwhile the renderer's SDF shader stack (rounded rects today) is *exactly* the machinery vector icons want. This page turns the `morph-icons` package sketch from [Tooling](../tooling/vscode.md) into a full design: first SVG, then the icon system on top.

## Why it matters

- **PNGs don't scale** — HiDPI made fixed-resolution assets a bug factory; vectors scale to any density for free
- **Icons are a dependency decision** — devs pick frameworks partly by "does it have an icon story"; a first-party set removes a whole evaluation row
- **SDF synergy** — signed-distance-field icon rendering gives crisp edges at any size with one cheap shader path, reusing the existing rounded-rect SDF work instead of adding a raster pipeline

## How it will work

### Phase 1 — `<svg>` element + path subset

```tsx
<svg width={24} height={24} viewBox="0 0 24 24">
  <path d="M12 2 L2 22 H22 Z" fill="currentColor" />
</svg>
```

Supported subset first: `path` (M/L/C/Z + fill), `rect`, `circle`, `viewBox`, `fill="currentColor"` (inherits text color — theming for free). Tessellate to triangles on the CPU at load; cache per (path, size) like the glyph cache. Full SVG (gradients, masks, text-on-path) is explicitly *not* v1 — that's a browser, and we are not building a browser. Again.

### Phase 2 — `morph-icons` package

```tsx
import { SearchIcon, CloseIcon } from 'morph-icons'

<SearchIcon size={20} color="muted" />
```

- Tree-shaken at compile time (unused icons never reach the binary — the module graph already knows what's imported)
- Sized in logical pixels, SDF-crisp at every density
- `currentColor` default so icons follow text styling with zero props

### Phase 3 — Custom icon sets

```bash
morph icons:add ./assets/my-icons/*.svg   # compiles a project set into the binary
```

Validates paths at build time (`morph check` rejects unsupported SVG features with the offending element named — no silent missing icons at runtime).

## Current state

| Piece | State |
|---|---|
| SDF shader stack (rounded rects) | ✅ Shipped |
| FreeType text pipeline (icon-font fallback possible) | ✅ Shipped |
| `<svg>` element + path tessellation | ❌ Not built |
| `morph-icons` package | ❌ Not started (sketch in [Tooling](../tooling/vscode.md)) |
| `morph icons:add` custom sets | ❌ Not built |

## Open questions

- **CPU tessellate vs GPU SDF** — tessellation is simpler and matches the glyph-cache pattern; SDF paths are crisper under animation. Tessellate first, SDF for icons later?
- **Subset boundary** — which SVG features make v1? (`path`/`rect`/`circle`/`currentColor` proposed; strokes? transforms?)
- **Icon-font interop** — support icon fonts as a fallback, or vectors-only and never look back?
- **Color icons (emoji)** — COLR/CPAL color fonts via FreeType, or out of scope? (Out of scope initially — monochrome icons cover 95% of UI.)

## Build steps (when picked up)

1. `<svg>` + path/rect/circle subset with CPU tessellation + cache
2. `currentColor` inheritance + `morph check` SVG validation
3. `morph-icons` first-party set (tree-shaken, SDF-crisp)
4. `morph icons:add` for project sets
5. Validation app: icon browser showing the full set at 5 sizes on HiDPI
