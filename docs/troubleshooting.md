# Troubleshooting Guide

## Common Issues

### `morph build` — Compilation errors

**Cause**: Missing C++ dependencies (FreeType, GLFW) or incorrect paths.

**Fix**: Run `morph doctor` to verify dependencies. The compiler expects FreeType headers at `/usr/include/freetype2` and GLFW/OpenGL libs via pkg-config.

### `morph build` — Pipeline fails silently

**Cause**: `IRBuilder.build()` returns `[]`. The pipeline produces empty IR, layout does nothing, serialization returns empty data.

**Fix**: Check the `.mx` file for syntax errors. Ensure there's a single exported component with a `<morph-window>` root.

### `morph dev` — "morph_devrt: command not found"

**Cause**: The dev mode runtime binary (`morph/bin/morph_devrt`) does not exist in the repository.

**Status**: Not yet built. Dev mode is blocked until the C++ dev runtime is compiled. Use `morph build` to compile a standalone binary instead.

### ImportError: `morph.lexer`

**Cause**: Old test files (`test_html_lexer.py`, `test_css_lexer.py`) import from `morph.lexer` which was removed when the project switched to tree-sitter-based parsing.

**Fix**: Delete or rewrite these tests for the current architecture.

### `morph doctor` — Missing dependencies

Run `morph doctor` to check for:

| Dependency | Linux Check | Install |
|---|---|---|
| Python 3.10+ | `python3 --version` | `apt install python3` |
| g++ 11+ | `g++ --version` | `apt install g++` |
| GLFW | `pkg-config --libs glfw3` | `apt install libglfw3-dev` |
| OpenGL | `glxinfo` or `pkg-config --libs gl` | `apt install libgl1-mesa-dev` |
| Node.js (optional) | `node --version` | `apt install nodejs` |
| npm (optional) | `npm --version` | `apt install npm` |

### `MorphParseError` on valid-looking `.mx` code

**Cause**: The tree-sitter JavaScript grammar has specific syntax requirements. Common issues:

1. **Missing quotes on attribute values**: `<div class=container>` → `<div class="container">`
2. **Self-closing tags without `/`**: `<br>` → `<br />`
3. **JSX expressions in single braces**: `{variable}` is fine; `{{style}}` is an object literal
4. **Unescaped HTML entities**: Use template literals or JSX expressions

**Fix**: Check the `.mx` file against standard JSX syntax rules.

### CSS imports not resolved

**Cause**: The `CSSFetcher` downloads remote CSS files but caches them in `.morph/css-cache/`. If the cache is stale:

```bash
morph cache clear
```

Or delete `.morph/css-cache/` manually.

### Tailwind classes not applying

**Cause**: The static Tailwind map only covers 500 common classes. If your class isn't in `STATIC_MAP` in `morph/style/tailwind.py`, it's silently skipped.

**Fix**: Add the missing class to `STATIC_MAP`, or use arbitrary value syntax like `bg-[#ff0000]` which is supported.

### No output from `morph dev` / blank window

**Cause**: Multiple possible causes:

1. `IRBuilder.build()` returns `[]` → no nodes to render
2. `morph_devrt` binary doesn't exist → no renderer to receive IR
3. Unix socket connection fails → IR never reaches renderer
4. CSS/Tailwind resolution fails → styles are empty

**Debug**: Check terminal output for `[morph] ERRROR:` messages. Add temporary print/log statements in `morph/dev/pipeline.py` to inspect the IR before serialization.

## Getting Help

- Open a GitHub issue
- Check the [development guide](docs/development.md) for codebase tour
- Check the [pipeline deep-dive](docs/pipeline.md) for component details
