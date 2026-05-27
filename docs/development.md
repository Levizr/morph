# Development Guide

## Setup

```bash
# Clone and install
git clone https://github.com/levizr/morph
cd morph
pip install -e ".[dev]"

# Verify dependencies
morph doctor

# Run tests
python -m pytest tests/ -v
```

## Project Layout

```
morph/                        # Python toolchain
├── cli/                      # Command-line interface
│   ├── main.py               #   argparse dispatch
│   ├── cmd_init.py           #   morph init
│   ├── cmd_dev.py            #   morph dev
│   ├── cmd_build.py          #   morph build (TODO)
│   ├── cmd_doctor.py         #   morph doctor
│   ├── cmd_pkg.py            #   morph pkg
│   └── cmd_cache.py          #   morph cache
├── parser/                   # .mx file parsing
│   ├── morph_parser.py       #   tree-sitter AST builder
│   ├── jsx_walker.py         #   AST → imports + components
│   └── errors.py             #   custom exceptions
├── style/                    # CSS pipeline
│   ├── css_parser.py         #   tree-sitter CSS parser
│   ├── css_fetcher.py        #   remote CSS download + cache
│   ├── tailwind.py           #   Tailwind class resolver
│   ├── resolver.py           #   cascade + specificity (STUB)
│   ├── properties.py         #   known CSS property defaults
│   ├── sheet.py              #   StyleRule/StyleSheet dataclasses
│   └── units.py              #   unit conversion (px, %, em)
├── ir/                       # Intermediate Representation
│   ├── builder.py            #   walked AST → IR (implemented)
│   ├── node.py               #   IRNode/IRWindow/IRPage/IRViewport
│   ├── style.py              #   IRStyle dataclass
│   ├── event.py              #   IREvent dataclass
│   └── serializer.py         #   IR → JSON dict
├── layout/                   # Layout engine
│   ├── engine.py             #   vertical stacking (basic)
│   ├── flex.py               #   flexbox (STUB)
│   ├── box.py                #   box model (partial)
│   └── units.py              #   layout unit resolution (minimal)
├── dom/                      # DOM model
│   ├── node.py               #   DOMNode/TextNode/ElementNode
│   ├── tree.py               #   DOMTree with walk()
│   ├── attributes.py         #   morph-open/close/navigate parsing
│   └── query.py              #   DOM query (partial)
├── js/                       # JavaScript interpretation
│   ├── walker.py             #   generic AST visitor
│   ├── interpreter.py        #   JS semantics → intents (STUB)
│   ├── bridge.py             #   intents → IREvent
│   └── builtins.py           #   morph JS builtins map
├── codegen/                  # C++ code generation
│   ├── emitter.py            #   Jinja2 rendering engine
│   ├── node_emitter.py       #   IR node → C++ (style inheritance, color cascade)
│   ├── event_emitter.py      #   event handlers → C++ lambdas
│   ├── feature_set.py        #   scan IR for required C++ headers
│   └── templates/            #   Jinja2 templates
├── dev/                      # Dev mode
│   ├── pipeline.py           #   full .mx → IR pipeline
│   ├── devrt.py              #   dev runtime launcher
│   ├── watcher.py            #   file watcher
│   └── server.py             #   Unix socket IPC
├── pkg/                      # Package system
│   ├── manager.py            #   CLI operations (partial)
│   ├── installer.py          #   tarball download + extract
│   ├── registry.py           #   package registry client
│   ├── manifest.py           #   morph.pkg.json parser
│   └── resolver.py           #   dependency resolution (STUB)
├── config/                   # Project config
│   ├── schema.py             #   MorphConfig dataclass
│   └── loader.py             #   morph.config.json reader
├── build/                    # Build compiler
│   ├── compiler.py           #   g++ invocation with FreeType/GLFW flags
│   └── optimizer.py          #   binary optimization (stub)
└── utils/                    # Shared utilities
    ├── logger.py             #   colored terminal output
    ├── platform.py           #   OS detection
    ├── fs.py                 #   file helpers
    └── color.py              #   hex/rgb color parsing

tests/                        # Test suite
├── unit/                     #   unit tests
└── integration/              #   integration tests

runtime/                      # C++ runtime headers
├── core/                     #   MorphNode, Window, Renderer, Event system
│   ├── node.h / node.cpp     #   MorphNode base + layout + margin auto re-resolution
│   ├── window.h / window.cpp #   GLFW window wrapper, ortho projection, render loop
│   ├── renderer.h            #   Renderer interface
│   ├── window_manager.h      #   Multi-window management
│   └── event.h               #   Event system (MouseDown/Up/Move, Click, Scroll)
├── render/                   #   OpenGL renderer
│   ├── gl_renderer.h / .cpp  #   Batch renderer (VAO/VBO/IBO), stencil ops, border batch
│   └── shader.h              #   GLSL shaders: SDF rounded rect, text glyph atlas
├── widgets/                  #   Node types
│   ├── morph_rect.h          #   RectNode (div, span, h1–h6) with overflow clipping
│   ├── morph_text.h          #   TextNode with FreeType glyph atlas + word-wrap
│   ├── morph_radius.h        #   RoundedRectNode
│   ├── morph_button.h        #   ButtonNode with onClick
│   └── morph_image.h         #   ImageNode (stb_image, stencil clip, border rendering)
├── style/features/           #   Optional style features
│   └── base.h                #   StyleBase with marginAuto[4] flags
├── dev/                      #   Dev mode: main.cpp, socket, JSON parser, deserializer, inspector
│   ├── main.cpp              #   Entry point: socket → IPC → GLFW → render loop
│   ├── dev_socket.h / .cpp   #   Unix socket server
│   ├── ir_deserializer.h     #   JSON → MorphNode tree
│   ├── json_parser.h         #   Minimal JSON parser
│   └── inspector.h           #   DevTools overlay + panel
└── vendor/                   #   Dependencies
    ├── glad/glad.h / .cpp    #   GLAD OpenGL 3.3 loader
    └── stb_image.c           #   Image loading

templates/default/            # morph init scaffolding
my-app/                       # sample project
```

## Key Design Decisions

### Pipeline Overview

```
.mx file ──► MorphParser ──► AST ──► JSXWalker ──► walked dict
                                                          │
                                              CSS files ──┤ ──► IRBuilder ──► LayoutEngine ──► IR dict
                                          Tailwind ──────┘                           │
                                                                         ┌────────────┴────────────┐
                                                                         ▼                         ▼
                                                                  [Dev: IPC Socket]         [Build: Codegen]
                                                                  morph_devrt binary      g++ → production binary
```

### .mx Files Instead of Separate HTML/CSS/JS

The original design used separate `.html`, `.css`, `.js` files. The current codebase uses single `.mx` files with JSX-like syntax parsed by tree-sitter. CSS and JS can be imported within the `.mx` file or loaded from external files.

### Tree-sitter Over Hand-written Parsers

Both the `.mx` parser and CSS parser use tree-sitter grammars instead of hand-written lexers/parsers. This gives us robust syntax error handling and support for evolving the language syntax without rewriting parsing logic. The `morph/lexer/` package referenced in old tests no longer exists.

### No Runtime Dependencies

The final binary has zero Python, zero Node.js, and zero browser engine. Python is build-time only. The C++ runtime uses GLFW + OpenGL directly.

### Two-pass Style Resolution

Style resolution happens in two phases:
1. **Build-time** — CSS files are parsed and Tailwind classes are resolved into CSS property dicts
2. **IR-time** — The resolved CSS is matched against elements (selector matching, cascade, specificity) to produce final `IRStyle` objects

The second phase is currently stubbed in `morph/style/resolver.py`.

## Common Tasks

### Adding a New CSS Property

1. Add default value to `morph/style/properties.py`
2. Add typed field to `morph/ir/style.py` (`IRStyle`)
3. Add conversion logic in `morph/ir/builder.py` (when implemented)
4. Add to Jinja2 template in `morph/codegen/templates/` if needed for C++ output

### Adding a New HTML Element

1. No parser changes needed — tree-sitter handles arbitrary tag names
2. Add rendering logic to the relevant C++ template or node emitter
3. If it's a special morph element (like `morph-window`), handle it in `IRBuilder.build()`

### Adding a Tailwind Class

Add the class to the `STATIC_MAP` dict in `morph/style/tailwind.py`. Follow the existing pattern:

```python
"bg-red-500": {"background-color": "#ef4444"},
```

### Stencil-based Border-radius Clipping

Border-radius clipping uses stencil buffers. The nested stencil test uses `GL_INCR` (increment operator) so that when a child with border-radius is inside a parent with border-radius/overflow clipping, the inner clip properly intersects with the ancestor mask (0→1→2→...). `endRoundedClip` restores the parent's stencil function rather than always setting `GL_EQUAL, 1`.

For images, the border ring (`drawBorderRing()`) is drawn inside the stencil scope (before `endRoundedClip`) so the border is visible at the correct stencil level.

### Image Rendering Pipeline

Images are loaded via `stb_image` and cached in a texture map keyed by `src` path. They are batched by texture ID: `m_imageBatches: unordered_map<GLuint, vector<ImageInstance>>`. This avoids binding the same texture multiple times per frame.

### Border Rendering

Borders are rendered as SDF rings via a dedicated `m_borderBatch` flushed last — on top of fills, text, and images. Each element splits into a fill draw and a border ring draw.

### Margin Auto at Runtime

`margin: auto` horizontal centering is re-resolved dynamically on window resize:
- **Build-time**: `margin_auto[4]` flags serialized, auto margins stored as `-1.0f` sentinel
- **Runtime**: `MorphNode::layout()` detects sentinel + flags → computes centering each frame

### Dev Binary Source Hash

The dev binary (`morph_devrt`) is auto-rebuilt via CMake when source files change. The hash covers `runtime/dev/`, `runtime/core/`, `runtime/render/`, `runtime/widgets/`, and `runtime/style/` — so changes to shared runtime files also trigger a rebuild.

## DevTools (`morph_devrt`)

The dev runtime (`morph_devrt`) includes a browser-style DevTools panel for inspecting elements.

### Opening DevTools

- Press **`F12`** to toggle the panel open/closed
- Press **`F2`** or click the **"Inspect Element"** button to enter inspect mode
- Hover over elements in the window while inspecting — an overlay highlights the element's box model

### Overlay Colors

The overlay draws translucent colored rectangles around the hovered element:

| Color   | Layer   | Description                    |
|---------|---------|--------------------------------|
| Orange  | Margin  | Area outside the border        |
| Yellow  | Border  | Border area (if any)           |
| Green   | Padding | Area inside the border         |
| Blue    | Content | Content area (innermost)       |

### Panel Info

The panel (300px wide, right side) shows:

| Section    | Fields                                                          |
|------------|-----------------------------------------------------------------|
| **ELEMENT** | Tag name badge (e.g. `<div>`)                                   |
| **LAYOUT**  | Size (w × h), Position (x, y), Margin (T/R/B/L), Padding (T/R/B/L) |
| **DISPLAY** | Display, Overflow, Box Sizing                                   |
| **STYLE**   | Color (hex swatch), Background (hex swatch), Font Size, Weight, Align |

### Color Values

Colors are displayed as hex (`#334155`) when opaque, or `rgba(R,G,B,A)` when semi-transparent. A color swatch is shown alongside each value.

### State Persistence

- **On hot reload**: DevTools state (open/closed, inspect mode) is preserved. The hovered element reference is cleared since the tree is rebuilt.
- **On resize**: DevTools panel width is fixed at 300px.

### Implementation

All DevTools code is in `runtime/dev/inspector.h`. The overlay and panel are rendered as GL quads and text via `GLRenderer`. The global `DevTools*` pointer is set in `runtime/dev/main.cpp` for GLFW key/mouse callbacks.

## Debugging

- **`morph dev`** — runs the full pipeline; check terminal output for errors
- **`morph doctor`** — verify system dependencies
- **`python -m pytest tests/ -v -k <test_name>`** — run specific test
- **Pipeline logs** — errors are printed via `morph/utils/logger.py` with colored output
- **No IR output?** — Check `morph/ir/builder.py` for errors in the style resolution or node building logic
