# Dev Docs: How Morph Works

**Part of:** [Dev Docs](overview.md)

Welcome to the engine room. The [main docs](../../index.md) teach you how to *use* Morph; these pages explain how Morph *works* — for contributors and the deeply curious. If you want to fix a bug, extend the translator, add a runtime feature, or just understand what happens between `morph build` and a native binary, start here and follow the table below.

## The pipeline in thirty seconds

Everything is Rust. The `morph` binary compiles `.mx` / `.tsx` / `.ts` through the workspace crates (parse → IR → codegen → build) and drives direct file morphing (`.ts` → C++/Rust) through `morpher`. No Python anywhere in the pipeline — the only Python left in the repo is the translator's test harness, and it ships nothing.

| Input | Path |
|---|---|
| **Direct file morph** | `app.ts` `--to cpp\|rust` → `morpher` crate |
| **GUI project build** | `.mx` app (`morph build`/`dev`/`run`) → full workspace pipeline |

Two pages tell this story at different altitudes: [GUI Pipeline](gui-pipeline.md) is the short version, [Compiler Pipeline](compiler-pipeline.md) follows one component through every stage with names, files, and a worked example.

## The crates at a glance

| Crate | One-line job | Deep dive |
|---|---|---|
| `morph-parser` | Reads text, produces facts (`MxSource`, `ModuleGraph`, lints) | [crate-parser](../crates/crate-parser.md) |
| `morph-ir` | Thinks: facts → plan (`IRWindow`/`IRNode`, styles, Tailwind, transforms) | [crate-ir](../crates/crate-ir.md) |
| `morph-codegen` | Writes C++: plan → `app.cpp` + headers, feature-gated | [crate-codegen](../crates/crate-codegen.md) |
| `morpher` | Translates standalone `.ts` → C++ (escape analysis + intent emit) | [Morpher Internals](../morpher/morpher-internals.md) |
| `morph-build` | Compiles, fingerprints, links, compresses, serves dev mode | [Build Machine](build-system.md) |
| `morph-cache` | Global runtime pantry (`~/.morph`), downloads, linkage | [Config, Cache & Versions](../build-cli/config-cache-versions.md) |
| `morph-config` | Project config, lockfiles, version files | [Config, Cache & Versions](../build-cli/config-cache-versions.md) |
| `morphc` | The `morph` binary itself: every CLI command | [CLI Tour](../build-cli/cli-tour.md) |

Dependency direction: `morphc` → `morpher` / `morph-codegen` → `morph-ir` → `morph-parser`; `morph-build` and `morph-cache` support the CLI. Data flows one way down that chain — nothing downstream ever edits an upstream structure, which means every bug lives in the *earliest* stage whose output looks wrong.

## Where to start, by goal

| Goal | Read first |
|---|---|
| Contribute code (setup, conventions, PRs) | [Contributing](../contributing/contributing.md) + [`CONTRIBUTING.md`](../../../CONTRIBUTING.md) |
| Orient in the repo | [Repo Map](repo-map.md) |
| Follow one file from `.mx` to C++ | [Compiler Pipeline](compiler-pipeline.md), then [GUI Pipeline](gui-pipeline.md) |
| Read the crate that reads (parsing, graph, lints) | [morph-parser](../crates/crate-parser.md) |
| Read the crate that thinks (IR, Tailwind, builder) | [morph-ir](../crates/crate-ir.md) |
| Read the crate that writes C++ (emitters, features) | [morph-codegen](../crates/crate-codegen.md) |
| Extend the TS→C++ translator | [Morpher Internals](../morpher/morpher-internals.md), then [Intent-Based Codegen](../../guides/intent-based-codegen.md), then [Escape Analysis](../morpher/escape-analysis.md) |
| Understand memory without a GC | [Escape Analysis](../morpher/escape-analysis.md) |
| Understand JS-in-C++ (comparisons, strings, types) | [JS Semantics](../morpher/js-semantics.md) |
| Work on state, shared stores, or events | [State & Event Internals](../state/state-events-internals.md), then [Reactivity Engine](../runtime/reactivity-engine.md) |
| Work on the GUI build / dev pipeline | [GUI Pipeline](gui-pipeline.md), [Dev Mode](dev-mode.md), [Build Machine](build-system.md) |
| Work on the CLI, config, cache, or releases | [CLI Tour](../build-cli/cli-tour.md), [Config, Cache & Versions](../build-cli/config-cache-versions.md) |
| Work on signals, effects, async, or networking | [Reactivity Engine](../runtime/reactivity-engine.md), [`fetch()`](../runtime/networking.md) |
| Work on rendering, layout, or style | [Rendering Deep Dive](../runtime/rendering-deep-dive.md), [Layout & Style Engine](../runtime/layout-style-engine.md) |
| Add a runtime helper or type | [Runtime Layout](../runtime/runtime-layout.md), [JS Semantics](../morpher/js-semantics.md) |
| Write a test or a fixture | [Testing](../testing/testing.md) |
| Write or fix documentation | [Docs System](../contributing/docs-system.md) |

## Ground rules for this section

- **Internals, not promises.** Anything here can change without a migration note — it describes the implementation as it is, not a contract. User docs make promises; dev docs tell the truth about today.
- **File references are load-bearing.** When code moves, update the reference. A dev doc pointing at the wrong file is worse than no doc — it sends the next contributor on a hike.
- **User docs stay user docs.** If a change affects how Morph is *used*, it also needs an update in the main docs — say so in your PR.
- **Every page earns its keep the same way:** concepts before mechanics, a worked example, a "where to cut" table, and verify steps. If a page lacks those, it is a draft wearing a finished page's clothes.
