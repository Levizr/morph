# Morpher Internals

**Part of:** [Dev Docs](../architecture/overview.md)

Morpher is the crate that performs the headline trick: TypeScript in, C++ out, no interpreter, no garbage collector, behavior identical to Node.js. It powers both hats of the `morph` binary — direct file morphing (`morph foo.ts --to cpp`) and the component-logic translation inside GUI builds. This page is the contributor's cutting guide: the source layout, the public surface, the decision flow, and exactly where each kind of change goes.

For the *what and why* — the intent model, escape analysis in depth, the memory story — read [Intent-Based Codegen](../../guides/intent-based-codegen.md) first, then [Escape Analysis](escape-analysis.md). For the JS-semantics machinery (comparisons, strings, the `Js*` family), see [JS Semantics](js-semantics.md). Here: the map.

## Public surface (`lib.rs`, `parser.rs`, `error.rs`)

| Item | Role |
|---|---|
| `TranslateOptions { type_mode, runtime_path, indent }` | The knobs: `TypeMode::{Strict, Infer}`, where the runtime lives, indent width |
| `translate(source, filename, options)` | The front door: full program → C++ |
| `translate_default` / `translate_fragment` / `translate_snippet` / `translate_with_indent` / `SnippetOutput` | Entry points for partial translation (GUI logic emitter uses the fragment/snippet paths) |
| `translate_rust` / `translate_to_rust` | Experimental TS→Rust emission — not production, same story as the codegen-side intern |
| `translate_file_to_cpp(path)` | File-level convenience over `translate` |
| `MorphJsError` (`error.rs`) | The error enum: unsupported syntax, type resolution failures, with source positions |
| `parser.rs` (`translate_to_cpp`, `translate_snippet_to_cpp`, `translate_str`) | Oxc parsing + translator orchestration — parse once, analyze, emit |
| `linter.rs` (`check_str`, `check_with_strictness`, `Strictness`) | Pre-translation diagnostics: unsupported syntax rejected early with named errors instead of confident garbage |

Errors are values, not panics: translation failures come back as `MorphJsError`, and user-facing commands render them as prose. An `unwrap()` on user input anywhere in this path is a bug.

## Source layout (`crates/morpher/src/`)

| File | Owns | Size (approx) |
|---|---|---|
| `codegen/analyzer.rs` | `EscapeAnalyzer`: escape/widen/usage/comparison collection, both fixpoints | ~2.9k lines |
| `codegen/cpp.rs` | `CppTranslator`: all emission — navigate by function, never by reading | ~4.5k lines |
| `codegen/context.rs` | `Ctx`: `var_types`, `async_fns`, `escape_hints`, `shared_ptr_vars`, `fn_return_types`, include-needs (`need()` / `generate_includes()`), indentation (`indent()`, `sub()`, `merge()`), depths, `async_result_type()` | the notebook |
| `codegen/type_resolver.rs` | Annotation → C++ maps, `Promise`→`Result` denormalization, `headers_for()` include registration | the dictionary |
| `codegen/string_methods.rs` | `StringMethod` + `StringMethodHandler`: JS name → `morph::str::*` | the phrasebook |
| `codegen/js_comparison.rs` | `OperandClass`, `ComparisonSignature`, per-file `morph::js_cmp` block builder | the etiquette guide |
| `codegen/rust.rs` | Experimental TS→Rust emission | not production |
| `codegen/mod.rs` | Module wiring | — |

The iron rule, repeated because it saves careers: **analysis in `analyzer.rs`, emission in `cpp.rs`, never mixed.** If your change needs a new fact about the code, collect it in the analyzer and read it in the emitter. Smuggling analysis into the emitter produces bugs that manifest three fixtures away. `Ctx` is the only sanctioned channel between the two — `var_types` for decisions, `need()` for includes (new headers must register via `Ctx::need()` / `headers_for()`, or generated files won't compile).

## The pipeline inside the crate

```
source text
   │  Oxc parse (parser.rs)
   ▼
AST + diagnostics
   │  linter (check_str): reject the unsupported early
   ▼
EscapeAnalyzer: two passes to fixpoint
   │  (escape kinds, widening map, usage, comparisons)
   ▼
CppTranslator + Ctx: emit
   │  (base type → widen → sync-strip → storage → linkage)
   ▼
C++ source (includes via generate_includes)
```

The analyzer runs to fixpoint because facts beget facts: learning that `x` escapes into a closure re-opens the closure's captures, which may reveal `y` escapes too. The loop exits when a full pass learns nothing — termination is guaranteed because each pass only *adds* facts from a finite set.

## The decision flow per declaration

`emit_typed_variable_declarator` (`codegen/cpp.rs`) decides everything about a `let`/`const` in this order (worked examples in [Escape Analysis](escape-analysis.md)):

1. **Base type** — strict: annotation, else infer from init. Infer: always from init (`infer_type_from_init`: literals → `std::string`/`bool`/`int64_t`/`double`; homogeneous arrays → `vector<T>` recursively; `new Promise<T>` → `morph::Result<T>`; class instantiation → `shared_ptr<Class>`; fallback `JsValue`, `auto` for deducible expressions).
2. **Widening** — apply the analyzer's `widens` map (`ToJsNumber`/`ToJsString`/`ToJsValue`/`ToJsArray`), except a trusted native-number annotation beats `ToJsNumber`/`ToJsValue`, and proven-small integers refine to `int32_t`.
3. **Sync strip** — `Result<T>` → `T` for plain sync call initializers (`strip_result_for_sync_call`).
4. **Storage** — `EscapeKind`: `None` → stack value; `Return`/`Global` → `unique_ptr` + `make_unique` for class/vector, stack copy for scalars; `ClosureCapture`/`MultipleRefs`/`AsyncBoundary` → `shared_ptr` + `make_shared` (scalar use sites dereference).
5. **Linkage/decoration** — `static` at file scope, `const`-fold for `Js*` scalars, record in `ctx.var_types` for later references.

The aesthetic throughout: stay native when provable (`std::string` over `JsString`, `int32_t` over `int64_t`), spend `JsValue` only where JS semantics are genuinely needed. A `JsValue` baseline drags in `<format>` and roughly half a megabyte — the variant tax, paid only when owed.

## How morpher serves the GUI pipeline

Morpher is not a rival of the GUI pipeline — it is the same brain in a second hat. The logic emitter leans on the fragment/snippet entry points to translate component logic (handlers, effects, reactive expressions), while `morph <file> --to cpp` translates whole files standalone. A morpher improvement (new method, better inference, fixed coercion) improves both consumers at once — and can break both at once, which is why the fixture suite covers both shapes (behavioral *and* structural; see [Testing](../testing/testing.md)).

## Common contributions, by file

| "I want to…" | Touch |
|---|---|
| Support a new string method (e.g. `padStart`) | JS name + helper mapping in `string_methods.rs` → helper in `runtime/cpp/types/js_string_helpers.h` → fixture case |
| Support a new array method natively | Check the array-method list in `analyze_call_expression` — listed methods force `JsArray`. Native lowering needs an emitter path in `cpp.rs` plus the same fixture treatment |
| Add a `node:*`/global (e.g. `Math`) | Analyzer: stop rejecting / classify the operand; emitter: lower to the C++ equivalent (`<cmath>`); `morph check` diagnostics updated in step — linter and emitter must never disagree about what exists |
| Fix a comparison coercion | `js_comparison.rs` — find the (`OperandClass`, `OperandClass`, op) cell, fix the helper section, check `21_js_comparison`-style fixtures |
| Fix a type decision | `infer_type_from_init` for init-based, `type_resolver.rs` for annotation-based, widen rules in `analyzer.rs` for usage-based — know which of the three you mean before editing |
| Change pointer/storage selection | The escape-allocation block in the typed declarator — and re-run *all* fixtures, since this affects every declaration |
| Add an include for a new helper | `Ctx::need()` at the use site + `headers_for()` in `type_resolver.rs` — unregistered headers compile in no generated file |
| Reject new unsupported syntax early | `linter.rs` with a named diagnostic — the emitter should never see what the linter can't describe |

## Testing a morpher change

```bash
cargo test --workspace          # unit + translator tests
python -m pytest tests/translate/ -v   # fixtures + intent tests
```

The translate suite does the real verification: each fixture `.ts` is translated, compiled with `g++-14 -std=c++23`, run, and its output diffed against Node.js. If your change alters any existing fixture's `.cpp`, inspect that diff line by line — it is the review. Fixture `.cpp` files are gitignored; only the `.ts` sources and expected outputs are reviewed.
