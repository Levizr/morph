# Changelog

## [0.0.5] - 2026-05-27
### Changed
- Restructured project layout for PyPI: runtime/, templates/, bin/ moved into morph/ package
- Source-hash based auto-rebuild for dev runtime binary on source change
- Modernized dev mode logs with per-step timing (parse, walk, build IR, layout, serialize)
- Auto-exit dev mode when GLFW window is closed
- Suppressed GLFW stderr noise from dev runtime
- Updated CLI commands (dev, doctor) for better UX and diagnostics
- `MorphNode::layout()` now clears `LayoutDirty`/`StyleDirty` at end — prevents redundant re-layout from `layoutIfNeeded` on nodes already positioned by parent (flex, inline, absolute)

### Added
- morph doctor: system dependency checks (FreeType, X11, GLFW, bundled vendor files)
- .devrt_source_hash tracking for incremental dev runtime rebuilds
- MANIFEST.in for proper PyPI package data inclusion
- Package metadata: readme, license, project URLs
- **Dirty incremental rendering system** — `DirtyFlag` enum (LayoutDirty, StyleDirty, PaintDirty, ScrollDirty, SubtreeDirty); `markDirty()` propagates flags up the tree for correct ancestor invalidation; `layoutIfNeeded()` only processes dirty/subtree-dirty nodes, skipping clean ones; `recordPaintTree()` selectively re-records display lists only for paint-dirty nodes; `executeDisplayList()` replays cached `m_displayList` for all nodes each frame; compile-time `MORPH_FEATURE_DIRTY_RENDERING` gate auto-enabled when scroll, hover, event, or cursor features are detected
- **`DirtyStats` struct** — tracks `layoutCount`, `paintCount`, `fullTreeCount`, `skippedCount` per frame for diagnostics
- **DevTools Rendering tab** — new second tab (F12, tabs: Elements / Rendering) showing frame number, total nodes, layout count/skipped/percentage, repainted count/cache hit, layout/paint savings percentages with green/red color coding
- **CSS `:hover` pseudo-class support** — class-based hover rules from CSS files `.className:hover` resolved at runtime; snap-style swap on mouse enter/leave with `hoverStyle` pointer and `m_baseStyle` snapshot for restoration; supports color, background-color, margin, padding, border, border-radius, font-size, gap, justify-content, align-items, width, height
- **`m_computedMargin[4]`** — separate field on `MorphNode` stores resolved margin values after each `layout()` call, keeping `style.margin[]` declared values intact for flex positioning code; used by DevTools inspector overlay + panel for correct display

### Fixed
- CMake output path for dev runtime binary after project restructure
- setuptools PEP 639 metadata compatibility (pinned setuptools <70)
- **Inspector margin panel showing 0** — root cause was `layoutIfNeeded` calling a third `layout()` on children with `parentW = childW = 204px` (child's own width instead of container width), causing auto-margin resolution with zero available horizontal space; fixed by clearing dirty flags at end of `layout()`

## [0.0.4] - 2026-05-26
### Added
- **DevTools panel** — Press `F12` to toggle; element inspect via `F2` or click button; box-model overlay with non-overlapping colored rings (margin/orange, border/yellow, padding/green, content/blue)
- **Element info panel** — Shows tag name badge, size, position, margin/padding (T/R/B/L), display, overflow, box-sizing, color/background with hex swatches, font-size, font-weight, text-align
- **Color swatches** — Small colored squares next to color and background values; opaque colors shown as hex (`#334155`), semi-transparent as `rgba(R,G,B,A)`
- **Mouse callback support** — Click "Inspect Element" button to toggle inspect mode
- **`MorphNode::type` field** — Stores element tag name ("div", "button", etc.) displayed in DevTools badge
- **`<img>` tag support** — Full image pipeline: `ImageNode` with lazy texture load, intrinsic aspect-ratio layout, `stb_image` decoder (PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC), per-texture-ID batched rendering (unlimited images), `border-radius` stencil clipping on images
- **Image texture cache** — Shared GPU textures keyed by `src` path, zero duplicate memory
- **`width`/`height` HTML attributes on `<img>`** — Converted to CSS with correct cascade priority (inline > attributes > Tailwind > CSS rules > UA defaults)
- **`border` CSS shorthand** — `border: "2px solid #fff"` expanded to individual `border-width`/`border-style`/`border-color` in `builder.py`
- **Global border ring batch** — Borders now render via a separate `m_borderBatch` (flushed last, on top of everything). SDF shader supports `borderOnly` mode: renders just the ring (no fill) with proper rounded inner/outer corners. Every element (`div`, `button`, `img`, etc.) uses the same `drawBorderRing` path for consistent rendering.
- **`drawBorderRing` API** — New method on `Renderer` base class; takes position, radius, border-width, border-color. Used by `RectNode`, `ButtonNode`, and `ImageNode`.
- **Border on `<img/>`** — Images now get borders via the global SDF ring pipeline, not widget-specific quad hacks. Border ring is on top of the image texture (correct z-order).

### Fixed
- **Empty div rendering** — Auto-height formula initialized `maxBottom = 0` instead of `cy`; empty divs with padding had zero height. Fixed at `runtime/core/node.cpp:124`.
- **Overlay color blending** — Changed from overlapping filled quads to non-overlapping rings, each box fills only its exclusive area with no alpha bleed
- **DevTools panel layout** — Consistent 2-column alignment (label at `px+14`, value at `px+85`); swatch at far right; no text/swatch overlap
- **Radius clamp in shader** — `border-radius > 100px` caused no rendering due to SDF degeneration; clamped to `[0.001, 100.0]` in the SDF shader to prevent artifacts

## [0.0.3] - 2026-05-26

### Added
- **`morph_devrt` binary** — Pre-compiled C++ dev mode renderer at `morph/bin/morph_devrt`; GLFW window + Unix socket IPC + hot reload
- **Dev mode auto-build** — `morph dev` builds `morph_devrt` via CMake when binary is missing
- **Unix socket server** — `DevSocket` in C++ accepts IR JSON from Python side, swaps node tree without restarting window
- **JSON DOM parser** — Minimal recursive-descent JSON parser for dev runtime (`runtime/dev/json_parser.h`)
- **IR deserializer** — JSON → `MorphNode` tree with full style inheritance cascade (`runtime/dev/ir_deserializer.h`)
- **Hot reload** — Node tree swaps mid-frame; `glfwPollEvents()` loop checks socket between frames
- **Window config hot reload** — `MorphWindow::setTitle()` and `setSize()` methods; title updates on save
- **Box-sizing support** — `box-sizing: content-box` / `border-box` CSS property in layout engine
- **Border rendering** — `border-width`, `border-color`, `border-style` with SDF-based shader drawing
- **`position` property** — Parsing and codegen for `position`, `left`, `right`, `top`, `bottom`
- **Style inheritance in dev mode** — Deserializer applies `color`, `font-size`, `font-weight`, `text-align` cascade matching codegen output, ensuring pixel-identical dev/build rendering

### Fixed
- **Style inheritance in dev deserializer** — Raw IR values now resolve against parent style (color, font-size, font-weight, text-align), matching codegen in `node_emitter.py`
- **Height not reflecting in dev window** — Width/height from IR were parsed but never applied to GLFW window; fixed via `setSize()` (title only in hot reload to preserve dev flow)

### Changed
- **Dev socket protocol** — Null-terminated JSON messages over Unix domain socket at `/tmp/morph_dev.sock`
- **Dev mode pipeline** — `cmd_dev.py` launches `morph_devrt` subprocess, connects IPC client, sends IR on file change
- **`.mx` file watch** — Watcher now detects `.mx` changes; path resolved from `config.entry` instead of hardcoded `src/`

## [0.0.2] - 2026-05-22

### Added
- **cursor support** — `cursor: pointer` (hand) and `cursor: text` (I-beam) via GLFW standard cursors; `hitTest()` on `MorphNode` for deep node lookup under cursor
- **`textAlign` support** — `text-align: left|center|right` parsed, inherited, emitted; `TextNode` draw centers text within container width
- **`maxWidth` support** — `max-width` CSS property constrains child width in layout; affects text wrapping
- **Individual margin/padding sides** — `margin-top`, `margin-right`, `margin-bottom`, `margin-left`, `padding-top`, `padding-right`, `padding-left`, `padding-bottom` all parsed and merged into main margin/padding tuples
- **Flexbox content-based sizing** — `contentWidth()` virtual method on `MorphNode` for measuring intrinsic width; non-stretch flex children (`center`, `flex-end`) shrink to content size
- **Viewport culling** — children outside visible scroll area are skipped during draw and event dispatch; `overflow: auto/scroll` containers without explicit height clamp to parent height

### Fixed
- **`TextAlign::Center` rendering** — broken formula `penX -= totalW * 0.5f` centered on left edge, causing text to hang outside containers; now computed in `morph_text.h` using `x + (w - tw) * 0.5`
- **Flexbox row margin bug** — was using `mt+mb` (vertical margins) instead of `ml+mr` for total main size, preventing `justifyContent` from ever triggering
- **Flexbox child re-layout dimensions** — rows now pass `childMain` (content width) as parent width for re-layout (was using child's height)
- **`justifyContent`/`alignItems` on auto-sized containers** — guarded with `mainSize > totalMain + gapTotal` / `crossSize > crossDim` to prevent negative offsets
- **Spurious `flexDirection` on text nodes** — only emit `flexDirection` when `display == "flex"`
- **Scroll handling in dispatchEvent** — scroll/wheel only handled when cursor within container bounds; not-in-bounds falls through to children for nested scroll containers
- **Scrollbar Click/MouseDown conflict** — scrollbar consumes both events to prevent button onClick firing behind scrollbar
- **Feature guard `#define` placement** — was after `#include` in template, causing all `#ifdef` guards to fail silently; moved before includes

### Changed
- **Scroll offset system** — replaced mutable `child->y -= scrollY` hack with `Renderer::pushScrollOffset`/`popScrollOffset` stack; all draw calls automatically apply accumulated scroll offset
- **Two-pass flexbox layout** — pass 1 measures children at temp `(0,0)`, pass 2 re-calls `layout()` at final position so grandchildren correctly repositioned after `justifyContent`/`alignItems` adjustments
- **`drawRoundedRect` clamps radius** to `min(w,h)*0.5` to prevent SDF artifacts from values exceeding half-dimension
- **Binary size reduced ~19%** — template app from 137KB to 111KB (with all features) due to `-ffunction-sections -fdata-sections -Wl,--gc-sections`

### Infrastructure
- Feature-based conditional compilation — `FeatureSet` scans IR for `text`, `button`, `scroll`, `radius`, `bold`, `position`, `flex` and emits `#define MORPH_FEATURE_*` guards
- Dead code elimination via linker GC; FreeType only linked when `needs_freetype()` is true
- All runtime headers guard FreeType, scroll dispatch, `drawRoundedRect`, text shaders with `#ifdef MORPH_FEATURE_*`
