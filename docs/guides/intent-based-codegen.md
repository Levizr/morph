# Intent-Based Codegen & Memory Management (No GC)

Morph's `--optimize` flag enables **intent-based codegen with compile-time escape analysis** — generating minimal, optimal C++ that matches what a human expert would write, without a garbage collector.

## The Problem

Traditional JS→C++ translators emit `JsValue` (a `std::variant`) for everything, heap-allocate all objects, and use `shared_ptr` everywhere. This works but adds massive overhead:
- Binary size: ~500 KB for simple logic
- Compile time: ~1.5s (parsing `<format>` for every TU)
- Runtime: variant dispatch + heap allocation for every `int`, `string`, `vector`

## The Solution: Understand Intent, Emit Optimal C++

```
TypeScript Source
       │
       ▼
┌──────────────────┐
│  Oxc Parse       │  →  AST with type annotations
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Semantic Analyzer│  →  Annotated AST
│ - Escape analysis│     EscapeKind (None/Return/Global/Closure/...)
│ - Type widening  │     WidenedType (None/ToJsNumber/ToJsString/...)
│ - Async graph    │     UsageKind (Arithmetic/DynamicAssign/ToString/...)
│ - Closure detect │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ C++ Emitter      │  →  Optimized C++ (no templates unless needed)
│  - Native types  │     int32_t, std::string, std::vector on stack
│  - Smart pointers│     unique_ptr + move, shared_ptr only where required
│  - Coroutines    │     morph::Result<T>, morph::Task
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Runtime Linker   │  →  Minimal includes (only what's used)
│  - Feature flags │
└──────────────────┘
```

## Escape Analysis: The Decision Tree

For each variable, the analyzer determines **why** it escapes:

```
Does it escape the function?
│
├─► NO → Stack allocation (native type)
│       int x = 5;
│       std::string s = "hi";
│       std::vector<int> v = {1,2,3};
│
└─► YES → Why does it escape?
        │
        ├─► Returned (single owner moves out)
        │   → unique_ptr + std::move
        │   User createUser() { auto u = make_unique<User>(); return std::move(u); }
        │
        ├─► Stored in global/container (ownership transferred)
        │   → unique_ptr + std::move
        │   global.push_back(std::move(u));
        │
        ├─► Captured by closure (shared ownership)
        │   → shared_ptr
        │   auto c = make_shared<int>(0); return [c](){ return ++*c; };
        │
        ├─► Multiple simultaneous references (shared mutable)
        │   → shared_ptr
        │   let a = {x:1}; let b = a; b.x = 2; // both see change
        │
        └─► Crosses async boundary (await/co_return)
            → shared_ptr (coroutine frame may outlive caller)
```

### EscapeKind Hierarchy (Priority)

```
AsyncBoundary > ClosureCapture > MultipleRefs > Global > Return > None
```

Higher priority wins when a variable has multiple escape reasons.

## Smart Pointer Selection

| EscapeKind | C++ Type | Reason |
|---|---|---|
| `None` | `T` (stack) | Zero overhead, auto cleanup |
| `Return` / `Global` | `std::unique_ptr<T>` + `std::move` | Single owner, move semantics |
| `ClosureCapture` / `MultipleRefs` / `AsyncBoundary` | `std::shared_ptr<T>` | Shared ownership required |

**`shared_ptr` only where semantically required** — never "just in case."

## Type Widening: Static Annotation = Intent, Usage = Reality

```ts
let x: int = 5;
x = await fetchBigNumber();  // Could overflow int64, or be string
```

The analyzer widens the type based on **usage**, not just annotation:

| Annotation | Only Arithmetic | Assigned from Dynamic | String/Number Method Called |
|---|---|---|---|
| `int` / `int32` / `int64` | `int32_t` / `int64_t` | `JsNumber` | native + `morph::str::*` helper (`to_string`, `charAt`, …) |
| `float` / `double` | `float` / `double` | `JsNumber` | native + `morph::str::*` helper |
| `string` | `std::string` | `JsString` | `s` stays native, calls nest: `to_lower(to_upper(s))` |
| `number` | `JsNumber` | `JsNumber` | `x.as_string()` |
| `T[]` with `.push()` / `.length` | `std::vector<T>` | `JsArray` | method decides: `push_back` vs `push`, `.size()` for both |

### Widening Rules

- **Arithmetic only** → keep native (`int64_t`, `double`)
- **Dynamic assign** (`await`, `fetch`, `JSON.parse`, widened var) → `JsNumber`
- **String/number methods on native** → keep native, rewrite the call to a `morph::str::*` helper (`runtime/cpp/types/js_string_helpers.h`); chains nest so no intermediate boxes
- **Array methods (`.push()`, `.length`, iteration)** → `JsArray` when the usage needs JS semantics, else homogeneous `std::vector<T>` inferred from the elements (recursively, so `[[1,2]]` is `vector<vector<int32_t>>`)
- **Property access on unknown** → `JsValue`
- **`--type infer` (default) skips widening entirely** — natives are kept and helpers do the work; `--type strict` applies the table above

## JS Comparison Semantics: Native Types, JavaScript Answers

Native types are fast but answer comparisons differently than JavaScript (`"" == 0` is a compile error in C++, `true` in JS). So the analyzer records a `ComparisonSignature` for every comparison and truthiness test, and the emitter generates a `morph::js_cmp` helper block with exactly the sections that file uses:

```ts
let label: string = "";
let count: number = 0;
console.log(label == count);   // true — both are falsy
```

```cpp
std::println("{}", morph::js_cmp::loose_eq(label, count));
```

- Same-class pairs keep direct operators (`int64_t == int64_t` already matches JS).
- `===` across different types folds to `false` at compile time.
- The block is emitted inline in the translation unit — no extra header file, no unused helpers.
- Full coercion tables: [How JavaScript Comparisons Work in Morph](../javascript/js-comparisons.md).

## Intent Mapping: TS Pattern → C++ Strategy

| TS Pattern | Human Intent | C++ Translation (`--optimize`) |
|---|---|---|
| `let x: int = 5` | Native integer | `int64_t x = 5;` |
| `let x = 5` (only `+`, `-`, `*` used) | Native integer | `int32_t x = 5;` (inferred) |
| `let s = "hello"` | String value | `std::string s = "hello";` |
| `let a = [1,2,3]` + `for (x of a)` | Iterable sequence | `std::vector<int> a = {1,2,3};` |
| `let o = {a:1}` + `o.a` | Struct-like | `struct { int a; } o{1};` or `std::map` |
| `async function f() { await g() }` | Coroutine | `Task<Ret> f() { co_await g(); }` |
| `let r = await fetch()` | Async I/O | `auto r = co_await http_get(url);` |
| `fetch()` without await | Fire-and-forget | `morph::spawn_detached(http_post(url, data));` |
| `class C { method() {} }` | Polymorphic object | `class C { virtual void method(); }` + `shared_ptr<C>` |
| `interface I { x: number }` | Abstract contract | `class I { virtual int getX() = 0; }` |
| `Promise.all([...])` | Parallel wait | `when_all(vec_of_tasks)` |
| Top-level `await` | Program entry | `int main() { run_async([]{ ... }); }` |

## Edge Cases Handled

### 1. Shared Mutable Reference

```ts
let user = { name: "Alice" };
let admin = user;
admin.name = "Bob";
console.log(user.name); // "Bob"
```

**Detection**: Variable assigned to another + mutation through either
**Translation**: `shared_ptr<User>` for both

```cpp
auto user = std::make_shared<User>();
user->name = "Alice";
auto admin = user;  // shared_ptr copy, refcount=2
admin->name = "Bob";
std::println("{}", user->name); // "Bob"
```

### 2. Closure Capture

```ts
function makeCounter() {
    let count = 0;
    return () => ++count;
}
```

**Detection**: Variable used in nested function after parent returns
**Translation**: `shared_ptr<int>` captured by lambda

```cpp
auto makeCounter() {
    auto count = std::make_shared<int>(0);
    return [count]() mutable { return ++(*count); };
}
```

### 3. Async Boundary Crossing

```ts
async function fetchUser() {
    let user = await fetch("/user");  // user escapes to coroutine frame
    return user;
}
```

**Detection**: Variable live across `await` / `co_return`
**Translation**: `shared_ptr` (coroutine frame owns it)

```cpp
Task<User> fetchUser() {
    auto user = co_await http_get("/user"); // shared_ptr<User>
    co_return user;
}
```

### 4. Type Widening from Dynamic Source

```ts
let x: int = 5;
x = await fetchBigNumber();  // Could be 10^20 or "not a number"
```

**Detection**: Native-annotated variable assigned from `await` / dynamic call
**Translation**: Widen to `JsNumber` (handles int64, double, bigint, string)

```cpp
JsNumber x = 5;
x = co_await fetchBigNumber(); // JsNumber handles overflow/bigint
```

### 5. Native Type `.toString()` Call

```ts
let x: int = 42;
console.log(x.toString());
```

**Detection**: `.toString()` on native
**Translation**: Emit `morph::str::to_string(x)`

```cpp
int64_t x = 42;
std::println("{}", morph::str::to_string(x));
```

(`.toFixed()` / `.toPrecision()` are still unimplemented — on natives and wrappers alike.)

### 6. Chained String Calls Stay Native

```ts
let s: string = "hello world";
console.log(s.toUpperCase().toLowerCase());
console.log(n.toString().charAt(0).toUpperCase() + n.toString().slice(1));
```

**Detection**: a method call whose receiver is itself a method call (`CallExpression` as `StaticMemberExpression` object), plus computed receivers like `arr[0]` or `split(t, ",")[1]` — resolved recursively to the base variable's domain
**Translation**: nest the helpers so every step stays `std::string`:

```cpp
std::println("{}", morph::str::to_lower(morph::str::to_upper(s)));
std::println("{}", morph::str::to_upper(morph::str::char_at(morph::str::to_string(n), 0)) + morph::str::slice(morph::str::to_string(n), 1));
```

`JsString` / `JsValue` receivers keep their direct methods (`obj["name"].toUpperCase()` works because `JsValue` forwards them) — only native receivers go through helpers.

### 7. `new Promise<T>` Infers `morph::Result<T>`

```ts
let p2: Promise<number> = new Promise<number>((resolve) => { resolve(42); });
```

**Detection**: `NewExpression` with callee `Promise` (type argument read the same way `emit_new` reads it)
**Translation**: the variable infers `morph::Result<JsNumber>` even in `--type infer`, so the later `p2 = morph::Result<JsNumber>::resolved(42)` assignment type-checks; `Promise<void>` infers `morph::Task`.

### 8. Top-Level Execution Order

```ts
let p4: Promise<void> = voidPromise();  // logs "void" as a side effect
console.log(p1);
```

**Detection**: file-scope `static auto x = f();` with a call initializer, where `x` is never referenced inside any function/class body (fixpoint check over word-boundary references)
**Translation**: the declaration moves into `main()` as a local, in source order — `auto p4 = voidPromise();` runs exactly where JS would run it. Variables used by other functions stay at file scope (previous behavior).

### 9. Global Runtime Includes

```cpp
#include "/home/user/.morph/cache/runtimes/cpp/v0.1.0/types/js_types.h"
```

**Detection**: every `../../runtime/cpp/...` header collected during emission
**Translation**: rewritten to the absolute runtime path (`TranslateOptions.runtime_path`, auto-detected from the global cache or local `runtime/cpp`) so the file compiles from any directory. `js_value_format.h` (vector/optional formatters) is pulled in automatically when `std::vector` meets `println`/`format`, so `console.log([1,2,3])` prints `[ 1, 2, 3 ]` exactly like Node.

### 10. Global/Static Storage

```ts
const USERS: User[] = [];
function register(u: User) { USERS.push(u); }
```

**Detection**: Variable assigned to global/module-level container
**Translation**: Container owns `unique_ptr`, move into it

```cpp
std::vector<std::unique_ptr<User>> USERS;
void register(std::unique_ptr<User> u) { USERS.push_back(std::move(u)); }
```

### 11. Fire-and-Forget Async Call

```ts
fetch("/analytics", { method: "POST", body: data });
```

**Detection**: `await` not used on promise-returning call
**Translation**: Spawn detached task, no coroutine wrapper

```cpp
morph::spawn_detached(http_post("/analytics", data));
```

## Template Bloat Elimination

| Feature | Legacy | Optimized |
|---|---|---|
| Template literal `` `Hi ${x}` `` | `<format>` + `std::format` | `<format>` + `std::format` ✓ |
| `console.log("Hi", x)` | `<format>` + `std::format` | `<print>` + `std::println("Hi {}", x)` ✗ |
| `console.log(x)` | `<format>` + `std::format` | `<print>` + `std::println("{}", x)` ✗ |

**Rule**: `<format>` (costs ~1.5s/TU) only for `${}` template vars. `console.log` uses `std::println` directly.

## Header Minimization

The emitter tracks exactly what's used:

```cpp
// Analyzer tracks usage:
needs_vector    → #include <vector>
needs_string    → #include <string>
needs_coroutine → #include <coroutine>, "task.h"
needs_http      → #include "net.h"
needs_format    → #include <format>  // ONLY for template literals
```

No blanket `js_types.h` unless a `Js*` type is actually emitted.

## Performance Targets

| Metric | Legacy (Js* everywhere) | Optimized (`--optimize`) |
|---|---|---|
| Binary size (simple logic) | ~500 KB | **~150 KB** |
| Compile time (logic.ts) | ~1.5s | **~0.5s** |
| Runtime overhead (primitives) | Variant + heap | **Zero (stack)** |
| `int` arithmetic | `JsNumber` variant | Native `int64_t` |
| String concat | `JsString` heap | `std::string` SSO |
| Vector push | `JsArray` refcount | `std::vector` native |

## Usage

```bash
# Direct file morph with optimization
morph app.ts --to cpp --optimize

# Type mode is orthogonal: infer (default) or strict annotations
morph app.ts --to cpp --type infer --optimize
morph app.ts --to cpp --type strict

# In a project (add to morph.config.json build flags)
# Not yet exposed — currently only for direct file morph
```

## Current Status (Sept 2026)

| Feature | Status |
|---|---|
| Escape analysis (None/Return/Global/Closure/MultipleRefs/AsyncBoundary) | ✅ Built & integrated (`crates/morpher/src/codegen/analyzer.rs`) |
| Type widening (ToJsNumber/ToJsString/ToJsValue/ToJsArray) + chaining detection | ✅ |
| `--type infer` (default) / `--type strict` | ✅ (`TypeMode` in `context.rs`, `--type` CLI flag) |
| Native string methods via `morph::str::*` helpers, chains nest | ✅ (`string_methods.rs` + `js_string_helpers.h`) |
| JS comparison helpers (`morph::js_cmp`, only what's used) | ✅ |
| Native type emission (`int32_t`, `std::string`, `std::vector`, recursive literals) | ✅ |
| Smart pointer selection (`unique_ptr`/`shared_ptr`/`stack`) | ✅ |
| `Promise<T>` → `morph::Result<T>`, `Promise<void>` → `morph::Task`, `new Promise<T>` inference | ✅ |
| Sync `Result<T>` strip to `T` when no `co_await` | ✅ |
| Top-level `await` → async main wrapper; side-effectful `static auto` moved into `main` order-safely | ✅ |
| Global absolute runtime includes; auto `js_value_format.h` for printed vectors | ✅ |
| All 20 translate fixtures + 4 regression tests passing (outputs match Node.js) | ✅ |

## Implementation Files

| File | Role |
|---|---|
| `crates/morpher/src/codegen/analyzer.rs` | `EscapeAnalyzer`, `EscapeKind`, `WidenedType` (+`ToJsArray`), `UsageKind`, chaining detection, `AnalysisResult` |
| `crates/morpher/src/codegen/js_comparison.rs` | `OperandClass`, `ComparisonSignature`, `morph::js_cmp` header builder |
| `crates/morpher/src/codegen/string_methods.rs` | `StringMethod`, `StringMethodHandler` — JS→`morph::str::*` mapping |
| `crates/morpher/src/codegen/cpp.rs` | `emit_optimized_variable_declarator`, `infer_optimized_type`, `strip_result_for_sync_call`, `emit_new` (Promise→Result), `str_helper_decision`, recursive vector literals |
| `crates/morpher/src/codegen/type_resolver.rs` | Native type maps, `Promise`→`Result`, denormalization |
| `crates/morpher/src/codegen/context.rs` | `Ctx` with `var_types`, `async_fns`, `escape_hints`, `TypeMode`, `runtime_path` |
| `crates/morpher/src/lib.rs` | `TranslateOptions { optimize, type_mode, runtime_path, indent }` |
| `crates/morphc/src/commands/translate.rs` | `--optimize` / `--type` flags, `wrap_top_level_in_main`, safe `static auto` move |
| `runtime/cpp/types/js_string_helpers.h` | `namespace morph::str` — native string method equivalents |

## Future Work

1. **Per-assignment narrowing** — currently widens per-variable; could narrow after dynamic assign ends
2. **Move vs copy for large structs** — when JS object literal doesn't escape but is large
3. **Closure detection completeness** — verify all capture patterns in Oxc AST
4. **Default to `--optimize`** — after more real-world validation

## Related

- [Architecture Overview](../concepts/architecture.md) — full pipeline
- [JavaScript Overview](../javascript/overview.md) — TS surface
- [Native Types](../javascript/native-types.md) — `int`/`float`/`std_string` annotations
- [JS Comparisons](../javascript/js-comparisons.md) — coercion tables and helper design