# Contributing: Setup, Workflow, Conventions

**Part of:** [Dev Docs](../architecture/overview.md)

How to get a working checkout, where contributions matter most, and the conventions the codebase enforces — plus the reasoning behind each rule, so they read as engineering instead of bureaucracy. The canonical short version lives in [`CONTRIBUTING.md`](../../../CONTRIBUTING.md); this page is the director's commentary.

## Setup

```bash
git clone https://github.com/Levizr/morph.git
cd morph
cargo build --workspace   # morphc + morpher + all crates
target/debug/morph doctor # verify the toolchain: Rust, C++ compiler, OpenGL, GLFW, fonts
```

Everything is Rust. The CLI (`morphc` crate → `morph` binary) drives both direct file morphing (`morph foo.ts --to cpp`) and project builds (`.mx` → C++ → native binary through the pipeline crates). If `morph doctor` complains, fix what it names — it checks the actual toolchain in `morph-build`'s `platform.rs`, not a wishlist.

## Where help matters most

| Area | Difficulty | Impact | Start at |
|---|---|---|---|
| TS→C++ translator coverage (`crates/morpher/`) | Medium | Every new builtin/method is a workaround removed | [Morpher Internals](../morpher/morpher-internals.md) |
| Escape analysis / type decisions | Medium | Better types = smaller binaries, less `JsValue` tax | [Escape Analysis](../morpher/escape-analysis.md) |
| CSS cascade + selector matching (`morph-parser` CSS side, IR `style.rs`) | Medium | Full cascade at runtime | [crate-parser](../crates/crate-parser.md), [crate-ir](../crates/crate-ir.md) |
| Dev-mode hot reload + loopback IPC (`morphc/src/commands/dev.rs`, `morph-build`, `morph_devrt`) | Medium | The most-used developer surface | [Dev Mode](../architecture/dev-mode.md) |
| CLI + config + cache + releases | Easy–Medium | Everything users touch before they see a pixel | [CLI Tour](../build-cli/cli-tour.md) |
| Layout engine parity across renderers (Flash / Forge) | Hard | Feature gap on complex layouts | [Layout & Style](../runtime/layout-style-engine.md) |
| Forge tile pool (`runtime/cpp/renderers/forge/`) | Hard | Content-keyed tile caching, LRU, scroll-shift remap | [Rendering](../runtime/rendering-deep-dive.md) |
| Reactivity / async / networking | Medium | Correctness of every stateful app | [Reactivity Engine](../runtime/reactivity-engine.md) |
| Tests: fixtures, translator, runtime self-tests | Easy | The safety net for all of the above | [Testing](../testing/testing.md) |
| Docs (user + dev) | Easy | See [Docs System](docs-system.md) | — |

New contributors: tests and docs are not consolation prizes. Coverage is what lets everyone else move fast, and a fixture proving your understanding is worth a paragraph claiming it.

## Conventions (and why they exist)

- **Rust**: `cargo fmt` clean, no new warnings (`rustfmt.toml` + workspace Clippy config in `Cargo.toml`). Morpher code follows the analyzer → emitter split — analysis in `analyzer.rs`, emission in `cpp.rs`, never mixed — because mixed analysis/emission produces bugs that manifest three fixtures away.
- **CLI**: one command per file under `crates/morphc/src/commands/`, `Result`-shaped errors, never panic on user input. A typo is an occasion for a helpful sentence, not a stack trace.
- **C++ runtime**: C++17 for `runtime/cpp` (header-only), C++23 for *generated* file-morph output (`g++-14 -std=c++23`). Two standards, two rooms — don't confuse them or the headers stop compiling in generated units.
- **Errors**: `thiserror` for libraries, `anyhow` for applications, no `unwrap()` in production paths, why-only comments (full doctrine: `CODING_STANDARDS.md`).
- **Fixtures are sacred**: `tests/translate/fixtures/*.ts` plus expected outputs are the translator's contract. If your change alters output for an existing fixture, that diff *is* the review — inspect it line by line before asking anyone else to.
- **Dev docs**: file references required, decision flows over prose, "verify by" steps for anything load-bearing. If behavior changes for *users*, the user docs change in the same PR.

## Workflow

1. Open an issue first for anything significant — design alignment before code. Surprises are for birthdays, not PRs.
2. One focused PR per feature/fix. Reviewers can hold one idea in their head; give them one.
3. Add or update tests (`crates/*/` unit tests, `tests/translate/fixtures/` for morpher, runtime fixtures for runtime).
4. Run them:
   ```bash
   cargo fmt --all
   cargo test --workspace
   cargo clippy --all-targets --all-features -- -D warnings
   python3 -m pytest tests/translate -v   # fixtures drive the built binary
   ./tests/runtime/run-selftests.sh       # full-app self-tests
   ```
5. Update docs if behavior changes: user docs for *use*, dev docs for *internals* (see [Docs System](docs-system.md)).
6. In the PR description, state what's working and what's still TODO — honestly, like the docs do. A known limitation declared upfront is a footnote; discovered in review, it's a finding.
