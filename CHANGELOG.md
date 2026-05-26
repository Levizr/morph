# Changelog

## v0.0.4 (2026-05-26)

### Added
- DevTools panel (`F12` toggle) — element inspect, box-model overlay, info panel
- Element name badge in DevTools panel
- Color swatches with hex/rgba display for color and background values
- Click-to-inspect on "Inspect Element" button
- Mouse callback support for DevTools UI interactions
- `MorphNode::type` field to store element tag name
- DevTools documentation in `docs/development.md`

### Fixed
- Empty div with padding now renders (auto-height bug: `maxBottom` initialized to `0` instead of `cy`)
- DevTools panel layout: consistent 2-column alignment, no text/swatch overlap
- Formatting: opaque colors shown as hex (`#334155`) instead of raw rgba

### Changed
- Bumped version to `0.0.4`
- Updated README with DevTools features and C++ runtime notes

## v0.0.3 (2026-05-25)

### Added
- `morph_devrt` binary — standalone C++ dev runtime with hot reload
- Unix socket IPC between Python toolchain and C++ renderer
- IR deserializer with style inheritance cascade
- Window config hot reload (title update without restart)
- Dev mode auto-build via CMake
- Border rendering (`border-width`, `border-color`) via SDF shader
- Box-sizing CSS property (`content-box` / `border-box`)
- Inspector.h overlay + panel (initial version)
- `MorphWindow::setTitle()` / `setSize()` API

## v0.0.2 (2026-05-15)

### Added
- Scrollbar with drag, wheel, track-click, nested containers
- Viewport culling for draw + event dispatch
- Feature-based compilation (`#define MORPH_FEATURE_*`)
- Flexbox layout (`justify-content`, `align-items`, content-based sizing)
- `text-align` CSS property (left, center, right)
- `max-width` layout constraint
- Margin/padding individual side properties
- Cursor support (`pointer`, `text`) via GLFW cursors
- Text measurement and automatic word wrapping in TextNode
- `console.log()` support in onClick handlers

### Fixed
- Button centering in flexbox
- contentWidth for flex rows
- Text maxWidth constraint
- Button border-radius rendering

## v0.0.1 (2026-05-01)

### Added
- Initial release
- `.mx` file parsing via tree-sitter
- CSS parsing (local files, remote URLs, MD5-cached)
- Tailwind CSS (500+ utility classes)
- IRBuilder — AST to Intermediate Representation
- CLI: `init`, `dev`, `build`, `pkg`, `doctor`, `cache`
- Config system (`morph.config.json`)
- Layout engine (box model, vertical stacking, flexbox)
- Unix socket IPC for dev mode
- C++ node emitter via Jinja2 templates
- Build compiler (g++ with FreeType/GLFW/OpenGL flags)
- Package registry client
- OpenGL 3.3 batch renderer (instanced VAO/VBO/IBO)
- FreeType text rendering with glyph atlas
- Rounded rectangles via SDF fragment shader
- Font weight support (bold/normal)
- Style inheritance (color, font-size, font-weight, text-align)
- Transparent backgrounds by default
- Event system (onClick, MouseDown, MouseUp, MouseMove)
- RectNode, TextNode, ButtonNode widgets
