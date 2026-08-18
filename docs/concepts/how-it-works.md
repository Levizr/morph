# How It Works

Morph is a **compiler**, not an interpreter. Your source files never ship — only the compiled binary does.

## The Pipeline

```
src/App.mx  ──►  MorphParser  ──►  JSXWalker  ──►  IRBuilder  ──►  LayoutEngine  ──►  IRSerializer
                                                                                    │
                                                    ┌───────────────────────────────┴──────────────┐
                                                    ▼                                               ▼
                                           [Dev: IPC Socket]                              [Build: C++ Codegen]
                                           morph_devrt binary                     node_emitter → g++ → binary
                                           + logic.so (dlopen)                    TS→C++ (logic) → g++ → logic
```

### Step by step

1. **MorphParser** — tree-sitter parses your `.mx` file using the TypeScript grammar into an AST
2. **JSXWalker** — walks the AST and extracts imports, components, props, and JSX structure
3. **CSS Parser** — parses CSS files and resolves Tailwind classes into property dictionaries
4. **IRBuilder** — merges the walked JSX with CSS rules and Tailwind classes into an Intermediate Representation (IR) — a list of windows containing a tree of styled nodes
5. **LayoutEngine** — computes positions and sizes using box model math (margin, padding, flex, inline)
6. **IRSerializer** — converts the IR to a JSON-safe dictionary

From here, the pipeline splits:

- **Dev mode** — sends the IR dict over a Unix socket to the pre-compiled `morph_devrt` renderer. Your JS logic is compiled to a `logic.so` shared library loaded via `dlopen`.
- **Build mode** — feeds the IR into Jinja2 C++ code generation, producing `app.cpp` which is compiled with g++ into a standalone binary.

## What Gets Compiled

Python handles the toolchain:
- `.mx` parsing (tree-sitter)
- IR building
- Layout math
- CSS cascade and Tailwind resolution
- C++ code generation (Jinja2 templates)
- TypeScript → C++ translation

C++ handles the runtime:
- OpenGL rendering
- Window management
- Event handling
- Reactivity (signals, effects)
- Coroutines and networking

The final binary contains **zero Python** and **zero Node.js**.

## The .mx Format

An `.mx` file is a single file containing JSX markup with TypeScript/JavaScript logic and CSS imports:

```tsx
import { CSS, morphState } from 'morph'
import { compute } from './math.cpp'    // C++ import

CSS.load("./style.css")

export const windowConfig = { title: "App", width: 800, height: 600 }

export default function App() {
  const [value, setValue] = morphState(0)
  return (
    <body>
      <div>Result: {compute(value)}</div>
      <button onClick={() => setValue(value + 1)}>Add</button>
    </body>
  )
}
```

## Feature-Based Compilation

Morph scans the IR tree and detects which features your app actually uses. Only the required C++ code is compiled in — everything else is stripped by the linker:

- `text` — text rendering (adds FreeType)
- `button` — button widget
- `scroll` — scroll containers
- `flex` — flexbox layout
- `hover` — hover pseudo-class
- `animation` — CSS animations
- `transform` — CSS transforms
- `image` — image rendering (adds stb_image)
- and more...

This is why Morph binaries are so small — a simple "Hello World" app doesn't include image loading, animation, or scroll code.
