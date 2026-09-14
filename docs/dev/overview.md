# Dev Docs: How Morph Works

**Part of:** [Dev Docs](overview.md)

This section is not user documentation. The [main docs](../index.md) teach you how to *use* Morph; these pages explain how Morph *works* — for contributors and the deeply curious. If you want to fix a bug, extend morpher, or just understand what happens between `morph build` and a native binary, start here.

## The pipeline

Everything is Rust. The `morph` binary compiles `.mx` / `.tsx` / `.ts` through the workspace crates (parse → IR → codegen → build) and drives direct file morphing (`.ts` → C++/Rust) through `morpher`.

| Input | Path |
|---|---|
| **Direct file morph** | `app.ts` `--to cpp\|rust` → `morpher` crate |
| **GUI project build** | `.mx` app (`morph build`/`dev`/`run`) → full workspace pipeline |

The complete flow is in [GUI Pipeline](gui-pipeline.md).

## Where to start, by goal

| Goal | Read first |
|---|---|
| Contribute code (setup, conventions, PRs) | [Contributing](contributing.md) + [`CONTRIBUTING.md`](../../CONTRIBUTING.md) |
| Orient in the repo | [Repo Map](repo-map.md) |
| Extend the TS→C++ translator | [Morpher Internals](morpher-internals.md), then [Intent-Based Codegen](../guides/intent-based-codegen.md) |
| Work on the GUI build / dev pipeline | [GUI Pipeline](gui-pipeline.md) |
| Add a runtime helper or type | [Runtime Layout](runtime-layout.md) |
| Write or fix documentation | [Docs System](docs-system.md) |

## Ground rules for this section

- **Internals, not promises.** Anything here can change without a migration note — it describes the implementation as it is, not a contract.
- **File:line references are load-bearing.** When code moves, update the reference. A dev doc pointing at the wrong file is worse than no doc.
- **User docs stay user docs.** If a change affects how Morph is *used*, it also needs an update in the main docs — say so in your PR.
