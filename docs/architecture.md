# Morph — Architecture (v0.0.2)

## Pipeline

```
.mx source ──► MorphParser ──► JSXWalker ──► IRBuilder ──► LayoutEngine ──► Codegen ──► g++ ──► Binary
                                                                                         │
                                                                                  [Dev: Unix Socket]
```

| Step | Module | Input | Output | Status |
|---|---|---|---|---|
| 1 | `parser/morph_parser.py` | `.mx` source text | tree-sitter AST | ✅ |
| 2 | `parser/jsx_walker.py` | AST | `{imports, components}` dict | ✅ |
| 3 | `style/css_parser.py` | CSS files / URLs | `{selector: {prop: val}}` dict | ✅ |
| 4 | `style/tailwind.py` | class strings | resolved CSS dicts | ✅ |
| 5 | `ir/builder.py` | walked + CSS + Tailwind | `list[IRWindow]` | ✅ |
| 6 | `layout/engine.py` | `list[IRWindow]` | computed positions | ✅ |
| 7 | `codegen/node_emitter.py` | `list[IRWindow]` | C++ source files | ✅ |
| 8 | `build/compiler.py` | C++ source | native binary | ✅ |

## Module Boundaries

- `parser/` — tree-sitter AST building + JSX walking (no hand-written lexer)
- `style/` — CSS parsing, Tailwind resolution, cascade + specificity (stub)
- `ir/` — language-agnostic node tree + builder + serializer
- `layout/` — box model math (basic vertical stacking; flexbox stub)
- `codegen/` — IR → C++ via Jinja2 templates + node emitter
- `build/` — compiler invocation (g++ with FreeType/GLFW/OpenGL flags)
- `dev/` — IPC, file watching, hot reload pipeline
- `pkg/` — package registry client + installer
- `cli/` — terminal I/O, no business logic
- `utils/` — pure helpers, no morph imports

## C++ Runtime

```
runtime/
├── core/
│   ├── morph_node.h          ─ MorphNode base + MorphStyle
│   ├── morph_window.h        ─ GLFW window, ortho proj, render loop
│   ├── renderer.h            ─ Renderer interface
│   ├── gl_renderer.h         ─ OpenGL 3.3 batch renderer (VAO/VBO/IBO)
│   ├── window_manager.h      ─ Multi-window management
│   └── event.h               ─ Event system
├── widgets/
│   ├── morph_rect.h          ─ RectNode (div, span, etc.)
│   ├── morph_text.h          ─ TextNode
│   ├── morph_radius.h        ─ RoundedRectNode
│   └── morph_button.h        ─ ButtonNode with onClick
├── vendor/
│   └── glad/                 ─ GLAD OpenGL loader
└── dev/                      ─ dev mode IPC (to be built)
```

## Dev Mode Flow

```
File save → watcher → pipeline.run() → IR dict → Unix socket → morph_devrt (binary TBD)
```
