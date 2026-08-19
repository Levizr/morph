# Rust Support — Full Rust Runtime & Interop

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Morph compiles apps to **either** a C++ runtime **or** a completely Rust-native runtime, chosen at project creation:

```
morph init my-app --lang rust     # Rust runtime (100% Rust, no C++ runtime code)
morph init my-app --lang c++      # C++ runtime (current default)
```

When `--lang rust` is chosen, **the C++ runtime is never used** — a parallel runtime written entirely in Rust replaces it: windowing, OpenGL rendering, layout, style, reactivity, coroutines, networking, and the JS value model. The `.mx` files you write are identical — only the backend changes.

## Why it matters

- **Rust guarantees** — memory safety, no segfaults, fearless concurrency for a renderer that already runs a compositor thread
- **Ecosystem access** — users can pull any Rust crate into their UI logic (parsers, crypto, ML, audio)
- **Cross-language calling** — Rust functions callable from JSX, and JSX functions callable from Rust, like the existing C++ interop
- **Appeal** — a large developer population prefers Rust; language choice becomes a selling point, not a limitation

## Choosing the language

```bash
morph init my-app --lang rust     # rust project (rust-toolchain.toml, Cargo-based build)
morph init my-app --lang c++      # c++ project (g++/CMake-based build, as today)
```

- Default stays `c++` — existing projects and the whole runtime remain untouched
- `morph doctor` validates the matching toolchain: `cargo` + `rustc` (edition 2021+) for rust, `g++` for c++
- The language is set at init; mixing runtimes inside one app is out of scope (the *code you write* can still mix — see Interop)

## How it will work

### The pipeline (only the backend changes)

```
src/App.mx ──► MorphParser ──► JSXWalker ──► IRBuilder ──► IRSerializer
                                                              │
                                     ┌────────────────────────┴───────────────┐
                                     ▼                                        ▼
                             [Dev: logic.rs → cdylib]              [Build: Rust codegen]
                             dlopen hot reload                    Jinja2 .rs templates
                                                                  cargo build → binary
```

- Parsing, JSX walking, IR building, CSS/Tailwind resolution, and layout math stay in Python — the exact same IR the C++ pipeline uses
- The **codegen templates** (`node_emitter.py`) get a Rust sibling that emits `.rs` instead of `.cpp`
- Dev mode compiles the app's JS logic to a `logic-<hash>.so` **Rust cdylib** loaded via `dlopen` — hot reload works exactly like today's `logic.so` (the `dlopen`/`RTLD_NOLOAD` machinery is language-agnostic)
- Production compiles one self-contained binary via `cargo build --release` (static GLFW/FreeType/HarfBuzz story mirrors `morph build --static`)

### The Rust runtime (mirrors the C++ runtime 1:1)

| C++ runtime | Rust runtime |
|---|---|
| `MorphNode` tree + layout + style | `MorphNode` struct tree + layout + style modules |
| `GLRenderer` (batched quads, SDF shaders) | OpenGL via `glow` (or raw FFI) — same shaders, same pipeline |
| `Compositor` thread + lock-free `RenderFrame` | `Compositor` on its own thread + crossbeam/atomics frame swap |
| `Signal<T>` + effects | `Signal<T>` with the same auto-subscription semantics |
| `morph::Task` coroutines + `next_frame` | Rust `async` (a small hand-rolled executor — no heavyweight runtime) |
| `morph::net::fetch` on a worker thread | HTTP on a worker thread (std `TcpStream` or `reqwest`) |
| `JsValue` variant + `JsNumber`/`JsString`/`JsArray`/`JsObject` | A `JsValue` enum with the same JS semantics (truthiness, `==`, coercion) |
| FreeType + HarfBuzz text | `freetype` / `harfbuzz-rs` crates (or FFI to the same system libs) |
| OpenGL 3.3 renderer | `glow` today — or `wgpu`, which wraps Vulkan/Metal/D3D12/GL in one crate (see [Graphics APIs](graphics-api.md)) |
| Feature gates + linker GC | Rust's dead-code elimination (already excellent) |

Design goal: **pixel-identical output** between the C++ and Rust runtimes — same layout math, same shaders, same event semantics — so apps behave identically regardless of `--lang`.

### Cross-language interop (both directions)

**Rust functions from JSX** — the Rust mirror of the existing C++ import:

```rust
// src/math.rs
pub fn compute(a: f64, b: f64) -> f64 { a * b }
```

```tsx
// src/App.mx
import { compute } from './math.rs'   // exactly like './file.cpp' today

export default function App() {
  const [result, setResult] = morphState(0)
  return (
    <div>
      <h1>{compute(6, 7)}</h1>   {/* → 42 */}
    </div>
  )
}
```

**JSX functions from Rust** — the codegen emits a generated bridge module (the Rust sibling of `_morph_state.h`):

```rust
// generated: _morph_state.rs (exposes signals + JSX functions to Rust)
use morph::prelude::*;

pub fn use_state() -> Signal<f64> { /* the app's signals */ }
```

```rust
// src/controller.rs — your Rust logic drives the UI
pub fn increment_counter(state: &Signal<f64>) {
    state.set(state.get() + 1.0);   // JSX re-renders, like morphState
}
```

- Rust imports are `#included`/`mod`-ed into the generated translation unit — same mechanism as today's `import { fn } from './file.cpp'`
- A `native` config block for rust: `[dependencies]`-style entries, `extern crate` paths, `cflags`/`ldflags` equivalents → forwarded to cargo

### TS → Rust translation

The `TSToCppTranslator` gets a sibling translator. Type mapping:

| TS | Rust |
|---|---|
| `string` | `String` |
| `number` / `int` / `double` | `f64` / `i64` / `f64` |
| `boolean` | `bool` |
| `any` | `JsValue` (the Rust enum) |
| `Array<T>` | `Vec<JsValue>` |
| `object` / `Record` | `JsObject` (`HashMap<String, JsValue>`) |
| `Promise<T>` | `async` / a `Result`-like future |
| `MouseEvent` / `Element` | `&MorphEvent` / `&MorphNode` |

`morph translate file.ts --lang rust` emits `.rs`. The existing `morph check` diagnostics apply unchanged — they audit the JS surface, not the backend.

> **Related:** the toolchain that compiles all of this is itself moving to Rust (SWC/Oxc parsing, native CLI, Python removed) — see [Rust Compiler & Native CLI](compiler.md).

## What stays identical for users

- `.mx` files, JSX, CSS, Tailwind, `morphState`/`morphEffect`, `fetch()`, timers
- The shipped `node_modules/morph` `.d.ts` — autocomplete is backend-agnostic
- Dev mode UX: hot reload, DevTools (Elements/Rendering/Network/Logs)
- `morph dev`, `morph build`, `morph run --static`, `morph check`

## Current state

| Building block | State |
|---|---|
| C++ runtime + interop pattern (`import from './file.cpp'`, `_morph_state.h`) | ✅ Shipped — the blueprint to mirror |
| IR pipeline shared by both backends | ✅ Shipped |
| `dlopen` hot-reload machinery (language-agnostic) | ✅ Shipped |
| Jinja2 codegen templates (C++ flavor) | ✅ Shipped |
| Rust runtime | ❌ Not started |
| Rust codegen templates + TS→Rust translator | ❌ Not started |
| `--lang rust` in `morph init` + `morph doctor` cargo checks | ❌ Not started |

## Suggested phases

1. **Foundation** — `--lang rust` flag, project template (Cargo.toml, rust-toolchain.toml), window + GLFW + GL context via `glow`
2. **Rendering core** — node tree, layout, style, batched SDF renderer; smoke-test the existing `examples/calculator` pixel-identical to C++
3. **JS runtime** — `JsValue` enum, `Signal<T>` reactivity, effects, timers
4. **TS→Rust translator** — `morph translate --lang rust`, JSX codegen in Rust
5. **Interop bridge** — `import from './math.rs'` + generated `_morph_state.rs` (bidirectional)
6. **Dev mode** — `logic.rs` cdylib + hot reload via dlopen
7. **Production** — static linking, `morph build --static` parity, binary-size targets
8. **Parity & hardening** — pixel-identical test matrix across both runtimes; CI jobs for both

## Open questions

- **Dependency policy** — zero-dep ethos vs convenience crates (`glow`, `reqwest`); the C++ runtime vendors everything, Rust should probably do the same (vendored crates or FFI to system libs)
- **Async runtime** — hand-rolled executor vs `tokio`; Morph's coroutine scheduler is deliberately tiny
- **Renderer parity** — the Flash/Forge renderer split exists in C++; does the Rust runtime start with Flash only and add Forge later?
- **`morph check`** — same JS audit, plus new rust-specific diagnostics (unsupported crate patterns?)
- **Binary size** — Rust's std is bigger than C++; does `--static` stay <1 MB or does the target move?

## Build steps (when picked up)

1. `morph init --lang rust` + `morph doctor` cargo checks + rust project template
2. Minimal window + render loop in Rust, consuming the existing JSON IR (dev path)
3. JsValue/Signal/reactivity + TS→Rust translator (calculator example compiles)
4. Bidirectional interop bridge (`math.rs` import + `_morph_state.rs`)
5. Dev hot reload via rust cdylib; production `cargo build --release` parity with C++