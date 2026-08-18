# Troubleshooting Guide

## Common Issues

### `morph build` — Compilation errors

**Cause**: Missing C++ dependencies (FreeType, GLFW) or incorrect paths.

**Fix**: Run `morph doctor` to verify dependencies. The compiler expects FreeType headers at `/usr/include/freetype2` and GLFW/OpenGL libs via pkg-config.

### `morph build` — Pipeline fails silently

**Cause**: `IRBuilder.build()` returns `[]`. The pipeline produces empty IR, layout does nothing, serialization returns empty data.

**Fix**: Check the `.mx` file for syntax errors. Ensure there's a single exported component with a `<morph-window>` root.

### `morph dev` — "morph_devrt: command not found"

**Cause**: The dev mode runtime binary (`morph_devrt`) has not been compiled yet.

**Fix**: `morph dev` auto-builds the binary via CMake on first run (and rebuilds it whenever shared runtime sources change, tracked via a source hash). It needs `cmake`, `g++`, and `make` on your PATH — run `morph doctor` to verify.

### `morph translate` — unexpected TS output

**Cause**: The translator (`morph/js/`) targets the TypeScript grammar via tree-sitter. Mixing plain-JS constructs the TS grammar parses differently (e.g. implicit coercion patterns) can produce surprising C++.

**Fix**: Check the emitted `.cpp` (translate writes it next to the source). The supported surface is documented in the README's JS/TS Runtime section.

### `morph doctor` — Missing dependencies

Run `morph doctor` to check for:

| Dependency | Linux Install | Notes |
|---|---|---|
| Python 3.10+ | `apt install python3` | toolchain |
| g++ 11+ (C++23) | `apt install g++` | toolchain |
| cmake / make / pkg-config | `apt install cmake make pkg-config` | dev binary build |
| GLFW | `apt install libglfw3-dev` | graphics |
| OpenGL / X11 | `apt install libgl1-mesa-dev libx11-dev` | graphics |
| FreeType | `apt install libfreetype-dev` | text |
| HarfBuzz | `apt install libharfbuzz-dev` | text |

`morph doctor` knows the package name per manager (apt/dnf/pacman/zypper/apk/brew/winget/choco) and can auto-install missing packages with `-y`. Use `-v` for detailed version output.

### `MorphParseError` on valid-looking `.mx` code

**Cause**: The tree-sitter TypeScript grammar has specific syntax requirements. Common issues:

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
5. `JSON parse error: Unexpected char: I` → `float('inf')` not cleaned from IR dict before JSON serialization (fixed in `pipeline.py` via `_clean_inf()`)

**Debug**: Check terminal output for `[morph] ERROR:` messages. Add temporary print/log statements in `morph/dev/pipeline.py` to inspect the IR before serialization.

### Dev window closes immediately

**Cause**: The dev binary (`morph_devrt`) crashes or exits immediately. Possible causes:

1. **GLFW not initialized**: `glfwInit()` fails or `glfwCreateWindow` returns null — check `[GLFW] error` messages in terminal output
2. **Wayland vs X11**: If on Wayland with the X11-only `libglfw3`, window creation fails. Try: `GDK_BACKEND=x11 morph dev`
3. **JSON parse failure**: If the IR dict contains `Infinity` (from `float('inf')`), the C++ JSON parser rejects it. Ensure `_clean_inf()` is applied
4. **`morph_devrt` exited with code -11**: SIGSEGV — usually from `~MorphWindow` destructor calling GLFW functions after `glfwTerminate()`. Fixed by scoping window destruction before `glfwTerminate()`

**Fix**: Run `morph dev` with binary stderr visible (default after v0.0.5). Check for `[GLFW] error` or `[devrt]` messages. Ensure `libglfw3` is installed and try with `GDK_BACKEND=x11` on Wayland.

## Getting Help

- Open a GitHub issue
- Check the [development guide](docs/development.md) for codebase tour
- Check the [pipeline deep-dive](docs/pipeline.md) for component details
