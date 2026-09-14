# Contributing: Setup, Workflow, Conventions

**Part of:** [Dev Docs](overview.md)

How to get a working checkout, where contributions are most needed, and the conventions the codebase enforces. The canonical short version lives in [`CONTRIBUTING.md`](../../CONTRIBUTING.md) — this page adds the context behind it.

## Setup

```bash
git clone https://github.com/Levizr/morph.git
cd morph
cargo build --workspace   # morphc + morpher + all crates
target/debug/morph doctor # verify the toolchain
```

Everything is Rust. The CLI (`morphc` → `morph` binary) drives both direct file morphing (`morph foo.ts --to cpp`) and project builds (`.mx` → C++ → native binary via the compiler pipeline crates).

## Where help matters most

| Area | Difficulty | Impact |
|---|---|---|
| TS→C++ translator coverage (`crates/morpher/`) | Medium | Every new builtin/method is a workaround removed — start at [Morpher Internals](morpher-internals.md) |
| CSS cascade + selector matching (`crates/morph-parser/` CSS side) | Medium | Full cascade at runtime |
| Dev-mode hot reload + loopback IPC (`crates/morphc/src/commands/dev.rs`, `morph_devrt`) | Medium | Most-used developer surface |
| Layout engine parity across renderers (Flash / Forge) | Hard | Feature gap on complex layouts |
| Forge tile pool (`runtime/renderers/forge/`) | Hard | Content-keyed tile caching, LRU, scroll-shift remap |
| Tests, especially fixtures + JS→C++ + C++ runtime | Easy | Coverage is the safety net for all of the above |
| Docs | Easy | See [Docs System](docs-system.md) |

## Conventions

- **Rust**: `cargo fmt` clean, no new warnings. Morpher code follows the analyzer → emitter split — analysis in `analyzer.rs`, emission in `cpp.rs`, never mixed.
- **CLI**: one command per file under `crates/morphc/src/commands/`, `Result`-shaped errors, never panic on user input.
- **C++ runtime**: C++17 for `runtime/cpp` (header-only), C++23 for *generated* file-morph output (`g++-14 -std=c++23`). Don't confuse the two standards.
- **Errors**: `Result`/error enum patterns, no `unwrap()` in production paths, why-only comments (see `CODING_STANDARDS.md`).
- **Fixtures are sacred**: `tests/translate/fixtures/*.ts` plus their expected outputs are the translator's contract. If your change alters output for an existing fixture, that diff *is* the review.

## Workflow

1. Open an issue first for anything significant — design alignment before code.
2. One focused PR per feature/fix.
3. Add or update tests (`crates/*/` unit tests, `tests/translate/fixtures/` for morpher).
4. Run them: `cargo test --workspace` and, for the fixture suite, `python3 -m pytest tests/translate -v` (the fixtures drive the built `morph` binary; the only remaining Python in the repo).
5. Update docs if behavior changes: user docs for *use*, dev docs for *internals* (see [Docs System](docs-system.md)).
6. In the PR description, state what's working and what's still TODO — honestly, like the docs do.