# Contributing to Morph

Morph is in early development. All contributions — code, docs, bug reports, ideas — are welcome.

## Quick Start

```bash
git clone https://github.com/levizr/morph
cd morph
cargo build --workspace
target/debug/morph doctor
```

The entire toolchain is **Rust**. The only Python left in the repo is the translator's fixture-test harness under `tests/translate/` (it drives the built `morph` binary).

## Where to Start

The most impactful areas right now:

| Area | Difficulty | Impact | Guide |
|---|---|---|---|
| **TS→C++ translator coverage** (`crates/morpher/`) | Medium | Broaden supported JS surface (built-ins, arrays/objects in UI logic) | [Morpher Internals](docs/dev/morpher-internals.md) |
| **CSS Style Resolver** (`crates/morph-parser/` CSS side) | Medium | Full cascade + selector matching at runtime | [Repo Map](docs/dev/repo-map.md) |
| **Dev-mode hot reload + IPC** (`crates/morphc/src/commands/dev.rs`) | Medium | Most-used developer surface | [Repo Map](docs/dev/repo-map.md) |
| **Forge tile pool** (`runtime/renderers/forge/`) | Hard | Content-keyed tile caching, LRU budget, scroll-shift remap | [Repo Map](docs/dev/repo-map.md) |
| **Tests** | Easy | Increase coverage (esp. layout, JS→C++, C++ runtime) | [Testing Guide](docs/guides/testing.md) |
| **Bug fixes** | Varies | Any open issue | — |

## Code Conventions

- **Rust**: `cargo fmt` clean, no new warnings. See `CODING_STANDARDS.md` for why-only comments, error handling (no `unwrap()` in production paths), and nesting rules
- **C++**: C++17, header-only for runtime headers
- **Errors**: `Result`-shaped errors, never panic on user input
- **Naming**: follow existing conventions per crate

## Pull Request Process

1. Open an issue first for significant changes so we can align on design
2. Keep PRs focused — one feature/fix per PR
3. Add or update tests (`crates/*/` unit tests, `tests/translate/fixtures/` for morpher)
4. Run tests: `cargo test --workspace` and `python3 -m pytest tests/translate -v`
5. Update docs if changing public API or pipeline behavior
6. Mention in PR description what's working and what's still TODO

## Project Board

See the [Current State](./README.md#current-state-early-development) section in the README for a full breakdown of what's implemented, stubbed, and missing.