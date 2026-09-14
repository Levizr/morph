# Working on morph — The Rust Rewrite

**Part of:** [The Story of Morph](index.md)

> This page documents the Rust rewrite of Morph's Python toolchain. The rewrite is **done** — Python is removed from the repo and the Rust `morph` binary is the only toolchain. This page is the record of how that happened.

## What happened

Morph's toolchain was rewritten from Python to Rust. The CLI is now **`morph`** (Morph Compiler) — a single lightweight Rust binary (`morphc` on crates.io, `cargo install morphc`) that replaced the entire Python dependency. The Python source was removed from the repository in September 2026.

**Why?** Python was the right choice to prove the concept — it validated parse `.mx` → IR → render natively in days. Once the concept was proven, compile speed mattered. The Rust compiler removes the Python dependency, makes `morph dev` instant, and gives users a single binary to install.

## What shipped

- **Rust-only pipeline** — parse (Oxc + lightningcss) → IR → codegen → build, all in-crate under `crates/`
- **Direct file morphing** — `morph app.ts --to cpp|rust`, intent-based codegen always on
- **Full CLI** — `new`, `install`, `update`, `dev`, `build`, `run`, `check`, `doctor`, `cache`
- **Dev mode with hot reload** — in-process compile + loopback TCP push to `morph_devrt`
- **Published on crates.io** — `cargo install morphc` (workspace: `morph-config`, `morph-parser`, `morph-cache`, `morph-ir`, `morph-codegen`, `morph-build`, `morpher`, `morphc`)
- **Python removed** — `morph/`, `tests/unit`, `tests/integration`, `pyproject.toml`, the PyPI workflow, and every Python pipeline file are gone. The only Python left is the fixture-test harness under `tests/translate/`, which drives the built `morph` binary.

## Intent-Based Codegen & Memory Management

The **JS/TS → C++ translator** (`crates/morpher`) uses **intent-based codegen with compile-time escape analysis**:

- **No garbage collector** — static escape analysis determines ownership at compile time
- **`shared_ptr` only where semantically required** — closures, shared mutable refs, async boundaries
- **Stack allocation for non-escaping locals** — `int`, `std::string`, `std::vector` on stack
- **`unique_ptr` + move for single-owner escapes** — returns, global storage
- **Type widening when needed** — `int` assigned from dynamic source → `JsNumber` automatically
- **Template bloat eliminated** — `<format>` only for template literals, not `console.log`
- **Native types preferred** — `int64_t` not `JsNumber` when usage allows

This is the only codegen mode — there is no `--optimize` flag; escape analysis runs on every translation.

## Architecture

### Pipeline (before → after)

```
Before (Python):  .mx → tree-sitter → Python AST → IR → Jinja2 → C++ → g++
After (Rust):     .mx → Oxc → Typed AST → IR → C++ emitter → g++/clang++
```

### Key decisions

| Decision | Choice | Why |
|----------|--------|-----|
| JSX/TSX parser | **Oxc** | 3x faster than SWC, arena-allocated, spec-compliant |
| CSS parser | **lightningcss** | Extremely fast, typed property values, browser-grade |
| CLI framework | **clap** (derive) | Industry standard, derive macros, completions |
| Runtime naming | `cpp` | Simple, clear |
| Binary name | `morph` | Follows `rustc`, `gcc` convention |

### Repository structure (single repo)

```
morph/
├── Cargo.toml               # Rust workspace
├── crates/
│   ├── morphc/             # CLI binary → `morph`
│   ├── morph-config/        # Config + lock file parsing
│   ├── morph-parser/        # Oxc + lightningcss → MxSource
│   ├── morph-ir/            # Intermediate Representation
│   ├── morph-codegen/       # C++ / Rust code generation
│   ├── morph-build/         # Build system, dev rt + IPC
│   ├── morph-cache/         # Global cache, runtime download
│   └── morpher/             # TS→C++/Rust translator
├── runtime/
│   └── cpp/                 # C++ runtime source (release artifact)
├── versions/                # Version files (release triggers)
├── tests/                   # translate fixtures + runtime smoke projects
└── docs/                    # user docs
```

## CLI commands

| Command | What it does |
|---------|--------------|
| `morph <file> [--to cpp\|rust]` | Direct file morphing (.ts/.js) |
| `morph new [name]` | Scaffold a new .mx or .tsx project |
| `morph install` | Download runtime from GitHub Releases, cache globally |
| `morph update [--runtime\|--self]` | Update runtime or morph binary |
| `morph dev` | Start dev mode with hot reload (loopback TCP) |
| `morph build` | Compile .mx → native binary |
| `morph run` | Build + run |
| `morph check` | Lint .ts/.tsx/.mx files |
| `morph doctor` | Verify system dependencies |
| `morph cache` | Manage fetched CSS cache |

## Version system

### Two separate versions

- **morph binary** — the CLI tool (e.g., `v0.1.0`)
- **Runtime** — the C++ runtime source (e.g., `v0.1.0`)

### Version files (release triggers)

```json
// versions/runtime/cpp.json
{
  "version": "0.1.0",
  "changelog": "Initial C++ runtime release",
  "breaking": false
}
```

Edit the file, push — CI builds releases automatically. No manual trigger needed.

### morph.config.json

```json
{
  "name": "my-app",
  "entry": "src/App.mx",
  "output": ".morph/output",
  "runtime": {
    "type": "cpp",
    "version": "0.1.0"
  },
  "window": {
    "width": 800,
    "height": 600,
    "title": "My App"
  }
}
```

## What stays the same for users

- `.mx` files, CSS, Tailwind — unchanged
- `morphState`, `morphEffect` — unchanged
- `morph dev`, `morph build`, `morph run` — same commands, same flags
- JSON IR wire format — unchanged (now produced and consumed in-process)
- Dev-mode hot reload — same UX (now over loopback TCP)

## What changed

- **No Python dependency** — single binary install
- **Much faster** — Oxc parsing, no interpreter startup
- **Global cache** — never re-download the same runtime version
- **Install** — `cargo install morphc` (no `pip install`)

## The Python toolchain

The Python toolchain served Morph well. It proved the concept, enabled rapid iteration, and made the project possible. When the Rust rewrite shipped, the Python code was removed from the repository together with its tests and PyPI workflow — its job is done, and keeping it around would only confuse contributors.

The Python version was never meant to ship to end users. It was the prototype that became the blueprint. The Rust version is the production tool: fast, self-contained, and zero-dependency.

## Implementation phases

- [x] Phase 1: Foundation (CLI, config, parser, new/install)
- [x] Phase 2: Parser (Oxc + lightningcss)
- [x] Phase 3: AST + IR (style registry, Tailwind, IRBuilder)
- [x] Phase 4: Codegen (node/logic emitters, feature set)
- [x] Phase 5: Build system (platform, g++/clang++ cross-platform, output .morph/output)
- [x] Phase 6: Dev mode (watch + loopback TCP IPC + hot reload)
- [x] Phase 7: Polish (check, doctor, crates.io distribution)
- [x] Phase 8: Python removal (delete `morph/`, Python tests, PyPI workflow)

## Follow the progress

This rewrite is complete. New work — features, the roadmap — lives in the [Future Plans](../future/index.md).

---

> Built with frustration, curiosity, and a refusal to accept "impossible" — [PIYUSH](https://github.com/Piyushthelagend)