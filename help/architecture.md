# Morph — Architecture

## Pipeline

```
.mx source ──► MorphParser ──► JSXWalker ──► IRBuilder ──► LayoutEngine ──► IRSerializer
                                                                              │
                                              ┌───────────────────────────────┴──────────────┐
                                              ▼                                               ▼
                                     [Dev: IPC Socket]                              [Build: C++ Codegen]
                                     morph_devrt binary                     node_emitter → g++ → binary
                                     + logic.so (dlopen)                    TS→C++ (logic) → g++ → logic
```

| Step | Module | Input | Output | Status |
|---|---|---|---|---|
| 1 | `parser/morph_parser.py` | `.mx` source text | tree-sitter AST (TS grammar) | ✅ |
| 2 | `parser/jsx_walker.py` | AST | `{imports, components}` dict | ✅ |
| 3 | `style/css_parser.py` | CSS files / URLs | `{selector: {prop: val}}` dict | ✅ |
| 4 | `style/tailwind.py` | class strings | resolved CSS dicts | ✅ |
| 5 | `style/selector.py` | selectors | combinators + specificity scores | ✅ |
| 6 | `ir/builder.py` | walked + CSS + Tailwind | `list[IRWindow]` | ✅ |
| 7 | `layout/engine.py` | `list[IRWindow]` | computed positions | ✅ |
| 8 | `ir/serializer.py` | `list[IRWindow]` | JSON-safe dict | ✅ |
| 9a | `codegen/node_emitter.py` (build) | `list[IRWindow]` | C++ source files | ✅ |
| 9b | `codegen/logic_emitter.py` (build) | `.ts`/`.mx` JS | C++ logic (signals/effects) | ✅ |
| 9c | `dev/server.py` (dev) | JSON dict | Unix socket → `morph_devrt` | ✅ |
| 9d | `dev/pipeline.py` (dev) | `.ts`/`.mx` JS | `logic.<hash>.so` via `dlopen` | ✅ |

## Module Boundaries

- `parser/` — tree-sitter AST building + JSX walking (no hand-written lexer)
- `style/` — CSS parsing, Tailwind resolution, selector matching + specificity
- `ir/` — language-agnostic node tree + builder + serializer
- `layout/` — box model math (vertical stacking, flexbox, inline measure, auto margins, percentage values)
- `js/` — TypeScript → C++ translator (`ast_builder.py` + `codegen.py`)
- `codegen/` — IR → C++ via Jinja2 templates, node emitter, logic emitter, feature detection
- `build/` — compiler invocation (g++ with FreeType/GLFW/OpenGL flags, `--static`)
- `dev/` — IPC (Unix socket), file watcher, hot reload pipeline, dev binary rebuild, `logic.so` wiring
- `pkg/` — package registry client + installer
- `cli/` — terminal I/O, no business logic
- `utils/` — pure helpers (colors, units, logging), no morph imports

## C++ Runtime

```
runtime/
├── core/
│   ├── node.h + node/           ─ MorphNode base + MorphStyle; node.cpp split into
│   │   │                          node.cpp, layout.cpp, style.cpp, events.cpp,
│   │   │                          flatten.cpp, paint_order.cpp
│   ├── window.h / window.cpp    ─ MorphWindow: GLFW wrapper, ortho proj, compositor
│   │                              integration (startCompositor/commitFrame/renderFrame),
│   │                              docked-DevTools content area (contentWidth())
│   ├── compositor.h / .cpp      ─ Compositor thread that owns the GL context, swaps
│   │                              lock-free RenderFrame snapshots, interpolates at vsync
│   ├── render_frame.h           ─ Lock-free frame snapshot (flat nodes + DrawOps + AnimationStates)
│   ├── draw_op.h                ─ Display-list draw operations (rounded rect, bordered rect, image, text)
│   ├── spsc_queue.h             ─ Single-producer/single-consumer event queue (compositor → main)
│   ├── renderer.h               ─ Renderer interface
│   ├── window_manager.h         ─ Multi-window management
│   └── event.h                  ─ Event system (MouseDown/Up/Move, Click, Scroll)
├── render/
│   └── gl_renderer.h / .cpp     ─ OpenGL 3.3 batch renderer (VAO/VBO/IBO, stencil, border batch)
├── renderers/
│   ├── renderer.h / .cpp        ─ RenderMode dispatch (Flash | Forge), activeRenderMode()
│   ├── flash/                   ─ flash renderer (full-clear direct path)
│   └── forge/                   ─ forge retained renderer: damage.cpp/h (DamageSet),
│                                  layer.h, tile.h, tile_pool.h (content-keyed tile pool)
├── shaders/
│   └── shader.h                 ─ GLSL shaders (SDF rounded rect, text glyph atlas, border ring)
├── ui/
│   ├── rect.h                   ─ RectNode (div, span, h1–h6, p) with overflow/border-radius clipping
│   ├── text.h                   ─ TextNode with FreeType glyph atlas + word-wrap
│   ├── button.h                 ─ ButtonNode with onClick + hover
│   ├── image.h                  ─ ImageNode (stb_image, stencil clip, border)
│   ├── input.h                  ─ InputNode (MORPH_FEATURE_INPUT)
│   ├── radius.h                 ─ RoundedRectNode (legacy, superseded by SDF shader)
│   ├── viewport_node.h          ─ <morph-viewport> node (planned)
│   └── viewport_driver.h        ─ viewport driver interface (declared)
├── types/                       ─ JS runtime types
│   ├── js_value.h               ─ JsValue variant (undefined/null/bool/number/string/array/object/function)
│   ├── js_number.h              ─ JsNumber (int64/double/big variants + arithmetic)
│   ├── js_string.h              ─ JsString (upper/lower/trim/charAt/indexOf/substring/slice/replace/split)
│   ├── js_array.h               ─ JsArray (push/pop/index, shared-ptr storage)
│   ├── js_object.h              ─ JsObject (map-backed, has/keys/index)
│   ├── js_boolean.h             ─ JsBoolean
│   └── js_types.h               ─ shared type aliases
├── reactivity/
│   ├── signal.h                 ─ Signal<T> with thread-local effect auto-subscription
│   ├── effect.cpp               ─ create_effect()/run_pending_effects()/cleanup
│   ├── task.h / task.cpp        ─ morph::Task coroutines, next_frame awaiter, timers
│   └── promise.h                ─ morph::Result<T> promise-like coroutine return type
├── net/
│   └── net.h / net.cpp          ─ fetch() HTTP GET on a worker thread, Response (status/headers/ok/text)
├── style/
│   └── features/                ─ optional style feature structs (hover, zindex, flex, base)
├── dev/
│   ├── main.cpp                 ─ Dev binary entry point: socket server → IPC → GLFW → render loop
│   ├── dev_socket.h / .cpp      ─ Unix socket server (listen, accept, read messages)
│   ├── ir_deserializer.h        ─ JSON → MorphNode tree deserializer with style inheritance
│   ├── json_parser.h            ─ Minimal JSON parser (header-only)
│   ├── inspector.h              ─ DevTools panel (Elements / Rendering / Network / Logs tabs)
│   ├── dev_log.h                ─ ring buffer of log entries (info/ok/warn/error)
│   ├── dev_net.h                ─ ring buffer of fetch() request logs
│   ├── node_registry.h          ─ keeps node references across logic.so reloads
│   └── signal_store.h           ─ keeps signal/state references across logic.so reloads
└── vendor/
    ├── glad/glad.h / glad.c     ─ GLAD OpenGL 3.3 loader
    └── stb_image.c / .h         ─ stb_image (PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC)
```

## Dev Mode Flow

```
File change → watcher (with settle + content-hash skip) → pipeline.run() → IR dict (cleaned via _clean_inf)
→ IPCClient.send_ir() → Unix socket → morph_devrt readMessage()
→ JsonValue::parse() → parseIR() → MorphNode tree swap → glfwPollEvents() → render

JS logic:  .ts/.mx JS → tree-sitter TS AST → TSAstBuilder → TSToCppTranslator
           → logic.<hash>.so (compiled via g++) → dlopen + morph_logic_init/rewire/cleanup
```

The dev binary (`morph_devrt`) is a CMake project auto-built on missing binary or source hash change. The hash covers `runtime/dev/`, `runtime/core/`, `runtime/render/`, `runtime/widgets/`/`runtime/ui/`, `runtime/style/`, and the renderers — so changes to shared runtime files also trigger a rebuild.

## Key Runtime Data Flow

### Threading Model (since v0.0.6)

- **Main thread** — GLFW event pump, style, layout, paint. Flattens the node tree + display-list `DrawOp`s into a lock-free `RenderFrame` snapshot and atomically swaps the pointer the compositor reads.
- **Compositor thread** — owns the GL context exclusively. Waits on `g_framePending`, interpolates compositor-safe animations (X/Y, opacity, bg color, text color, border-radius) at vsync, draws via the active renderer, and pushes animation-completion events back over an SPSC queue that the main thread drains.

### Renderers

- **Flash** — lightweight full-clear direct renderer (the pre-v0.0.6 path). **Default and recommended.**
- **Forge** — retained FBO + `DamageSet` damage tracking. Per-frame damage = box-geometry diff vs the prev-frame rect map ∪ pre-layout paint dirt; only nodes touching damage are re-rastered; the whole surface is presented via `glBlitFramebuffer`; idle frames just blit. **Status: beta/in progress** — known bugs around damage-rect edges, scroll-shift, and some compositor-animation paths; toggleable live in dev for testing.
- Production resolves the renderer at compile time (`constexpr`), dev builds both and hot-switches live from the DevTools Rendering tab (`MORPH_FEATURE_DEV_RENDERER_SWITCH`).

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

### Docked DevTools

- The DevTools panel occupies the right side of the window; the app's layout is constrained to the remaining content area via `MorphWindow::contentWidth()` (window width − `m_devtoolsWidth`, clamped ≥ 120px) so the panel never covers app elements
- The panel is drag-resizable from its left edge (min 240px, app keeps ≥ 360px); F12 toggles open/closed and re-applies the strip width
