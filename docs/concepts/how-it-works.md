# How It Works

Morph is a **compiler**, not an interpreter. Your source files never ship — only the compiled binary does.

## The Pipeline

```
src/App.mx  ──►  Oxc Parser  ──►  lightningcss  ──►  IRBuilder  ──►  LayoutEngine  ──►  IRSerializer
                                                                                   │
                                                           ┌────────────────────────┴──────────────┐
                                                           ▼                                       ▼
                                                  [Dev: IPC Socket]                          [Build: C++ Codegen]
                                                  morph_devrt binary                   node_emitter → g++ → binary
                                                  + logic.so (dlopen)                    TS→C++ (logic) → g++ → logic
```

### Step by Step

1. **Oxc Parser** — Parses your `.mx` / `.tsx` / `.ts` file using the TypeScript grammar into an AST (3× faster than SWC, arena-allocated)
2. **lightningcss** — Parses CSS files and resolves Tailwind classes into property dictionaries (browser-grade, parallel)
3. **IRBuilder** — Merges the walked JSX with CSS rules and Tailwind classes into an Intermediate Representation (IR) — a list of windows containing a tree of styled nodes
4. **LayoutEngine** — Computes positions and sizes using box model math (margin, padding, flex, inline)
5. **IRSerializer** — Converts the IR to a JSON-safe dictionary

From here, the pipeline splits:

- **Dev mode** — Sends the IR dict over a Unix socket to the pre-compiled `morph_devrt` renderer. Your JS logic is compiled to a `logic.so` shared library loaded via `dlopen`.
- **Build mode** — Feeds the IR into Tera C++ code generation, producing `app.cpp` which is compiled with g++/clang++ into a standalone binary.

## What Gets Compiled

**Rust (morph) handles the toolchain:**
- `.mx`/`.tsx`/`.ts` parsing (Oxc)
- CSS parsing (lightningcss)
- IR building
- Layout math
- CSS cascade and Tailwind resolution
- C++ code generation (Tera templates)
- TypeScript → C++ translation (morph-js crate)

**C++ handles the runtime:**
- OpenGL rendering (Flash / Forge backends)
- Window management (GLFW)
- Event handling
- Reactivity (signals, effects)
- Coroutines and networking (fetch, timers)

The final binary contains **zero Rust** (compiler only), **zero Python**, and **zero Node.js**.

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

## Two Codegen Modes (JS/TS → C++)

| Mode | Command | Behavior |
|---|---|---|
| **Legacy** | `morph file.ts` | `auto` inference, `Js*` types everywhere, no escape analysis |
| **Optimized** | `morph file.ts --optimize` | Intent-based: escape analysis → stack/`unique_ptr`/`shared_ptr`, native types (`int32_t`, `std::string`, `std::vector`), type widening only when needed |

See [Intent-Based Codegen](../guides/intent-based-codegen.md) for the full memory management strategy.

## Dev Mode Detail

```
┌──────────────┐      Unix Socket       ┌──────────────────┐
│   morph     │  ◄──────────────────►  │   morph_devrt    │
│  (watcher)   │      JSON IR +         │  (always running)│
│              │      logic.so path     │                  │
└──────┬───────┘                        └────────┬─────────┘
       │                                         │
       ▼                                         ▼
┌──────────────┐                        ┌──────────────────┐
│  Rebuild     │                        │  Hot Reload      │
│  logic.so    │                        │  - dlopen new    │
│  (g++ -shared)                        │    logic.so      │
└──────────────┘                        │  - Rewire signals│
                                        │  - Re-run effects│
                                        └──────────────────┘
```

- `morph_devrt` is a pre-compiled renderer binary (shipped with runtime)
- Only the **logic layer** recompiles on edit (~300-500 ms)
- Window, GL context, layout tree **stay alive** — zero flicker

## Build Mode Detail

```
.mx source
    │
    ▼
┌──────────────────────────────────────┐
│  Feature Detection (FeatureSet)      │
│  - Scans IR nodes for used features  │
│  - Emits only required #defines      │
└──────────────┬───────────────────────┘
               │
               ▼
┌──────────────────────────────────────┐
│  Tera Templates (app_main.cpp.tera)  │
│  - window_code + keyframe_code       │
│  - list_factory_code                 │
│  - headers + defines                 │
│  - premain_functions                 │
│  - state_decls                       │
└──────────────┬───────────────────────┘
               │
               ▼
┌──────────────────────────────────────┐
│  g++ -std=c++20 -O2                  │
│  -ffunction-sections                 │
│  -fdata-sections                     │
│  -Wl,--gc-sections                   │
└──────────────┬───────────────────────┘
               │
               ▼
        native binary (~200-800 KB)
```

**Dead code elimination**: Linker GC sections removes unused runtime code. A hello-world with only `<div>` + `<button>` includes ~15 KB of runtime; adding `<img>` pulls in stb_image (~50 KB).

## Version & Cache System

```
~/.morph/
├── cache/
│   └── runtimes/
│       └── cpp/
│           ├── v0.1.0/        # Runtime source (symlinked to .morph/runtime/)
│           ├── v0.2.0/
│           └── v0.3.0/
└── index.json                 # Metadata

Project:
.morph/
├── runtime/        → ~/.morph/cache/runtimes/cpp/v0.2.0/ (symlink or copy)
├── build/          # Object files, fingerprints, logic.so
└── cache/
    ├── css/        # Remote CSS + @font-face files (MD5 keyed)
    └── *.fingerprint  # Content hashes for incremental builds
```

- **Never re-download** same runtime version
- **Fingerprint** = SHA256(`morph.config.json` + entry source + runtime tree + imported files)
- **morph.lock** pins exact version + hash for reproducibility

## Distribution

| Artifact | How It's Built |
|---|---|
| `morph` binary | `cargo install --locked` → static musl on Linux, native on macOS/Windows |
| C++ runtime | GitHub Actions: `g++-14` build → tar.gz → GitHub Release (tagged by `versions/runtime/cpp.json`) |
| Version files | `versions/{morph,version.json}` + `versions/runtime/cpp.json` — push to `main` = auto-release |

Security: `CODEOWNERS` protects `versions/**`, semver validation in CI, sha256 verified on download, only `main` branch triggers releases.