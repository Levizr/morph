# Repo Map

**Part of:** [Dev Docs](overview.md)

Where everything lives and which direction the dependencies point. Read this before grepping blindly.

## Top level

| Path | What it is |
|---|---|
| `crates/` | Rust workspace: the CLI and all translation machinery |
| `runtime/cpp/` | C++ runtime linked into every app (types, UI, renderers, net) |
| `tests/` | Translator fixtures (`tests/translate/`) + full-app smoke projects (`tests/runtime/`) |
| `docs/` | User docs (`docs/`) and these dev docs (`docs/dev/`) |
| `help/` | Deep-dive working notes (renderer design, rewrite plans, testing) |
| `versions/` | Version files (release triggers) |
| `examples/` | Sample apps (calculator, login, ...) |

## `crates/` — the Rust workspace

| Crate | Role |
|---|---|
| `morphc` | The `morph` binary — all CLI commands: `new`, `install`, `update`, `dev`, `build`, `run`, `check`, `doctor`, `cache`, plus direct file morphing (`morph <file> --to cpp\|rust`) |
| `morpher` | TS→C++/Rust translator: Oxc parse → escape analysis → intent-based emit. Deep dive: [Morpher Internals](morpher-internals.md) |
| `morph-config` | `morph.config.json` / `morph.lock` / version-file handling |
| `morph-parser` | `.mx`/TS parsing + JSX walker + linting on the Rust side |
| `morph-ir` | Intermediate representation: `IRBuilder`, style, Tailwind, transforms, serializer |
| `morph-codegen` | C++ / Rust code emission: node emitter, logic emitter, feature set |
| `morph-build` | Build orchestration: compilation, dev rt + IPC, platform, UPX, static deps |
| `morph-cache` | Global cache (`~/.morph/cache/runtimes/`), runtime download, fingerprints |

Dependency direction: `morphc` → `morpher` / `morph-codegen` → `morph-ir` → `morph-parser`; `morph-build` and `morph-cache` support the CLI. There is no Python anywhere in the pipeline — the entire toolchain is Rust.

## `runtime/cpp/` — the C++ runtime

Header-heavy, linked into every binary. Full tour: [Runtime Layout](runtime-layout.md). The parts contributors touch most: `types/` (Js* wrappers + `morph::str` helpers), `net/` (fetch/HTTP), `reactivity/` (state/effects), `ui/` + `renderers/` (flash production, forge beta).

## `tests/`

| Path | Runs against |
|---|---|
| `tests/translate/` | The Rust `morph` binary: fixtures (`.ts` → `.cpp` → compile → run, output must match Node.js) + regression/intent tests, via `python3 -m pytest tests/translate -v` |
| `tests/runtime/` | Full-app smoke projects (`.mx` apps with configs) exercised by `morph new`/`build`/`dev` |
| `tests/translate/fixtures/*.cpp` | Gitignored build artifacts — never commit these |

The only Python in the repo is the translator's test harness (`tests/translate/*.py`) — it drives the built binary, it is not part of the toolchain.

## `help/` vs `docs/` vs `docs/dev/`

- `docs/` — user documentation, rendered at `morph.levizr.com/docs`. Promises.
- `docs/dev/` — this section, rendered at `morph.levizr.com/dev/docs`. Internals.
- `help/` — working notes and design docs (renderer internals, rewrite plans, test setup). Raw material: promote finished thinking into `docs/` or `docs/dev/`, don't link users here. Anything describing the Python toolchain is historical.