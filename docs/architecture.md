# Morph — Architecture

## Pipeline

```
HTML/CSS/JS → Lexer → Parser → DOM + StyleSheet → IRBuilder → LayoutEngine → Codegen → C++ → Binary
```

## Module Boundaries

- `lexer/`   — raw text → token stream
- `parser/`  — tokens → typed tree/sheet
- `dom/`     — HTML tree operations
- `style/`   — CSS cascade + resolution
- `ir/`      — language-agnostic node tree
- `layout/`  — box model math only
- `js/`      — AST walking + intent extraction
- `codegen/` — IR → C++ via Jinja2 templates
- `dev/`     — IPC, file watching, hot reload
- `build/`   — compiler invocation
- `pkg/`     — package registry client
- `cli/`     — terminal I/O, no business logic
- `utils/`   — pure helpers, no morph imports
