# Morpher Internals

**Part of:** [Dev Docs](overview.md)

How the TS→C++ translator is built and — more importantly — where to cut when you want to extend it. For the *what and why* (intent model, escape analysis, memory story), read [Intent-Based Codegen](../guides/intent-based-codegen.md) first; this page is the contributor's cutting guide.

## Source layout (`crates/morpher/src/`)

| File | Owns |
|---|---|
| `lib.rs` | Public surface: `TranslateOptions { optimize, type_mode, runtime_path, indent }`, `TypeMode::{Strict, Infer}` |
| `codegen/analyzer.rs` | `EscapeAnalyzer`: escape/widen/usage/comparison collection, both fixpoints |
| `codegen/cpp.rs` | `CppTranslator`: all emission (~3.5k lines — navigate by function, not by reading) |
| `codegen/context.rs` | `Ctx`: `var_types`, `async_fns`, `escape_hints`, `shared_ptr_vars`, include-needs, indent |
| `codegen/type_resolver.rs` | Annotation → C++ maps, `Promise`→`Result` denormalization |
| `codegen/string_methods.rs` | `StringMethod` + `StringMethodHandler`: JS name → `morph::str::*` |
| `codegen/js_comparison.rs` | `OperandClass`, `ComparisonSignature`, per-file `morph::js_cmp` block builder |
| `codegen/rust.rs` | Experimental TS→Rust emission (not production) |

Rule: **analysis in `analyzer.rs`, emission in `cpp.rs`, never mixed.** If your change needs a new fact about the code, collect it in the analyzer and read it in the emitter.

## The decision flow per declaration

`emit_optimized_variable_declarator` (`codegen/cpp.rs:390`) decides everything about a `let`/`const` in this order:

1. **Base type** — strict: annotation, else infer from init. Infer: always from init (`infer_type_from_init` → `infer_optimized_type`, `cpp.rs:624`: literals → `std::string`/`bool`/`int32_t`/`int64_t`/`double`; homogeneous arrays → `vector<T>` recursively; `new Promise<T>` → `morph::Result<T>`; class instantiation → `shared_ptr<Class>`; fallback `JsValue`).
2. **Widening** — apply the analyzer's `widens` map (`ToJsNumber`/`ToJsString`/`ToJsValue`/`ToJsArray`); skipped entirely in infer mode.
3. **Sync strip** — `Result<T>` → `T` for plain sync call initializers (`strip_result_for_sync_call`, `cpp.rs:1337`).
4. **Storage** — `EscapeKind`: `None` → stack value; `Return`/`Global` → `unique_ptr` + `make_unique`; `ClosureCapture`/`MultipleRefs`/`AsyncBoundary` → `shared_ptr` + `make_shared`.
5. **Linkage/decoration** — `static` at file scope, `const`-fold for `Js*` scalars, record in `ctx.var_types` for later references.

## Common contributions, by file

| "I want to…" | Touch |
|---|---|
| Support a new string method (e.g. `padStart`) | Add the JS name + helper mapping in `string_methods.rs`, implement the helper in `runtime/cpp/types/js_string_helpers.h`, add a fixture case |
| Support a new array method natively | Check the widen list in `analyzer.rs:726` — methods listed there force `JsArray`. Native lowering needs an emitter path in `cpp.rs` plus the same fixture treatment |
| Add a `node:*`/global (e.g. `Math`) | Analyzer: stop rejecting / classify the operand; emitter: lower to the C++ equivalent (`<cmath>`); `morph check` diagnostics must be updated in step |
| Fix a comparison coercion | `js_comparison.rs` — find the (`OperandClass`, `OperandClass`, op) cell, fix the helper section, check `21_js_comparison`-style fixtures |
| Fix a type decision | `infer_optimized_type` for init-based, `type_resolver.rs` for annotation-based, widen rules in `analyzer.rs` for usage-based — know which of the three you mean before editing |
| Change pointer/storage selection | The `match escape_kind` block at `cpp.rs:455` — and re-run *all* fixtures, since this affects every declaration |

## Testing a morpher change

```bash
cargo test --workspace          # unit + translator tests
python -m pytest tests/translate/ -v   # 21 fixtures + 4 regression tests
```

The translate suite does the real verification: each fixture `.ts` is translated, compiled with `g++-14 -std=c++23`, run, and its output diffed against Node.js. If your change alters any existing fixture's `.cpp`, inspect that diff line by line — it is the review. Fixture `.cpp` files are gitignored; only the `.ts` sources and expected outputs are reviewed.
