# Morph — Architecture

## Pipeline

```
.mx source ──► MorphParser ──► JSXWalker ──► IRBuilder ──► LayoutEngine ──► IRSerializer
                                                                                   │
                                                  ┌────────────────────────────────┴──────────────┐
                                                  ▼                                               ▼
                                          [Dev: IPC Socket]                              [Build: C++ Codegen]
                                          morph_devrt binary                       node_emitter → g++ → binary
```

| Step | Module | Input | Output | Status |
|---|---|---|---|---|
| 1 | `parser/morph_parser.py` | `.mx` source text | tree-sitter AST | ✅ |
| 2 | `parser/jsx_walker.py` | AST | `{imports, components}` dict | ✅ |
| 3 | `style/css_parser.py` | CSS files / URLs | `{selector: {prop: val}}` dict | ✅ |
| 4 | `style/tailwind.py` | class strings | resolved CSS dicts | ✅ |
| 5 | `ir/builder.py` | walked + CSS + Tailwind | `list[IRWindow]` | ✅ |
| 6 | `layout/engine.py` | `list[IRWindow]` | computed positions | ✅ |
| 7 | `ir/serializer.py` | `list[IRWindow]` | JSON-safe dict | ✅ |
| 8a | `codegen/node_emitter.py` (build) | `list[IRWindow]` | C++ source files | ✅ |
| 8b | `dev/server.py` (dev) | JSON dict | Unix socket → `morph_devrt` | ✅ |

## Module Boundaries

- `parser/` — tree-sitter AST building + JSX walking (no hand-written lexer)
- `style/` — CSS parsing, Tailwind resolution, cascade + specificity (stub)
- `ir/` — language-agnostic node tree + builder + serializer
- `layout/` — box model math (vertical stacking, flexbox, auto margins, percentage values)
- `codegen/` — IR → C++ via Jinja2 templates + node emitter
- `build/` — compiler invocation (g++ with FreeType/GLFW/OpenGL flags)
- `dev/` — IPC (Unix socket), file watcher, hot reload pipeline, dev binary rebuild
- `pkg/` — package registry client + installer
- `cli/` — terminal I/O, no business logic
- `utils/` — pure helpers (colors, units, logging), no morph imports

## C++ Runtime

```
runtime/
├── core/
│   ├── node.h / node.cpp          ─ MorphNode base + MorphStyle + layout dispatch
│   ├── window.h / window.cpp      ─ MorphWindow: GLFW wrapper, ortho proj, render loop
│   ├── renderer.h                 ─ Renderer interface
│   ├── window_manager.h           ─ Multi-window management
│   └── event.h                    ─ Event system (MouseDown/Up/Move, Click, Scroll)
├── render/
│   ├── gl_renderer.h / .cpp       ─ OpenGL 3.3 batch renderer (VAO/VBO/IBO, stencil, border batch)
│   └── shader.h                   ─ GLSL shaders (SDF rounded rect, text glyph atlas)
├── widgets/
│   ├── morph_rect.h               ─ RectNode (div, span, h1–h6, p, etc.) with overflow/border-radius child clipping
│   ├── morph_text.h               ─ TextNode with FreeType glyph atlas + word-wrap
│   ├── morph_radius.h             ─ RoundedRectNode (legacy, superseded by SDF shader)
│   ├── morph_button.h             ─ ButtonNode with onClick
│   └── morph_image.h              ─ ImageNode (stb_image, stencil clip, border inside stencil scope)
├── style/
│   └── features/
│       └── base.h                 ─ StyleBase with marginAuto[4] flags
├── dev/
│   ├── main.cpp                   ─ Dev binary entry point: socket server → IPC → GLFW → render loop
│   ├── dev_socket.h / .cpp        ─ Unix socket server (listen, accept, read messages)
│   ├── ir_deserializer.h          ─ JSON → MorphNode tree deserializer with style inheritance
│   ├── json_parser.h              ─ Minimal JSON parser (header-only)
│   └── inspector.h                ─ DevTools overlay + panel (F12 toggle, element inspect)
└── vendor/
    ├── glad/glad.h / glad.c       ─ GLAD OpenGL 3.3 loader
    └── stb_image.c                ─ stb_image (PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC)
```

## Dev Mode Flow

```
File change → watcher → pipeline.run() → IR dict (cleaned via _clean_inf)
→ IPCClient.send_ir() → Unix socket → morph_devrt readMessage()
→ JsonValue::parse() → parseIR() → MorphNode tree swap → glfwPollEvents() → render
```

The dev binary (`morph_devrt`) is a CMake project auto-built on missing binary or source hash change. The hash covers `runtime/dev/`, `runtime/core/`, `runtime/render/`, `runtime/widgets/`, and `runtime/style/`.

## Key Runtime Data Flow

### Flush Order
1. `m_batch` (quad fills — background colors, rounded rects)
2. Text (glyph atlas quads)
3. `m_imageBatches` (per-texture-ID batched images)
4. `m_borderBatch` (border rings drawn on top of everything)

### Border-radius Clipping
- Stencil-based: `GL_INCR` for nesting (0→1→2→...), child clips properly intersect ancestor masks
- `endRoundedClip` restores parent stencil function
- Borders for images are queued inside the stencil scope
- Radius clamped to `[0.001, 100.0]` in SDF shader

### Margin Auto (Runtime)
- Build-time: `margin_auto[4]` flags serialized, auto margins stored as `-1.0f` sentinel
- Runtime: `MorphNode::layout()` detects sentinel + flags → re-computes centering each frame
