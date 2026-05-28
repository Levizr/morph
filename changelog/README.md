# Changelog

## [0.0.6] - 2026-05-28
### Changed
- **Default `<body>` margin → padding** — `_UA_DEFAULTS["body"]` switched from `margin: 8px` to `padding: 8px`. In browsers the margin creates white gaps around the window edges because the body background doesn't paint into margin space. Since Morph has zero backward-compat constraints, padding is the better default: the body background fills edge-to-edge, and internal spacing still works. Users can override with any CSS rule (e.g. `body { margin: 8px; padding: 0; }`).
- **Window clear color matches body** — `MorphWindow::render()` now reads `m_root->style.bgColor` and sets `glClearColor` before each `clear()` call, falling back to `(1,1,1,1)` when the body is transparent. Eliminates the color-mismatch glitch visible during window resize.

### Added
- **CSS transition build-mode fix** — `cmd_build.py` `_deser_node()` now passes `transition_duration` and `transition_easing` when reconstructing IRNode objects from the JSON pipeline output. Previously these fields were silently dropped, causing build mode to always produce zero-duration (no transition) nodes even when CSS `transition` was specified.

### How CSS Transitions Work
Transitions animate style changes when `:hover` activates. Configured via standard CSS on any element:

```css
.card {
  transition: all 0.3s ease-in-out;
}
/* or individually: */
.swatch {
  transition-duration: 0.2s;
  transition-timing-function: ease;
}
```

**Pipeline:**
1. Python IR builder parses `transition`, `transition-duration`, `transition-timing-function` from CSS merged cascade and stores them as `IRNode.transition_duration` / `transition_easing`.
2. In dev mode, the C++ deserializer reads these fields from JSON IR and sets `m_transitionDuration` / `m_transitionEasing` on the node.
3. In build mode, the C++ codegen emits `node->m_transitionDuration = 0.2f;` / `node->m_transitionEasing = Easing::EaseInOut;` for each node.
4. At runtime, `onHover()` allocates a `HoverTransition` struct (heap pointer `m_hoverTransition`, null when idle) capturing current style as start and the target (`hoverStyle` on enter, `m_baseStyle` on leave).
5. `updateHoverTransition(dt)` runs each frame: advances `elapsed`, applies easing function, calls `interpolateStyles(startStyle, targetStyle, t, &out)`.
6. `interpolateStyles()` lerps all numeric/color properties (`bgColor`, `color`, `margin`, `padding`, `border`, `borderRadius`, `fontSize`, `gap`, `width`/`height`, position offsets, scrollbar props). String/bool properties (`display`, `position`, `flexDirection`, `fontWeight`, `overflow`, `textAlign`, `boxSizing`, `borderStyle`, `cursor`, `marginAuto`) snap to target immediately. Width/height lerp only when both start and target are explicit (≥ 0).
7. `HoverTransition` is deleted on completion. Mid-transition direction changes (hover leave during entry) capture current interpolated style as new start for smooth reversal.
8. Easing: `Easing::Linear`, `EaseIn`, `EaseOut`, `EaseInOut` (extensible enum). Duration 0 disables transitions (backward-compatible snap behavior).

Key design decisions:
- Heap-allocated `HoverTransition` only during active transition — zero memory overhead when idle.
- Transitions run per-node: parent and child can transition independently.
- String/display properties snap instantly because changing layout mode mid-animation produces undefined intermediate states.
- `updateHoverTransition()` runs before `updateAnimations()` in `update()` — explicit animations win if both target the same property.

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
- **CSS transition support** — `transition-duration` / `transition-timing-function` CSS properties and `transition` shorthand; `HoverTransition` system on `MorphNode` with start/target style interpolation; `interpolateStyles()` lerps all numeric/color properties (bgColor, color, margin, padding, border, borderRadius, fontSize, gap, width/height, etc.) while snapping string properties (display, position, flex-direction, etc.) instantly; Easing enum with Linear, EaseIn, EaseOut, EaseInOut; configurable per-element via CSS transition properties; 4 easing functions with extensible enum for future additions
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
