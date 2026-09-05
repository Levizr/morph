# Morph

Build native desktop applications with HTML, CSS, and JavaScript. No browser, no Electron, no WebView — just a lightweight native binary.

```
morph new my-app
cd my-app
morph dev
```

Write familiar `.mx` files (JSX + CSS + TypeScript). Morph compiles them directly to native OpenGL binaries with zero runtime overhead.

```tsx
// src/App.mx
import { CSS, morphState } from 'morph'

CSS.load("./style.css")

export const windowConfig = { title: "My App", width: 800, height: 600 }

export default function App() {
  const [count, setCount] = morphState(0)
  return (
    <body>
      <div className="app">
        <h1 style="color: #e0e0e0;">Hello from Morph</h1>
        <button className="btn" onClick={() => setCount(count + 1)}>
          Clicked {count} times
        </button>
      </div>
    </body>
  )
}
```

```bash
morph dev      # live window, hot reload
morph run      # optimized native binary
```

## Why Morph?

| | Electron | Qt | Morph |
|---|---|---|---|
| Write UI in | HTML/CSS/JS | C++ / QML | TS/JSX/CSS |
| Runtime | Chromium (~150MB) | Qt libs | **Zero** |
| Binary size | ~80MB+ | ~20MB+ | **<1MB** |
| Native OpenGL | ✗ | ✓ | ✓ |
| Hot reload | ✓ | ✗ | ✓ |

## Key Features

- **`.mx` files** — JSX-like syntax with TypeScript/JavaScript and CSS in a single file
- **CSS** — Flexbox, transitions, animations, transforms, Tailwind utilities
- **Reactive state** — `morphState` and `morphEffect` for component reactivity
- **Async networking** — `fetch()` with coroutines, runs on worker threads
- **C++ interop** — Import user `.cpp` files directly into your JSX
- **DevTools** — Built-in element inspector, rendering profiler, network log
- **Tiny binaries** — Feature-based dead code elimination, optional UPX compression
- **Intent-based codegen** — `--optimize` flag uses escape analysis for native types, zero GC

## Next Steps

- [Installation](getting-started/installation.md) — Set up your system
- [Quick Start](getting-started/quick-start.md) — Build your first app in 2 minutes
- [Configuration](getting-started/configuration.md) — All `morph.config.json` options
- [Architecture](concepts/architecture.md) — How the compiler and runtime work
- [Intent-Based Codegen](guides/intent-based-codegen.md) — Memory management without GC
- [Deployment](guides/deployment.md) — Ship AppImage, DMG, MSI, and sign releases
- [Debugging](guides/debugging.md) — GDB, DevTools, profiling, sanitizers