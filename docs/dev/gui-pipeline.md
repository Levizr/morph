# GUI Pipeline: Python vs Rust

**Part of:** [Dev Docs](overview.md)

The single most important internals page for contributors: which parts of `morph build` / `dev` / `run` each CLI owns, what breaks if you assume otherwise, and the exact sequence that ends with Python deleted. (User-facing summary: [What Still Needs the Python CLI](../guides/migration.md#what-still-needs-the-python-cli).)

## Build flow ownership

```
.mx source
   │
   ▼
┌──────────────┐   Python morph/jsx_walker.py + morph/ir/  ✅ full surface
│ Parse + IR   │   Rust morph-ir IRBuilder                ⚠️ drops props/events/effects,
   │               keys; x/y/w/h hardcoded 0.0
   ▼
┌──────────────┐   Python morph/layout/engine.py          ✅ measure + layout pass
│ Layout       │   Rust                                   ❌ missing entirely
   │
   ▼
┌──────────────┐   Python morph/js/codegen.py             ✅ full translator
│ JS → C++     │   Rust morph-codegen translate_js        ❌ string-level shim;
   │               morpher never called; premain JS
   │               emitted verbatim (won't compile)
   ▼
┌──────────────┐   both sides shell out to g++/CMake      ✅ same compilers
│ Compile      │
   │
   ▼
native binary
```

## Dev flow ownership

Python `morph dev`: watches sources → rebuilds IR → emits logic TU → compiles `logic.<hash>.so` → pushes over IPC (Unix socket / TCP) → running app `dlopen`s it live. Full hot reload.

Rust `morph dev`: verifies IR and stops (`crates/morphc/src/commands/dev.rs:118-119`). No logic-TU emit, no `.so` compile, no IPC push. The window opens; nothing hot-reloads.

## The removal sequence

Order matters — each step is verifiable before the next begins:

1. **Wire morpher into `morph-codegen` in strict mode.** GUI logic translated by the real translator instead of the shim. Verify: GUI fixture outputs unchanged.
2. **Parity harness.** Translate GUI snippets through both `TSToCppTranslator` and morpher-strict, diff. The harness *is* the definition of done for step 1 — no harness, no claim of parity.
3. **Port the layout engine.** Measure + layout pass in Rust, real `x/y/w/h` in IR. Verify: sample apps render identically (screenshot diff).
4. **Port dev hot-reload.** Logic-TU emit + `.so` compile + IPC push in `dev.rs`.
5. **Delete Python**: remove `morph/`, `pyproject.toml`, `python-publish.yml`, port or drop `tests/unit` + `tests/integration`, wire a Rust release workflow. Smoke-test `new` → `dev` → `build --static` → run before merging.

Until step 5 lands: **file morphing → Rust, `.mx` projects → Python.** Any PR that assumes otherwise will fail in ways no test catches (mispositioned widgets, untranslated logic) — which is exactly why the harness comes before the deletion.
