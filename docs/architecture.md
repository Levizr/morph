# Morph — Architecture

## Pipeline

```
.mx source ──► MorphParser ──► JSXWalker ──► IRBuilder ──► LayoutEngine ──► Codegen ──► C++ ──► Binary
```

| Step | Module | Input | Output | Status |
|---|---|---|---|---|
| 1 | `parser/morph_parser.py` | `.mx` source text | tree-sitter AST | ✅ |
| 2 | `parser/jsx_walker.py` | AST | `{imports, components}` dict | ✅ |
| 3 | `style/css_parser.py` | CSS files / URLs | `{selector: {prop: val}}` dict | ✅ |
| 4 | `style/tailwind.py` | class strings | resolved CSS dicts | ✅ |
| 5 | `ir/builder.py` | walked + CSS + Tailwind | `list[IRWindow]` | ❌ stub |
| 6 | `layout/engine.py` | `list[IRWindow]` | computed positions | ⚠️ basic |
| 7 | `codegen/` | `list[IRWindow]` | C++ source files | ⚠️ partial |
| 8 | (missing) | C++ source | native binary | ❌ not built |

## Module Boundaries

- `parser/` — tree-sitter AST building + JSX walking (no hand-written lexer)
- `dom/` — DOM node dataclasses + tree operations
- `style/` — CSS parsing, Tailwind resolution, cascade + specificity (stub)
- `ir/` — language-agnostic node tree + serializer
- `layout/` — box model math (basic vertical stacking; flexbox stub)
- `js/` — AST walking + intent extraction (partial; interpreter stub)
- `codegen/` — IR → C++ via Jinja2 templates (templates done, emitter stub)
- `dev/` — IPC, file watching, hot reload pipeline
- `build/` — compiler invocation (module does not exist yet)
- `pkg/` — package registry client + installer
- `cli/` — terminal I/O, no business logic
- `utils/` — pure helpers, no morph imports

## Dev Mode Flow

```
File save → watcher → pipeline.run() → IR dict → Unix socket → morph_devrt (binary TBD)
```
