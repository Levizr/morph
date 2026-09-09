# Contributing: Setup, Workflow, Conventions

**Part of:** [Dev Docs](overview.md)

How to get a working checkout, where contributions are most needed, and the conventions the codebase enforces. The canonical short version lives in [`CONTRIBUTING.md`](../../CONTRIBUTING.md) — this page adds the context behind it.

## Setup

```bash
git clone https://github.com/Levizr/morph.git
cd morph
pip install -e ".[dev]"   # Python CLI + dev extras
cargo build --workspace   # Rust CLI (morphc + morpher + friends)
morph doctor              # verify the toolchain
```

You need both toolchains: Python runs the GUI pipeline and most tests, Rust owns direct file morphing. See [GUI Pipeline: Python vs Rust](gui-pipeline.md) for why both still exist.

## Where help matters most

| Area | Difficulty | Impact |
|---|---|---|
| TS→C++ translator coverage (`crates/morpher/`) | Medium | Every new builtin/method is a workaround removed — start at [Morpher Internals](morpher-internals.md) |
| CSS cascade + selector matching (`morph/style/`) | Medium | Full cascade at runtime |
| GUI parity harness (Python `TSToCppTranslator` vs morpher-strict) | Medium | Unlocks the Python removal sequence |
| Layout engine port (Python → Rust) | Hard | The biggest remaining GUI gap |
| Forge tile pool (`runtime/renderers/forge/`) | Hard | Content-keyed tile caching, LRU, scroll-shift remap |
| Tests, especially layout + JS→C++ + C++ runtime | Easy | Coverage is the safety net for all of the above |
| Docs (both tracks) | Easy | See [Docs System](docs-system.md) |

## Conventions

- **Python**: 3.10+, type hints everywhere, `from __future__ import annotations` in new files. Imports: stdlib → third-party → morph modules, blank line between groups. `snake_case` functions, `PascalCase` classes, `UPPER_CASE` constants.
- **Rust**: `cargo fmt` clean, no new warnings. Morpher code follows the analyzer → emitter split — analysis in `analyzer.rs`, emission in `cpp.rs`, never mixed.
- **C++ runtime**: C++17 for `runtime/cpp` (header-only), C++23 for *generated* file-morph output (`g++-14 -std=c++23`). Don't confuse the two standards.
- **Errors**: custom exception classes in the `morph/parser/errors.py` pattern for Python; `Result`-shaped errors, never panics on user input, in Rust.
- **Fixtures are sacred**: `tests/translate/fixtures/*.ts` plus their expected outputs are the translator's contract. If your change alters output for an existing fixture, that diff *is* the review.

## Workflow

1. Open an issue first for anything significant — design alignment before code.
2. One focused PR per feature/fix.
3. Add or update tests (`tests/` for Python, fixtures for morpher).
4. Run them: `python -m pytest tests/ -v` and `cargo test --workspace`.
5. Update docs on **both** tracks if behavior changes: user docs for *use*, dev docs for *internals* (see [Docs System](docs-system.md)).
6. In the PR description, state what's working and what's still TODO — honestly, like the docs do.
