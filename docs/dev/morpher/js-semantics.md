# JavaScript Wearing a C++ Costume

**Part of:** [Dev Docs](../architecture/overview.md)

Morph promises that your TypeScript behaves like TypeScript after becoming C++. That promise is kept by three pieces of machinery working in concert: a type resolver that maps annotations to C++, a comparison engine that reproduces JS coercion without a runtime, and a string/array lowering layer that turns `.slice()` into `morph::str::*` calls. Plus the costume department itself — the `Js*` value family in `runtime/cpp/types/`. This page tours all four, with examples of each lie told convincingly.

## Piece 1 — The type resolver (`type_resolver.rs`)

Annotation → C++ maps, plus the `Promise` → `Result` denormalization (an `async` function's return type becomes `morph::Result<T>`, the eager awaitable in `runtime/cpp/reactivity/promise.h`). Two modes:

- **`Strict`**: annotations are law. Say `: number` and you get a native double-or-int; the compiler holds you to it.
- **`Infer`**: annotations are hints, init expressions are evidence. `let x = "hi"` is a string because it obviously is one.

Unannotated awaits become `auto ... = co_await f();` — the compiler refuses to guess what the coroutine returns and lets C++ deduction do the honors. And `: int` beats dynamic widening, because a trusted annotation is a contract: you promised it fits in 32 bits, and widening backs off.

The cross-language map (also pinned in `CODING_STANDARDS.md`) is worth memorizing:

| TypeScript | C++ |
|---|---|
| `number` | `double` / `int32_t` / `JsNumber` (proven range decides) |
| `string` | `std::string` / `JsString` |
| `boolean` | `bool` / `JsBoolean` |
| `Array<T>` | `std::vector<T>` / `JsArray` |
| `object` | `std::map` / `JsObject` |
| `Promise<T>` | `morph::Task<T>` / `morph::Result<T>` |
| `null` / `undefined` | `JsNull{}` / `JsUndefined{}` |
| `any` | `JsValue` |

Rule of thumb: the left column is what you write, the right column's *first* option is what you get when the compiler can prove it, and the `Js*` option is the honest fallback when it can't.

## Piece 2 — The comparison engine (`js_comparison.rs`)

JavaScript's `==` is famously unhinged (`[] == ![]` is `true`; don't think about it too hard). C++'s `==` is a strict Victorian. Somebody has to bridge the gap, and that somebody is `OperandClass` + `ComparisonSignature`: every comparison is classified by the operand kinds and the operator, and each cell of that matrix gets a generated `morph::js_cmp` helper block per file.

Examples of the matrix at work:

```ts
a == b    // mixed types  → morph::js_cmp::loose_eq(a, b)   (JS semantics, generated helper)
a === b   // strict       → direct comparison where provable
x < y     // numbers      → plain operator, zero overhead
if (v)    // truthiness   → JS truthiness rules, not C++ truthiness
a && b    //              → JS short-circuit value semantics
```

Same-type comparisons stay direct — no helper, no overhead. The engine only spends money where JS semantics actually differ from C++ semantics. Fix a coercion bug by finding the (`OperandClass`, `OperandClass`, op) cell and fixing that helper section, then check the `21_js_comparison`-style fixtures. The matrix is the territory; the fixtures are the map.

## Piece 3 — String and array lowering (`string_methods.rs` + friends)

Every JS string method you use must land somewhere in C++. The mapping lives in `StringMethod` + `StringMethodHandler`: JS name → `morph::str::*` free function. The runtime side follows the **`morph::str` pattern** — JS-shaped behavior over native storage, free functions in a namespace, no classes unless state is required. `runtime/cpp/types/js_string_helpers.h` is the template to copy when adding one.

```ts
s.slice(1, 4)     // → morph::str::slice(s, 1, 4)
s.padStart(8)     // → (add the mapping + the helper + a fixture case)
```

Array methods go through the same idea: methods listed in the analyzer's array-method list force `JsArray`; natively-lowered ones need an emitter path in `cpp.rs` plus fixture proof of Node-identical output. And `node:*` globals / `Math` lower to C++ equivalents (`<cmath>`) — with `morph check` diagnostics updated in step, so the linter and the emitter never disagree about what exists.

To add a method, the checklist is always the same three steps: **mapping** (the `.rs` side), **helper** (the runtime header), **fixture** (a `.ts` case proving Node-identical output). Skip the fixture and the method is a rumor.

## Piece 4 — The costume department (`runtime/cpp/types/`)

When values genuinely need JS semantics, they wear the `Js*` family:

| Header | Contents |
|---|---|
| `js_value.h` | `JsValue` itself — the variant that can be anything, plus dispatch |
| `js_number.h` | `JsNumber` — NaN-payload doubles plus a separate i64 path |
| `js_string.h`, `js_string_helpers.h` | `JsString` + the `morph::str` free-function helpers |
| `js_array.h` | `JsArray` — elements in a `shared_ptr` (JS reference semantics *require* shared ownership; copies alias, they don't clone) |
| `js_object.h` | `JsObject` — properties in a `shared_ptr`, same reasoning |
| `js_boolean.h` | `JsBoolean` (plus `JsNull` / `JsUndefined` elsewhere in the family) |
| `js_value_format.h` | Printing: `JsValue` formatting, vector/optional `println` formatters |
| `js_types.h` | The umbrella include |

Memory rules, inherited from the no-GC contract: deterministic destruction, `shared_ptr` only for genuinely shared ownership (`JsArray.elements` and `JsObject.properties` are the canonical examples), no hidden global allocators, no exceptions crossing generated-code boundaries unless the emitter generates the matching `try`/`catch`. The runtime headers are **C++17, header-only, self-contained** — morpher includes them directly into generated translation units, while generated file-morph output itself is C++23. Never use C++20/23 features in `runtime/cpp/`; the two standards are different rooms, don't track mud between them.

## The linter, bouncer of the costume party (`morpher/src/linter.rs`, `morph-parser/src/linter.rs`)

Not everything JS-flavored gets in. The linters reject unsupported syntax early with named diagnostics (`morph check` surfaces them) instead of letting the emitter produce confident garbage. When you extend coverage (a new builtin, a new method), update the diagnostics in the same PR — a feature the linter rejects is a feature that doesn't exist.

## Where to cut

| "I want to…" | Touch |
|---|---|
| Fix an annotation → C++ mapping | `type_resolver.rs` |
| Fix a comparison coercion | `js_comparison.rs` matrix cell + `21_js_comparison`-style fixtures |
| Add a string method | Mapping in `string_methods.rs` → helper in `js_string_helpers.h` → fixture case |
| Add an array method natively | Emitter path in `cpp.rs` → same fixture treatment |
| Add a global (`Math`, `node:*`) | Analyzer classification + emitter lowering + `morph check` diagnostics |
| Add a `Js*` capability | Wrapper header → `js_value.h` dispatch → `js_value_format.h` if it should print |

## Verify by

```bash
cargo test --workspace
python -m pytest tests/translate/ -v   # fixtures compiled with g++-14 -std=c++23, diffed vs Node
```

Behavioral identity with Node.js is the acceptance bar. If the C++ prints what `npx tsx` prints (within 0.001 for floats), the costume fits.
