# Repo Map

**Part of:** [Dev Docs](overview.md)

Where everything lives and which direction the dependencies point. Read this before grepping blindly.

## Top level

| Path | What it is |
|---|---|
| `crates/` | Rust workspace: the CLI and all translation machinery |
| `morph/` | Python CLI: GUI pipeline (parser, layout, dev server, build) |
| `runtime/cpp/` | C++ runtime linked into every app (types, UI, renderers, net) |
| `tests/` | Python test suites + translate fixtures |
| `docs/` | User docs (`docs/`) and these dev docs (`docs/dev/`) |
| `help/` | Deep-dive working notes (renderer design, package authoring, testing) |
| `my-app/` | Sample app (disposition still open — see migration notes) |

## `crates/` — the Rust workspace

| Crate | Role |
|---|---|
| `morpher` | TS→C++ translator: Oxc parse → escape analysis → intent-based emit. Deep dive: [Morpher Internals](morpher-internals.md) |
| `morphc` | The `morph` binary: `translate`, `build`, `dev`, `run`, `new`, `check` commands |
| `morph-codegen` | GUI C++ emitter (JSX/IR → translation unit). Currently a thin shim over string-level `translate_js` — see [GUI Pipeline](gui-pipeline.md) |
| `morph-ir` | GUI intermediate representation (`IRBuilder` — layout fields currently hardcoded `0.0`) |
| `morph-parser` | `.mx`/TS parsing on the Rust side |
| `morph-config` | `morph.config.json` / `morph.lock` handling |
| `morph-cache` | Global cache (`~/.morph/cache/runtimes/`) |
| `morph-build` | Build orchestration helpers |

Dependency direction: `morphc` → `morpher` / `morph-codegen` → `morph-ir` → `morph-parser`. `morph-codegen` does **not** call `morpher` yet — that wiring is the first step of the Python removal sequence.

## `morph/` — the Python CLI

| Module | Role |
|---|---|
| `morph/js/codegen.py` | `TSToCppTranslator` — the full JS→C++ translator GUI builds use |
| `morph/layout/engine.py` | Measure + layout pass; writes real `x/y/w/h` |
| `morph/dev/` | Dev server: watches, recompiles `logic.<hash>.so`, pushes over IPC |
| `morph/cli/` | `cmd_build.py`, `cmd_dev.py`, `cmd_run.py`, ... |
| `morph/jsx_walker.py`, `morph/ir/` | JSX → IR (props, events, effects, full surface) |
| `morph/style/` | Selector engine + (partial) cascade |
| `morph/pkg/` | `morph pkg` package CLI (downloaded, not compiled — see [Packages](../future/packages.md)) |

## `runtime/cpp/` — the C++ runtime

Header-heavy, linked into every binary. Full tour: [Runtime Layout](runtime-layout.md). The parts contributors touch most: `types/` (Js* wrappers + `morph::str` helpers), `net/` (fetch/HTTP), `reactivity/` (state/effects), `ui/` + `renderers/` (flash production, forge beta).

## `tests/`

| Path | Runs against |
|---|---|
| `tests/translate/` | The Rust binary: 21 fixtures (`.ts` → `.cpp` → compile → run, output must match Node) + 4 regression tests |
| `tests/unit/`, `tests/integration/` | Python `morph.*` directly (14 suites) — the reason Python can't be deleted yet |
| `tests/translate/fixtures/*.cpp` | Gitignored build artifacts — never commit these |

## `help/` vs `docs/` vs `docs/dev/`

- `docs/` — user documentation, rendered at `morph.levizr.com/docs`. Promises.
- `docs/dev/` — this section, rendered at `morph.levizr.com/dev/docs`. Internals.
- `help/` — working notes and design docs (renderer internals, package authoring, test setup). Raw material: promote finished thinking into `docs/` or `docs/dev/`, don't link users here.
