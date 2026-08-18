# Dynamic

Runtime-driven styles using state-controlled className, inline styles, and C++ interop.

## Files

| File | Description |
|---|---|
| `src/App.mx` | JSX template with dynamic classes and styles |
| `src/native.cpp` | C++ helper function |
| `src/style.css` | Animation and theme styles |
| `morph.config.json` | Window config (500×400) |

## Features Demonstrated

- `morphState` controlling `theme`, `width`, `accent`, `message`, `sig`
- Template-literal className driven by state
- Inline `style` objects bound to state
- `import { getTheme } from './native.cpp'` interop
- `fetch()` async in a coroutine with loading state
- CSS `@keyframes` for animations
- Theme toggle switching className and style props reactively

## Run

```bash
morph run
```

See the [full README](../../examples/dynamic/README.md) for more details.
