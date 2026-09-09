# Intent-Based Codegen & Memory Management (No GC)

Morph's `--optimize` flag enables **intent-based codegen with compile-time escape analysis** — generating minimal, optimal C++ that matches what a human expert would write, without a garbage collector.

## The Core Idea: Intent, Not Syntax

Most translators map syntax to syntax: `let x = 5` becomes `JsValue x = 5` because *it might be anything*. Morpher instead asks three questions about every variable and lets the answers pick the C++:

1. **Declared intent** — what did the programmer write? (`let x: int`, `const s: string`)
2. **Observed intent** — what does the code actually do with it? (arithmetic only? passed to `fetch`? captured by a closure? `_` recorded as `UsageKind` per use)
3. **Lifetime intent** — who owns it and how long does it live? (stack-local? returned? shared across an `await`? — recorded as `EscapeKind`)

Declared intent and observed intent are reconciled by **widening** (usage wins over annotation when they disagree), and lifetime intent picks the **storage** (stack, `unique_ptr`, or `shared_ptr`). When all three agree the value is a plain local integer, you get `int32_t x = 5;` — zero overhead, freed automatically. Nothing is boxed "just in case."

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

## The Pipeline in Full

One function runs the whole analysis — `EscapeAnalyzer::analyze_program` (`crates/morpher/src/codegen/analyzer.rs:116`) — in six phases, single pass plus fixpoints. No separate borrow checker, no IR round-trips; the Oxc AST is walked directly:

1. **`collect_signatures`** — every top-level function is recorded (`FunctionSignature`: name, async?, params, return type) and every file-scope variable is pre-marked `EscapeKind::Global`. Async functions and async arrow/function expressions assigned to variables join the `async_functions` set.
2. **`analyze_statement` (recursive walk)** — each statement is visited: declarations create a `VarInfo`; `return x` marks `Return`; `await x` marks `AsyncBoundary`; `x = y` (identifier to identifier) marks **both** sides `MultipleRefs`; method calls and `.length` accesses record widening (`ToJsString` / `ToJsArray`); every comparison and every truthiness test (`if`/`while`/`for` conditions, `&&`/`||`, `!`, ternaries) records a `ComparisonSignature` for the `js_cmp` emitter.
3. **`detect_chaining`** — a second walk finds `s.toUpperCase().toLowerCase()` shapes (a member access whose object is itself a call) so chained receivers resolve to the base variable's domain instead of degrading to boxed calls.
4. **`resolve_cross_function_escapes`** — fixpoint over function signatures: parameters of async functions that are awaited or co-returned inside get `AsyncBoundary`, since the coroutine frame will own them.
5. **`resolve_identifier_init_classes`** — fixpoint over `let alias = target` chains (forward references included): an alias inherits its target's operand class, so `let b = a; b + 1` stays arithmetic instead of falling back to `JsValue`.
6. **`AnalysisResult`** — the three maps (`escapes`, `widens`, `var_infos`) plus `async_functions` and `comparison_signatures` are handed to the emitter, which makes every declaration decision from them (`emit_optimized_variable_declarator`, `crates/morpher/src/codegen/cpp.rs:390`).

### What the Analyzer Records for Every Variable

```rust
pub struct VarInfo {
    pub name: String,
    pub annotated_type: Option<String>,  // declared intent: `: int`, `: string`, ...
    pub escape_kind: EscapeKind,         // lifetime intent (max wins, see below)
    pub widened_type: WidenedType,       // observed intent: None/ToJsNumber/ToJsString/ToJsValue/ToJsArray
    pub is_mutable: bool,                // let vs const
    pub usages: Vec<UsageKind>,          // every observed use (28 kinds: ArithmeticOp,
                                         // MethodCall, Awaited, Iterated, Spread, ...)
    pub init_operand_class: OperandClass, // what the initializer's C++ value is
                                         // (Integer/Float/Text/Boolean/JsValue/...)
}
```

`mark_escape` never downgrades: `escapes.insert(name, EscapeKind::max(current, kind))` (`analyzer.rs:1178`), with priority `AsyncBoundary > ClosureCapture > MultipleRefs > Global > Return > None` (`analyzer.rs:1232`). A variable that is both returned *and* captured gets `shared_ptr` — the stricter need always wins.

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

## Memory Management in Detail (No GC)

There is no garbage collector in emitted code — not a tracing one, not a reference-count-everything one. Memory is managed by a compile-time ownership decision per variable, executed by C++ RAII (destructors run deterministically at scope exit). This section is the full story: the three storages, what each costs, why sharing still needs `shared_ptr`, and where the `Js*` wrappers fit.

### The three storages

Every declaration lands in exactly one bucket (`cpp.rs:453`):

| Storage | When | Allocation | Cleanup | Cost |
|---|---|---|---|---|
| **Stack value** (`int32_t x = 5;`) | `EscapeKind::None` — provably local | None (register/stack slot) | Automatic at scope exit | **Zero** |
| **`unique_ptr<T>` + `std::move`** | `Return` / `Global` — single owner moves out | One heap allocation (`make_unique`) | Freed when the owner dies | One allocation, no counting |
| **`shared_ptr<T>`** (`make_shared`) | `ClosureCapture` / `MultipleRefs` / `AsyncBoundary` — genuinely shared | One heap allocation (`make_shared` merges object + control block) | Freed when the last owner dies | Atomic refcount inc/dec per copy |

`const` on a `Js*` scalar folds to `const JsNumber` etc., and file-scope declarations get a `static` prefix (`cpp.rs:513`) so they live for the program's duration instead of per-call.

### Why no collector at all

A GC exists to answer "is this still reachable?" at runtime. Morpher answers it at compile time instead: the escape walk proves, per variable, whether the value can outlive its scope and whether it has one owner or many. When the proof says "local integer, never leaves," there is nothing left for a collector to do — generating a traced or counted box around it would be pure overhead (allocation + write barriers + collection pauses) protecting against a situation the analyzer already ruled out.

The honest corollary: the proof is conservative. Anything the analyzer can't see through ( genuinely dynamic values — `fetch`, `JSON.parse`, unannotated cross-module data) widens to a `Js*` wrapper, and those wrappers *do* share internally (next section). Safety is never sacrificed for speed; speed comes only from cases proven safe.

### Why `shared_ptr` is still needed (and what it costs)

Three JS semantics genuinely require shared ownership — there is no cheaper correct answer:

- **Aliasing** (`let b = a; b.x = 2` must be visible through `a`). The analyzer marks both sides `MultipleRefs` (`analyzer.rs:788`), and both names share one `shared_ptr` — one object, two owners, exactly like the JS heap.
- **Closures** (`return () => ++count`). The lambda outlives the stack frame that created `count`, so the counter moves to the heap and both the (dead) frame's successor and the lambda hold a `shared_ptr` (`ClosureCapture`).
- **Coroutines** (anything live across `await`/`co_return`). A suspended coroutine's frame survives the caller's return, so locals it keeps must be heap-owned (`AsyncBoundary`).

The cost is real and worth naming: each `shared_ptr` copy is an atomic increment, each destroy an atomic decrement, plus the control block allocation (which `make_shared` fuses with the object into one allocation — the emitter always uses `make_shared`, never bare `new`). Atomics are cheap next to a heap allocation but not free next to a stack slot — which is precisely why they're emitted only for the three cases above.

### Where the `Js*` wrappers fit

When widening fires, the variable becomes a `Js*` type — and those are *not* bare values:

- **`JsValue`** is a `std::variant` of 8 alternatives (`JsUndefined`, `JsNull`, `JsBoolean`, `JsNumber`, `JsString`, `JsArray`, `JsObject`, `JsFunction` — `runtime/cpp/types/js_value.h:32`). It costs `sizeof(largest alternative)` per value plus a tag check (dispatch) on every operation. Correct for anything, free for nothing.
- **`JsArray` / `JsObject` share internally**: `elements` is a `shared_ptr<vector<JsValue>>` ("shared_ptr for JS-like reference semantics (no deep copy on assignment)" — `js_array.h:11`), and properties likewise. Assigning one `JsArray` to another copies the pointer, not the data — JS aliasing semantics preserved, at one atomic count per copy.

So the performance story is really a *widening-avoidance* story: every variable that stays native skips the variant size, the dispatch, and the refcounting. `--type infer` exists to maximize exactly that set.

### Coroutine memory: `Task`, `Result<T>`, and the sync strip

Async functions become coroutines returning `morph::Task` (no value) or `morph::Result<T>` (a value) — the coroutine *frame* (locals, suspend state) is heap-allocated by the C++ runtime and freed when the coroutine completes. That's why `AsyncBoundary` forces `shared_ptr`: a value the frame keeps must outlive the caller that created it.

Two refinements keep this cheap:

- **`new Promise<T>` infers `morph::Result<T>`** from the type argument at the declaration (`infer_optimized_type`, `cpp.rs:646`), so `let p: Promise<number> = new Promise<number>(...)` never touches `JsValue`.
- **Sync strip**: if a variable's initializer is a plain synchronous call but its declared type is `Result<T>`, the emitter strips it to `T` (`strip_result_for_sync_call`, `cpp.rs:1337`) — no coroutine frame, no wrapper, just the value.

### Strings and vectors: native storage, JS behavior

- **Strings** live in `std::string` (small-string optimization: short strings never touch the heap) while JS methods are served by `morph::str::*` helpers that take and return `std::string` — chains nest (`to_lower(to_upper(s))`), so no intermediate `JsString` box is ever allocated. Only genuinely dynamic strings become heap-owning `JsString`.
- **Vectors** are inferred recursively from literals: `[1,2,3]` → `std::vector<int32_t>`, `[[1,2]]` → `std::vector<std::vector<int32_t>>`, mixed or empty → `std::vector<JsValue>` fallback (`infer_optimized_type`, `cpp.rs:678`). Homogeneous data gets contiguous native storage and cache-friendly iteration; only heterogeneous data pays for the variant per element.

### Who frees file-scope and global state?

File-scope variables start as `EscapeKind::Global` and emit as `static` (`unique_ptr` for objects). They live until program exit — matching JS module-scope semantics, where top-level bindings never die. Side-effectful initializers (`static auto x = f();`) are additionally moved into `main()` in source order so they run exactly when JS would run them (edge case 8 above) — lifetime and *execution order* are both preserved.

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

### Strict vs Infer: Who Decides the Type?

The two modes differ in exactly one place — the `base_type` computation at the top of `emit_optimized_variable_declarator` (`cpp.rs:401`):

```
--type strict                          --type infer (default)
    │                                          │
    ▼                                          ▼
Has annotation? ──yes──► use it        Always infer from the
    │                      │           initializer, ignore the
    no                     │           annotation entirely
    │                      │
    ▼                      ▼
infer from init ◄──┴──► (same inference)
    │                      │
    ▼                      ▼
Apply widening table       Skip widening (natives stay native,
above                      helpers serve the methods)
```

Consequences: in strict mode `let x: number = 5` is `JsNumber` (you asked for the general type, you get it); in infer mode it's `int32_t` (the initializer is all the evidence there is). Parameters follow the same split — strict classifies them from their annotations, infer treats them as `JsValue` until use proves otherwise (`analyzer.rs:481`). Widening from *dynamic sources* (`await`, `fetch`, `JSON.parse`) still fires in both modes: reality beats declarations everywhere.

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