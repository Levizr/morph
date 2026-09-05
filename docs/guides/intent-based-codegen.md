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

| Annotation | Only Arithmetic | Assigned from Dynamic | `.toString()` Called |
|---|---|---|---|
| `int` / `int32` / `int64` | `int32_t` / `int64_t` | `JsNumber` | `std::to_string(x)` |
| `float` / `double` | `float` / `double` | `JsNumber` | `std::to_string(x)` |
| `string` | `std::string` | `JsString` | `s` (native) |
| `number` | `JsNumber` | `JsNumber` | `x.as_string()` |

### Widening Rules

- **Arithmetic only** → keep native (`int64_t`, `double`)
- **Dynamic assign** (`await`, `fetch`, `JSON.parse`, widened var) → `JsNumber`
- **`.toString()` / `.toFixed()` / `.toPrecision()`** on native → emit `std::to_string()`
- **Property access on unknown** → `JsValue`

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

**Detection**: `.toString()` / `.toFixed()` / `.toPrecision()` on native
**Translation**: Emit `std::to_string(x)` or `std::format`

```cpp
int64_t x = 42;
std::println("{}", std::to_string(x));
```

### 6. Global/Static Storage

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

### 7. Fire-and-Forget Async Call

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

# In a project (add to morph.config.json build flags)
# Not yet exposed — currently only for direct file morph
```

## Current Status (Sept 2026)

| Feature | Status |
|---|---|
| Escape analysis (None/Return/Global/Closure/MultipleRefs/AsyncBoundary) | ✅ Built & integrated (`crates/morph-js/src/codegen/analyzer.rs`) |
| Type widening (ToJsNumber/ToJsString/ToJsValue) | ✅ |
| Native type emission (`int32_t`, `std::string`, `std::vector`) | ✅ |
| Smart pointer selection (`unique_ptr`/`shared_ptr`/`stack`) | ✅ |
| `Promise<T>` → `morph::Result<T>`, `Promise<void>` → `morph::Task` | ✅ |
| Sync `Result<T>` strip to `T` when no `co_await` | ✅ |
| Top-level `await` → async main wrapper | ✅ |
| All 24 translate tests passing | ✅ |

## Implementation Files

| File | Role |
|---|---|
| `crates/morph-js/src/codegen/analyzer.rs` | `EscapeAnalyzer`, `EscapeKind`, `WidenedType`, `UsageKind`, `AnalysisResult` |
| `crates/morph-js/src/codegen/cpp.rs` | `emit_optimized_variable_declarator`, `infer_optimized_type`, `strip_result_for_sync_call`, `emit_new` (Promise→Result) |
| `crates/morph-js/src/codegen/type_resolver.rs` | Native type maps, `Promise`→`Result`, denormalization |
| `crates/morph-js/src/codegen/context.rs` | `Ctx` with `var_types`, `async_fns`, `escape_hints` |
| `crates/morphc/src/commands/translate.rs` | `--optimize` flag, `wrap_top_level_in_main` |

## Future Work

1. **Per-assignment narrowing** — currently widens per-variable; could narrow after dynamic assign ends
2. **Move vs copy for large structs** — when JS object literal doesn't escape but is large
3. **Closure detection completeness** — verify all capture patterns in Oxc AST
4. **Default to `--optimize`** — after more real-world validation

## Related

- [Architecture Overview](../concepts/architecture.md) — full pipeline
- [JavaScript Overview](../javascript/overview.md) — TS surface
- [Native Types](../javascript/native-types.md) — `int`/`float`/`std_string` annotations