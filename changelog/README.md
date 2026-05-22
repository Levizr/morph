# Changelog

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
