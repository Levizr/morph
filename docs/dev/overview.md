# Dev Docs: How Morph Works

**Part of:** [Dev Docs](overview.md)

This section is not user documentation. The [main docs](../index.md) teach you how to *use* Morph; these pages explain how Morph *works* — for contributors and the deeply curious. If you want to fix a bug, extend morpher, port the layout engine, or just understand what happens between `morph build` and a native binary, start here.

## The two pipelines

Morph has two translation pipelines, and confusing them is the #1 source of wasted contributor effort:

| Pipeline | Input | Owner | Output |
|---|---|---|---|
| **Direct file morph** | `app.ts` / `.mx` logic | Rust (`morpher` crate) | Native C++ (`--type infer`/`strict`, `--optimize`) |
| **GUI project build** | `.mx` app (`morph build`/`dev`/`run`) | Python CLI (`morph/`) + Rust shell | Windowed native binary |

Direct file morphing is fully Rust-owned and covered by fixtures. GUI builds are split — Python still owns the layout engine, dev hot-reload, and full JS translation. The complete map of who owns what is in [GUI Pipeline: Python vs Rust](gui-pipeline.md).

## Where to start, by goal

| Goal | Read first |
|---|---|
| Contribute code (setup, conventions, PRs) | [Contributing](contributing.md) + [`CONTRIBUTING.md`](../../CONTRIBUTING.md) |
| Orient in the repo | [Repo Map](repo-map.md) |
| Extend the TS→C++ translator | [Morpher Internals](morpher-internals.md), then [Intent-Based Codegen](../guides/intent-based-codegen.md) |
| Work on GUI builds or the Rust port | [GUI Pipeline: Python vs Rust](gui-pipeline.md) |
| Add a runtime helper or type | [Runtime Layout](runtime-layout.md) |
| Write or fix documentation | [Docs System](docs-system.md) |

## Ground rules for this section

- **Internals, not promises.** Anything here can change without a migration note — it describes the implementation as it is, not a contract.
- **File:line references are load-bearing.** When code moves, update the reference. A dev doc pointing at the wrong file is worse than no doc.
- **User docs stay user docs.** If a change affects how Morph is *used*, it also needs an update in the main docs — say so in your PR.
