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
│   ├── cmd_build.py          #   morph build / --static
│   ├── cmd_run.py            #   morph run
│   ├── cmd_doctor.py         #   morph doctor (system checks + auto-install)
│   ├── cmd_pkg.py            #   morph pkg
│   ├── cmd_cache.py          #   morph cache
│   └── cmd_translate.py      #   morph translate (TS → C++)
├── parser/                   # .mx file parsing
│   ├── morph_parser.py       #   tree-sitter AST builder (TS grammar)
│   ├── jsx_walker.py         #   AST → imports + components
│   └── errors.py             #   custom exceptions
├── style/                    # CSS pipeline
│   ├── css_parser.py         #   tree-sitter CSS parser
│   ├── css_fetcher.py        #   remote CSS download + MD5 cache
│   ├── tailwind.py           #   Tailwind class resolver
│   ├── selector.py           #   selector parsing + specificity + ancestor-hover
│   ├── resolver.py           #   runtime cascade (being built out)
│   ├── properties.py         #   known CSS property defaults
│   ├── sheet.py              #   StyleRule/StyleSheet dataclasses
│   └── units.py              #   unit conversion (px, %, em)
├── ir/                       # Intermediate Representation
│   ├── builder.py            #   walked AST → IR
│   ├── node.py               #   IRNode/IRWindow/IRPage/IRViewport
│   ├── style.py              #   IRStyle dataclass
│   ├── event.py              #   IREvent dataclass
│   └── serializer.py         #   IR → JSON dict
├── layout/                   # Layout engine
│   ├── engine.py             #   box model, vertical stacking
│   ├── flex.py               #   flexbox (wrap, grow/shrink, justify/align)
│   ├── inline.py             #   inline measure pass + line breaking
│   ├── box.py                #   box model helpers
│   └── units.py              #   layout unit resolution
├── js/                       # TypeScript → C++ translation
│   ├── ast.py                #   JS AST node types
│   ├── ast_builder.py        #   tree-sitter TS AST → JS AST
│   ├── codegen.py            #   JS AST → C++ (TS→C++ translator)
│   ├── interpreter.py        #   interactive JS interpretation (future)
│   ├── bridge.py             #   legacy intents → IREvent
│   └── builtins.py           #   morph JS builtins map
├── codegen/                  # C++ code generation
│   ├── emitter.py            #   Jinja2 rendering engine
│   ├── node_emitter.py       #   IR node → C++ (style inheritance, color cascade)
│   ├── logic_emitter.py      #   translated logic → C++ (signals/effects/handlers)
│   ├── event_emitter.py      #   event handlers → C++ lambdas
│   ├── feature_set.py        #   scan IR for required C++ headers/features
│   └── templates/            #   Jinja2 templates
├── dev/                      # Dev mode
│   ├── pipeline.py           #   full .mx → IR pipeline
│   ├── devrt.py              #   dev runtime launcher
│   ├── watcher.py            #   file watcher (settle + content-hash skip)
│   └── server.py             #   Unix socket IPC
├── pkg/                      # Package system
│   ├── manager.py            #   CLI operations
│   ├── installer.py          #   tarball download + extract
│   ├── registry.py           #   package registry client
│   ├── manifest.py           #   morph.pkg.json parser
│   └── resolver.py           #   dependency resolution
├── config/                   # Project config
│   ├── schema.py             #   MorphConfig dataclass
│   └── loader.py             #   morph.config.json reader
├── build/                    # Build compiler
│   ├── compiler.py           #   g++ invocation (FreeType/GLFW flags, --static)
│   └── optimizer.py          #   binary optimization
└── utils/                    # Shared utilities
    ├── logger.py             #   colored terminal output
    ├── platform.py           #   OS detection
    ├── fs.py                 #   file helpers
    └── color.py              #   hex/rgb color parsing

tests/                        # Test suite
├── unit/                     #   unit tests (parser, walker, css, tailwind, selector,
│                             #   layout, ir_builder, feature_set, js_codegen)
└── integration/              #   integration tests

runtime/                      # C++ runtime headers
├── core/                     #   Node, Window, Compositor, render frame, events
│   ├── node.h + node/        #   MorphNode base; split into node.cpp, layout.cpp,
│   │                         #   style.cpp, events.cpp, flatten.cpp, paint_order.cpp
│   ├── window.h / window.cpp #   GLFW window, ortho projection, compositor integration
│   ├── compositor.h / .cpp   #   render thread owning the GL context
│   ├── render_frame.h        #   lock-free frame snapshot (nodes + DrawOps + animations)
│   ├── draw_op.h             #   display-list draw operations
│   ├── spsc_queue.h          #   SPSC feedback queue (compositor → main)
│   ├── renderer.h            #   Renderer interface
│   ├── window_manager.h      #   multi-window management
│   └── event.h               #   event system (MouseDown/Up/Move, Click, Scroll)
├── render/                   #   OpenGL batch renderer
│   └── gl_renderer.h / .cpp  #   VAO/VBO/IBO, stencil ops, border batch
├── renderers/                #   Paint backends
│   ├── renderer.h / .cpp     #   RenderMode dispatch (Flash | Forge)
│   ├── flash/                #   flash: full-clear direct renderer
│   └── forge/                #   forge: retained FBO + DamageSet (damage.h, layer.h,
│                             #           tile.h, tile_pool.h)
├── shaders/                  #   GLSL shader sources (SDF rounded rect, text, border)
├── ui/                       #   Node types
│   ├── rect.h                #   RectNode (div, span, h1–h6, p) + overflow clipping
│   ├── text.h                #   TextNode (FreeType glyph atlas, word-wrap)
│   ├── button.h              #   ButtonNode with onClick + hover
│   ├── image.h               #   ImageNode (stb_image, stencil clip, border)
│   ├── input.h               #   InputNode (MORPH_FEATURE_INPUT)
│   ├── radius.h              #   RoundedRectNode (legacy)
│   ├── viewport_node.h       #   <morph-viewport> node (planned)
│   └── viewport_driver.h     #   viewport driver interface
├── types/                    #   JS runtime types (JsValue, JsNumber, JsString,
│                             #   JsArray, JsObject, JsBoolean)
├── reactivity/               #   Signal<T>, effects, coroutine tasks, timers, Result
├── net/                      #   fetch() HTTP client
├── style/features/           #   optional style feature structs (hover, zindex, flex, base)
├── dev/                      #   Dev mode: main.cpp, socket, JSON parser, deserializer,
│                             #   inspector, dev_log, dev_net, node_registry, signal_store
└── vendor/                   #   glad (GL loader), stb_image

templates/default/            # morph init scaffolding (incl. node_modules/morph TS types)
examples/                     # calculator, ipchecker, dynamic-styles example apps
help/                         # design docs (compositor thread, renderers, pipeline, etc.)
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
                                                                  + logic.so (dlopen)
```

### .mx Files Instead of Separate HTML/CSS/JS

The original design used separate `.html`, `.css`, `.js` files. The current codebase uses single `.mx` files with JSX-like syntax parsed by tree-sitter. CSS and JS can be imported within the `.mx` file or loaded from external files.

### Tree-sitter Over Hand-written Parsers

Both the `.mx` parser (TypeScript grammar) and CSS parser use tree-sitter grammars instead of hand-written lexers/parsers. This gives us robust syntax error handling and support for evolving the language syntax without rewriting parsing logic.

### No Runtime Dependencies

The final binary has zero Python, zero Node.js, and zero browser engine. Python is build-time only. The C++ runtime uses GLFW + OpenGL directly. In dev mode, the only dynamic piece is the `logic.<hash>.so` that hot reload re-wires in place.

### Compile-time TS→C++, not a JS Interpreter

Component logic is translated to C++ at compile time (`morph/js/`), giving native performance and zero interpreter overhead. An interactive JS interpreter remains on the roadmap for scripting use cases.

### Two-pass Style Resolution

Style resolution happens in two phases:

1. **Build-time** — CSS files are parsed and Tailwind classes are resolved into CSS property dicts
2. **IR-time** — The resolved CSS is matched against elements (selector matching, cascade, specificity) to produce final `IRStyle` objects. `morph/style/selector.py` handles combinator + specificity parsing; the runtime cascade is still being built out.

## Common Tasks

### Adding a New CSS Property

1. Add default value to `morph/style/properties.py`
2. Add typed field to `morph/ir/style.py` (`IRStyle`)
3. Add conversion logic in `morph/ir/builder.py`
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

The dev binary (`morph_devrt`) is auto-rebuilt via CMake when source files change. The hash covers the shared runtime — `runtime/dev/`, `runtime/core/`, `runtime/render/`, `runtime/ui/`, `runtime/style/`, `runtime/renderers/` — so changes to shared runtime files also trigger a rebuild.

### Docked DevTools (Content Area)

The DevTools panel is docked to the right side of the window. `MorphWindow::contentWidth()` returns `windowWidth − m_devtoolsWidth` (clamped ≥ 120px) and layout/rendering use it instead of the full window width, so the panel never covers app elements. F12 toggles the strip on/off; the panel's left-edge handle drags to resize (min 240px, app keeps ≥ 360px).

## DevTools (`morph_devrt`)

The dev runtime (`morph_devrt`) includes a browser-style DevTools panel for inspecting elements.

### Opening DevTools

- Press **`F12`** to toggle the panel open/closed
- Press **`F2`** or click the **"Inspect Element"** button to enter inspect mode
- Hover over elements in the window while inspecting — an overlay highlights the element's box model
- The panel is docked to the right side; drag its left edge to resize. The app layout is constrained to the remaining content area.

### Tabs

| Tab | Contents |
|---|---|
| **Elements** | Inspect button, tag badge, breadcrumb trail, LAYOUT / DISPLAY / STYLE cards with color swatches |
| **Rendering** | RENDERER card (active renderer badge — Flash/Forge — + segmented live switch in dev), FRAME / LAYOUT / PAINT / SAVINGS cards, "Highlight repaints" toggle switch |
| **Network** | Request summary (total / ok / err / bytes), per-request status dot + code, method, URL, duration, size; click a row to open a detail view with GENERAL / RESPONSE HEADERS / REQUEST HEADERS / BODY cards |
| **Logs** | Ring buffer of `info` / `ok` / `warn` / `error` entries with timestamps; clear button |

### Overlay Colors

The overlay draws translucent colored rectangles around the hovered element:

| Color   | Layer   | Description                    |
|---------|---------|--------------------------------|
| Orange  | Margin  | Area outside the border        |
| Yellow  | Border  | Border area (if any)           |
| Green   | Padding | Area inside the border         |
| Blue    | Content | Content area (innermost)       |

### State Persistence

- **On hot reload**: DevTools state (open/closed, inspect mode, active tab) is preserved. The hovered element reference is cleared since the tree is rebuilt.
- **On resize**: the panel width is preserved; the content area clamps so the app never collapses.

### Implementation

All DevTools code is in `runtime/dev/inspector.h` (panel), with `runtime/dev/dev_log.h` (log ring buffer) and `runtime/dev/dev_net.h` (network request ring buffer). The overlay and panel are rendered as GL quads and text via `GLRenderer`. The global `DevTools*` pointer is set in `runtime/dev/main.cpp` for GLFW key/mouse callbacks.

## Debugging

- **`morph dev`** — runs the full pipeline; check terminal output for errors
- **`morph doctor`** — verify system dependencies (with `-v` for versions; auto-installs via your package manager)
- **`python -m pytest tests/ -v -k <test_name>`** — run specific test
- **Pipeline logs** — errors are printed via `morph/utils/logger.py` with colored output
- **No IR output?** — Check `morph/ir/builder.py` for errors in the style resolution or node building logic
- **Window fails to open?** — GLFW init/creation errors print a clear Wayland/X11 fallback message; try `GDK_BACKEND=x11 morph dev`
