# Architecture Overview

Morph is a **native UI framework** where your TypeScript/JSX source compiles directly to a C++ OpenGL binary. No browser, no interpreter, no JIT.

## High-Level Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           MORPHC (Rust Binary)                              │
├─────────────────────────────────────────────────────────────────────────────┤
│  .mx/.tsx/.ts  ──►  Oxc Parser  ──►  lightningcss  ──►  IRBuilder  ──►      │
│       │                                                                │    │
│       │                    ┌───────────────────────────────────────────┘    │
│       ▼                    ▼                                                │
│  ┌─────────┐         ┌─────────┐                                            │
│  │Dev Mode │         │Build    │                                            │
│  └────┬────┘         └────┬────┘                                            │
│       │                   │                                                 │
│       ▼                   ▼                                                 │
│  IPC Socket          CppEmitter                                             │
│  (Unix/TCP)             │                                                   │
│       │                 ▼                                                   │
│       ▼          ┌─────────────┐                                            │
│  morph_devrt     │  app.cpp    │                                            │
│  (prebuilt)      └──────┬──────┘                                            │
│                        │                                                    │
│                        ▼                                                    │
│                 g++/clang++                                                 │
│                        │                                                    │
│                        ▼                                                    │
│                   native binary                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

## Crate Structure (Single Workspace)

```
morph/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── morph/                   # CLI binary (~12 MB)
│   │   ├── src/main.rs           # CLI entry, command dispatch
│   │   └── src/commands/         # new/install/update/dev/build/run/check/doctor/cache
│   ├── morph-config/             # morph.config.json + morph.lock parsing/validation
│   ├── morph-cache/              # Global cache (~/.morph/), runtime download, fingerprints
│   ├── morph-parser/             # Oxc + lightningcss → MxSource (AST + CSS)
│   ├── morph-ir/                 # Intermediate Representation (nodes, style, layout, keyframes)
│   ├── morph-codegen/            # C++ codegen via Tera templates
│   │   ├── src/cpp/              # CppEmitter, node_emitter, logic_emitter, feature_set
│   │   └── src/rust/             # RustEmitter (stub)
│   ├── morph-build/              # Compilation (g++/clang++), platform abstraction, dev IPC
│   └── morph-js/                 # Direct file morph: TS/JS → C++/Rust (Oxc-based)
│       └── src/codegen/          # analyzer, cpp, rust, type_resolver, context
├── runtime/
│   └── cpp/                      # C++ runtime source (shipped as release artifact)
└── versions/                     # Version files = release triggers
    ├── morph/version.json
    └── runtime/cpp.json
```

## Key Design Decisions

### Why Rust for the Compiler?

| Reason | Detail |
|---|---|
| **Speed** | Oxc parser = 3× SWC, arena-allocated, zero-copy |
| **Single binary** | `cargo install morph` → no Python/Node deps |
| **Parallelism** | Rayon-based parallel parsing, CSS resolution, codegen |
| **Type safety** | AST → IR → Codegen with compile-time guarantees |

### Why C++ for the Runtime?

| Reason | Detail |
|---|---|
| **JS semantics map 1:1** | Braces, operators, control flow — no impedance mismatch |
| **Binary size** | <1 MB hello-world (Rust std adds ~500 KB minimum) |
| **OpenGL ecosystem** | GLFW, FreeType, HarfBuzz, stb_image are C libraries |
| **Solo velocity** | Author thinks in C++; C++23 (smart pointers, ranges, modules) is productive |

### Why Oxc + lightningcss?

- **Oxc**: Spec-compliant TS/JSX parser, arena allocator, fast, used by Biome/ESLint
- **lightningcss**: Browser-grade CSS parser, typed property values, Tailwind-friendly, parallel

### Why Tera Templates?

- Jinja2-compatible syntax (easy migration from Python toolchain)
- Fast runtime compilation, sandboxed, no arbitrary code execution
- Powers `node_emitter.rs` + `logic_emitter.rs` for C++ generation

## Dev Mode Architecture

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

## Build Mode Architecture

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

## Runtime Structure

```
runtime/cpp/
├── core/                    # Scene graph + windowing
│   ├── node.h              # MorphNode, DirtyFlag, HoverTransition, flatten()
│   ├── window.h            # GLFW window, event loop, renderer dispatch
│   ├── window_manager.h    # Multi-window (stub)
│   ├── compositor.h        # Compositor thread, SPSC queue, vsync interp
│   ├── render_frame.h      # Lock-free frame data for compositor
│   └── draw_op.h           # DrawOp enum (Rect, Text, Image, Border, Clip, Scissor)
├── render/                 # Shared GL primitives
│   ├── gl_renderer.h       # Batched rects, text, borders, clips, textures
│   └── shader.h            # SDF text, rounded rect, image shaders
├── renderers/              # Paint backends
│   ├── renderer.h          # RenderMode {Flash, Forge}, activeRenderMode()
│   ├── flash/flash.h       # Full clear + replay (default, ~22 MB @1080p)
│   └── forge/              # Retained FBO + DamageSet (beta, ~30 MB floor)
│       ├── forge.h
│       ├── damage.h
│       ├── layer.h
│       ├── tile.h
│       └── tile_pool.h
├── style/                  # CSS → GPU pipeline
│   ├── style.h             # MorphStyle + feature-gated mixins
│   └── features/           # flex, position, scroll, border, cursor, zindex,
│                           # opacity, transform, animation, outline, shadow
├── ui/                     # Concrete widgets
│   ├── rect.h, text.h, button.h, input.h, image.h
│   ├── radius.h, viewport_node.h, viewport_driver.h, morph_list.h
├── widgets/                # Thin wrappers
│   └── morph_rect.h, morph_text.h, morph_button.h, morph_image.h, morph_radius.h
├── net/                    # Networking
│   └── net.h               # Headers, Response, fetch(), HttpAwaitable (coroutine)
├── reactivity/             # Async + signals
│   ├── signal.h            # Signal<T>, create_effect, create_memo
│   ├── promise.h           # morph::Result<T> (Promise<T>), ValueAwaiter
│   └── task.h              # morph::Task (Promise<void>), next_frame, timers
├── types/                  # JS value types
│   ├── js_types.h          # Umbrella + js_value_format.h (MORPH_NO_FORMAT guard)
│   ├── js_value.h          # JsValue variant + JS semantics
│   ├── js_string.h         # JsString + methods
│   ├── js_number.h         # JsNumber (int64/double/bigint) + ops
│   ├── js_boolean.h
│   ├── js_array.h          # shared_ptr<vector<JsValue>>
│   └── js_object.h         # shared_ptr<map<string,JsValue>>
├── dev/                    # DevTools (compiled out in build)
│   └── inspector.h, dev_log.h, dev_net.h, dev_socket.h, ...
└── vendor/                 # Bundled C deps
    ├── glad/glad.h
    └── stb_image.h
```

## JS/TS → C++ Translation (morph-js crate)

Two modes:

| Mode | Command | Behavior |
|---|---|---|
| **Legacy** | `morph file.ts` | `auto` inference, `Js*` types everywhere, no escape analysis |
| **Optimized** | `morph file.ts --optimize` | Intent-based: escape analysis → stack/`unique_ptr`/`shared_ptr`, native types (`int32_t`, `std::string`, `std::vector`), type widening only when needed |

See [Intent-Based Codegen](../guides/intent-based-codegen.md) for details.

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