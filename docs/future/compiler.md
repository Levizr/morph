# Rust Compiler (SWC / Oxc) & Native CLI

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

Today the entire toolchain is Python: tree-sitter parsing, JSX walking, IR building, CSS/Tailwind resolution, layout math, Jinja2 codegen, and the TS→C++ translator. The future is a **Rust-native compiler** — tree-sitter replaced by **SWC** or **Oxc** — and eventually a **native `morph` CLI with Python completely removed** for maximum performance.

## Why it matters

- **Compile speed** — Python orchestrates every build today (interpreter startup, dict-heavy IR, template rendering). A Rust compiler is an order of magnitude faster per stage and removes interpreter startup entirely
- **No Python dependency** — end users currently need Python 3.10+; a native binary means one self-contained install (`cargo install morph`, curl installer, or a release artifact)
- **Distribution** — `pip install levizr-morph` becomes a static binary download; `morph doctor` no longer checks Python
- **Ecosystem reuse** — Oxc powers **Rolldown** (the future bundler of Vite); SWC powers **Next.js** and **Rspack**. Both are battle-tested on TS/TSX parsing and transformation
- **Linting** — `morph check` can ride on oxlint's engine instead of a hand-rolled checker

## Current state (what gets replaced)

| Pipeline stage | Today (Python) | Future (Rust) |
|---|---|---|
| `.mx` / TS/TSX parsing | tree-sitter + `tree-sitter-typescript` grammar | SWC or Oxc parser |
| JSX walking + imports/props extraction | `jsx_walker.py` | Rust walker over the Rust AST |
| IR building | `ir/builder.py` | Rust IR builder (same IR shape) |
| CSS parse + cascade + Tailwind | `morph/style/` | Rust (or reuse the C++ style logic as a reference) |
| Layout math | `morph/layout/` (Python) | Rust layout engine — must stay pixel-identical |
| Codegen | Jinja2 templates (`node_emitter.py`) | Rust codegen emitting C++ / Rust source |
| TS→C++ / TS→Rust translation | `TSToCppTranslator` (Python) | Rust translator (SWC's TS parser + own emitter) |
| `morph check` | Python checker | Rust linter (oxlint-based) |
| CLI (`morph dev/build/run/...`) | Python `main.py` | Native Rust binary |

**Unchanged:** the C++ (or future Rust) runtime binaries, the JSON IR wire format, dev-mode IPC, and every user-facing command.

## Why Python first — and why the move now

*(Design history — written by [PIYUSH](https://github.com/Piyushthelagend), the original author.)*

Morph didn't start with Rust because the goal at the start wasn't a fast compiler. The goal was to find out whether the concept even works — and Python was the right tool for that experiment:

- **Validate the idea first.** The core question was "can we parse `.mx` → IR → render natively, and does it actually work?" Python answered that in days, not weeks. A Rust toolchain would have spent months on scaffolding before answering the same question.
- **No recompile on every change.** The pipeline was changing daily — new elements, new CSS properties, new IR fields. In Python, *save → run → see* is instant; every experiment happened without a compile step in between. That speed of iteration was worth more than compiler speed at that stage.
- **Better tools for text data.** The whole toolchain is shuffling structured text: tree-sitter ASTs, CSS rules, Tailwind classes, IR dicts, codegen templates. Python's dicts/lists, string handling, and standard library are purpose-built for exactly that — less friction, less code.
- **Less code to do the same thing.** The same pipeline in Rust is several times more code: types, lifetimes, error handling, ownership for every AST node. That cost is worth paying for a mature product — it's the wrong tax while you're still testing a hypothesis.

**So the order was deliberate: prove the concept with Python, then build the production compiler in Rust once the design is stable.**

That's exactly where we are now — and why the move is happening:

- The concept *is* proven: the C++ runtime ships, the pipeline is stable, and a full test suite exists
- Compile speed now matters: real apps rebuild the IR on every hot reload, and interpreter startup + Python overhead is the bottleneck
- Python is a real cost for users: every install requires Python 3.10+, and `morph doctor` has to babysit the toolchain
- The golden test corpus makes the port **safe** — every Rust stage must byte-match the Python output it replaces

## SWC vs Oxc — the parser choice

| | **SWC** (Speedy Web Compiler) | **Oxc** (Oxidation Compiler) |
|---|---|---|
| Maturity | Mature, production — used by Next.js, Rspack, Deno | Next-gen, rapidly maturing — powers Rolldown (Vite's future bundler), Oxlint |
| Parser | Very fast, full TS/TSX | **Ultra-fast** — reported 3–4× faster than SWC parsing |
| Transform/minify | Full transform + minifier | Transformer/minifier in active development |
| Linter | — | Oxlint — one of the fastest linters, natural home for `morph check` |
| Ecosystem | Huge (plugin ecosystem, wide adoption) | Growing (Rolldown, Vite-adjacent tooling) |
| Best for Morph | Safe, proven default; stable transform pipeline | Maximum speed; linter integration; aligns with the Rust runtime |

**Recommended path:** start with **SWC** for the parse/transform pipeline (mature, fewer surprises), evaluate **Oxc** as it stabilizes — the parser is swappable behind one AST interface. Use **oxlint** for the `morph check` engine either way.

## Target architecture

```
morph (Rust binary — single static artifact)
├── parser        # SWC/Oxc: TS/TSX/JSX → AST
├── walker        # .mx specifics: imports, components, props, JSX structure
├── css           # CSS parser, cascade, Tailwind utilities
├── layout        # box-model math (port of morph/layout/, golden-tested)
├── ir            # IR build + serialize (same JSON IR as today)
├── translate     # TS → C++ or TS → Rust (replaces TSToCppTranslator)
├── codegen       # C++ / Rust source emission (replaces Jinja2)
├── check         # semantic linting (oxlint-based)
└── cli           # dev / build / run / init / doctor / translate / pkg
```

- The compiler is a **library + thin CLI** — the same library the Python CLI calls today becomes the whole binary
- **Golden tests** guarantee parity: every stage's output must byte-match the Python pipeline's output for the existing examples and tests (the test suite is the migration harness)
- Dev mode keeps the same flow: `morph dev` still builds the runtime binary, watches files, and pushes IR over the Unix socket — just faster and without Python

## Suggested phases

1. **Parser swap** — Rust parse service producing the exact AST shape the walker consumes; Python shell calls it (CLI or FFI bridge). Zero behavior change, golden-tested against tree-sitter output
2. **Walker + IR builder in Rust** — Python becomes a thin orchestrator around Rust subcommands
3. **CSS / Tailwind / layout in Rust** — port with golden tests; pixel-parity with the C++ runtime layout is the acceptance bar
4. **Codegen + TS translation in Rust** — replace Jinja2 and `TSToCppTranslator`; `morph translate` emits from Rust
5. **`morph check` on oxlint** — reuse the Oxc linter engine for `.mx` diagnostics
6. **Native CLI** — `main.py` replaced; `morph` is a static Rust binary; drop the Python requirement
7. **Distribution** — release binaries per platform (Linux/macOS/Windows), `curl`/`cargo install` installers; PyPI package becomes a thin wrapper or is retired

## What stays identical for users

- `morph init/dev/build/run/check/translate/pkg` — same commands, same flags
- `.mx` files, CSS, Tailwind, `windowConfig`, `morphState`/`morphEffect`
- The JSON IR wire format, dev-mode socket protocol, hot reload
- `morph doctor` output (minus the Python check)

## Current state

| Building block | State |
|---|---|
| tree-sitter parsing + full Python pipeline | ✅ Shipped |
| Golden test corpus (unit + runtime tests) | ✅ Shipped — the parity harness |
| SWC / Oxc integration | ❌ Not started |
| Rust walker / IR / CSS / layout / codegen | ❌ Not started |
| Native CLI | ❌ Not started |

## Open questions

- **SWC vs Oxc** — proven SWC first, or bet on Oxc speed from day one? (Recommendation: SWC pipeline, oxlint for checks, Oxc parser behind the same interface once stable)
- **AST interface** — one internal AST type both parsers map into, or keep the parser's native AST?
- **FFI bridge during migration** — Rust core called from Python (napi-like without Node) vs. subprocess-per-stage (JSON in/out, simpler, slower)
- **Relation to the [Rust runtime](rust.md)** — a Rust toolchain + Rust runtime is the fully-native story; the C++ runtime remains supported (`--lang c++`) with the same Rust compiler
- **Python removal** — hard requirement or gradual? (Gradual: Python stays an optional dev fallback until phase 6 lands)

## Build steps (when picked up)

1. Rust workspace skeleton (`morph-compiler` crate) + parser swap with golden tests
2. Walker + IR builder port (existing `tests/unit/` suite must pass unchanged)
3. CSS/Tailwind/layout port with pixel-parity tests
4. Codegen + TS translation port; `morph translate` parity
5. oxlint-based `morph check`
6. Native CLI binary; retire Python; CI runs the full test matrix against it