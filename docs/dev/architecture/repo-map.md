# Repo Map

**Part of:** [Dev Docs](overview.md)

Where everything lives, which direction the dependencies point, and which door to knock on for which problem. Read this before grepping blindly — ten minutes here saves an afternoon of `find`-and-regret.

## Top level

| Path | What it is | Knock when |
|---|---|---|
| `crates/` | Rust workspace: the CLI and all translation machinery | Your bug involves anything between source text and `app.cpp` |
| `runtime/cpp/` | C++ runtime linked into every app (types, UI, renderers, net) | Your bug involves pixels, signals, or anything at app runtime |
| `tests/` | Translator fixtures (`tests/translate/`) + full-app smoke projects (`tests/runtime/`) | You changed the translator or the runtime and need proof |
| `docs/` | User docs (`docs/`) and these dev docs (`docs/dev/`, one dir per category) | Behavior changed and users or contributors must hear about it |
| `help/` | Deep-dive working notes (renderer design, rewrite plans, testing) | You want historical context — then promote finished thinking, don't link here |
| `versions/` | Version files — editing one is how a release gets cut | You're shipping |
| `changelog/` | Release notes with the project's history (including the Python era) | You want to know *why* something is the way it is |
| `examples/` | Sample apps (calculator, login, budget, …) | You need a manual verification target or a usage example |
| `my-app/` | The starter-template playground (with history and stray artifacts) | Never as a reference — it's a sandbox, not a spec |

## `crates/` — the Rust workspace

| Crate | Role | Deep dive |
|---|---|---|
| `morphc` | The `morph` binary — `new`, `install`, `update`, `dev`, `build`, `run`, `check`, `doctor`, `cache`, plus direct file morphing (`morph <file> --to cpp\|rust`) | [CLI Tour](../build-cli/cli-tour.md) |
| `morpher` | TS→C++/Rust translator: Oxc parse → escape analysis → intent-based emit | [Morpher Internals](../morpher/morpher-internals.md), [Escape Analysis](../morpher/escape-analysis.md), [JS Semantics](../morpher/js-semantics.md) |
| `morph-config` | `morph.config.json` / `morph.lock` / version-file handling | [Config, Cache & Versions](../build-cli/config-cache-versions.md) |
| `morph-parser` | `.mx`/TS parsing + JSX walker + module graph + linting | [crate-parser](../crates/crate-parser.md) |
| `morph-ir` | Intermediate representation: `IRBuilder`, styles, Tailwind, transforms, serializer | [crate-ir](../crates/crate-ir.md) |
| `morph-codegen` | C++ / Rust emission: node emitter, logic emitter, feature set | [crate-codegen](../crates/crate-codegen.md) |
| `morph-build` | Build orchestration: compilation, dev rt + IPC, platform, UPX, static deps | [Build Machine](build-system.md), [Dev Mode](dev-mode.md) |
| `morph-cache` | Global cache (`~/.morph/cache/runtimes/`), runtime download, fingerprints | [Config, Cache & Versions](../build-cli/config-cache-versions.md) |

Dependency direction: `morphc` → `morpher` / `morph-codegen` → `morph-ir` → `morph-parser`; `morph-build` and `morph-cache` support the CLI. There is no Python anywhere in the pipeline — the entire toolchain is Rust. The only Python in the repo is the translator's test harness (`tests/translate/*.py`): it drives the built binary, it ships nothing, and removing it would remove the test suite, so it stays.

## `runtime/cpp/` — the C++ runtime

Header-heavy, linked into every binary, and split by concern so you never have to hold the whole thing in your head. Full tour: [Runtime Layout](../runtime/runtime-layout.md). The neighborhoods, briefly:

- `types/` — the `Js*` value family + `morph::str` helpers (see [JS Semantics](../morpher/js-semantics.md))
- `reactivity/` — signals, effects, coroutines, channels (see [Reactivity Engine](../runtime/reactivity-engine.md))
- `net/` — the `fetch()` HTTP stack (see [`fetch()`](../runtime/networking.md))
- `core/` + `ui/` + `widgets/` + `style/` — nodes, window, layout, paint, computed style (see [Layout & Style](../runtime/layout-style-engine.md))
- `render/` + `renderers/` — GL backend plus flash (production) and forge (beta) (see [Rendering](../runtime/rendering-deep-dive.md))
- `dev/` — hot-reload + DevTools support, compiled out of production (see [Dev Mode](dev-mode.md))
- `viewport/` — embedded-canvas scaffolding (planned work, parser wiring still missing)
- `vendor/` — third-party code. Untouched, unformatted, unjudged.

Two standards live here and they are different rooms: **runtime headers are C++17** (header-only, included directly into generated units), **generated file-morph output is C++23** (`g++-14 -std=c++23`). Never track mud between them.

## `tests/` — proof, not decoration

| Path | Runs against | What it proves |
|---|---|---|
| `tests/translate/` | The `morph` binary: 29 fixtures (`.ts` → `.cpp` → compile → run, output diffed vs Node.js) + structural intent tests, via `python3 -m pytest tests/translate -v` | The translator tells the truth |
| `tests/runtime/` | Full-app smoke projects exercised by `morph new`/`build`/`dev`, via `run-selftests.sh` + `--morph-self-test` | The runtime keeps its promises |
| `tests/translate/fixtures/*.cpp` | Nothing — gitignored build artifacts | Never commit these |

Full doctrine: [Testing](../testing/testing.md).

## `docs/` vs `docs/dev/` vs `help/` — the three piles of words

- `docs/` — user documentation, rendered at `morph.levizr.com/docs`. **Promises.** Changes here need migration notes.
- `docs/dev/` — this section, rendered at `morph.levizr.com/dev/docs`. **Internals.** Organized one directory per category (`architecture/`, `crates/`, `morpher/`, `state/`, `runtime/`, `build-cli/`, `testing/`, `contributing/`, `bugs/`), registered in `docs/dev.registry.json`. Free to change without migration notes — but keep the registry in sync, or the page doesn't exist as far as the site is concerned.
- `help/` — working notes and design docs (renderer internals, rewrite plans, test setup). Raw material with historical value (including the Python era): promote finished thinking into `docs/` or `docs/dev/`, don't link users here. Anything describing the Python toolchain is history, not instruction.

And the two registries that route the whole site: `docs/docs.registry.json` (user track) and `docs/dev.registry.json` (this track). Both share the schema `{ title, slug, file, status, author, description, keywords, lastUpdated, publishedAt, priority, changefreq }`. How they work: [Docs System](../contributing/docs-system.md).
