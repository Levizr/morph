# morphc — the Morph compiler CLI

Build native OpenGL applications from HTML, CSS, and JavaScript. No browser. No Electron. No WebView.

**morphc** is the Rust rewrite of the Morph toolchain. It provides the `morph` binary that compiles strict TypeScript (`.ts` / `.tsx`) and Morph (`.mx`) sources to native OpenGL binaries, and direct file morphing (`.ts`/`.js` → C++ or Rust).

## Install

```bash
cargo install morphc
```

This installs the `morph` binary.

## Commands

```
morph <file> --to cpp|rust    Direct file morphing (.ts/.js only)
morph new [name]              Scaffold a new .mx or .tsx project
morph install                 Download runtime sources
morph dev [--entry]           Start dev mode with live hot reload
morph build [flags]           Compile .ts/.tsx/.mx → native binary
morph run [binary] [flags]    Build and run production binary
morph check [PATH]            Lint source files
morph update [--runtime|--self]  Update runtime or morphc
morph doctor [-v]             Verify system dependencies
morph cache                   Manage fetched CSS cache
```

## Quick Start

```bash
morph new my-app --yes
cd my-app
morph install     # download the C++ runtime
morph dev         # live window with hot reload
morph run         # optimized native binary
```

## Direct File Morphing

```bash
morph app.ts              # → app.cpp (default target: C++)
morph app.ts --to cpp     # → app.cpp
morph app.ts --to rust    # → app.rs (experimental)
morph app.ts --to cpp --type strict   # respect type annotations
```

- Only `.ts` / `.js` files (`.tsx` / `.jsx` / `.mx` use `morph build` / `morph run`)
- Output: `<basename>.cpp` or `<basename>.rs`
- Intent-based codegen is always on: escape analysis picks stack allocation vs `unique_ptr` / `shared_ptr`; there is no `--optimize` flag

## Workspace Crates

| Crate | Purpose |
|---|---|
| `morph-config` | `morph.config.json` + `morph.lock` parsing |
| `morph-parser` | `.mx` parsing via Oxc + lightningcss |
| `morph-cache` | Global cache, runtime downloads, fingerprints |
| `morph-ir` | Intermediate Representation |
| `morph-codegen` | C++ / Rust code generation |
| `morph-build` | Build system, dev mode, IPC, packaging |
| `morpher` | Intent-based TS → C++/Rust translator |

## Requirements

- Rust 1.85+
- C++23 compiler (g++-14, clang++ 13+)
- OpenGL 3.3+, GLFW (optional), FreeType / HarfBuzz (optional)

## License

Apache-2.0