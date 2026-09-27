# Rust Compiler & Native CLI — Shipped

**Status:** shipped · **Shipped:** September 2026

> The Rust compiler and native CLI are complete. Python is removed from the repository. This page is kept as the design record.

## What shipped

- **Oxc parser** for `.mx`/`.ts`/`.tsx` — replaces tree-sitter; arena-allocated, spec-compliant, 3× faster than SWC
- **lightningcss** for CSS parsing — typed property values, browser-grade, parallel
- **Full Rust CLI** (`morphc` → `cargo install morphc`) — `new`, `install`, `update`, `dev`, `build`, `run`, `check`, `doctor`, `cache`, plus direct file morphing (`morph <file> --to cpp|rust`)
- **Intent-based codegen** via `crates/morpher` — escape analysis, type widening, `unique_ptr`/`shared_ptr` decisions at compile time, no GC
- **Python removed** — `morph/`, `pyproject.toml`, `tests/unit`, `tests/integration`, PyPI workflow all deleted September 2026

## Final crate layout

```
morph/
├── crates/
│   ├── morphc/            # Native CLI binary → `morph`
│   ├── morpher/           # TS→C++/Rust translator (Oxc + escape analysis)
│   ├── morph-parser/      # .mx parsing (Oxc + lightningcss)
│   ├── morph-ir/          # Intermediate Representation
│   ├── morph-codegen/     # C++ / Rust code emission
│   ├── morph-build/       # Compilation, dev rt + IPC, packaging
│   ├── morph-cache/       # Global cache, runtime downloads
│   └── morph-config/      # Config + lock file parsing
```

## What stayed identical for users

- `.mx` files, CSS, Tailwind, `morphState`/`morphEffect`, `fetch()`
- `morph dev`, `morph build`, `morph run`, `morph check`
- The JSON IR wire format
- Dev-mode hot reload

## Original design notes

### Why Python was used first

Morph didn't start with Rust because the goal at the start wasn't a fast compiler. The goal was to find out whether the concept even works — and Python was the right tool for that experiment:

- **Validate the idea first.** The core question was "can we parse `.mx` → IR → render natively?" Python answered that in days.
- **No recompile on every change.** The pipeline was changing daily. In Python, *save → run → see* is instant.
- **Better tools for text data.** Python's dicts/lists and standard library were purpose-built for shuffling structured text.

### SWC vs Oxc (the parser choice)

Morph chose **Oxc**: 3–4× faster than SWC parsing, arena allocator, spec-compliant, powers Rolldown (Vite's future bundler), and the oxlint ecosystem provides a natural home for `morph check`.

### Phases as shipped

| Phase | What |
|---|---|
| 1 | Parser swap: Oxc + lightningcss producing `MxSource` |
| 2 | Walker + IR builder port |
| 3 | Codegen port (node emitter, logic emitter, feature set) |
| 4 | Build system + dev mode (CMake devrt, loopback TCP IPC) |
| 5 | CLI finalization + crates.io publishing |
| 6 | Python deletion — all Python files, tests, and workflows removed |