# Intent-Based Codegen & Memory Management Without GC

## Overview

This document outlines the strategy for translating JavaScript/TypeScript to optimized C++ using **intent-based codegen** with **compile-time escape analysis** — eliminating the need for a garbage collector while preserving JS reference semantics.

**Related**: [Working on morphc — The Rust Rewrite](../../docs/story/working-on-morphc.md) | [Full Rust Rewrite Plan](./morphc-rust-rewrite-plan.md) | [Architecture](./architecture.md)

---

## Current Status (Sept 2026)

### What Works (21/24 tests passing)

| Feature | Status |
|---------|--------|
| Native types (`int`, `int32`, `int64`, `float`, `double`, `bool`, `char`, `size_t`, `byte`) | ✅ Stack allocation |
| `std::string` / `std::vector<T>` / `std::optional<T>` | ✅ Native C++ types |
| `JsNumber`, `JsString`, `JsArray`, `JsObject`, `JsValue` | ✅ Heap with `shared_ptr` |
| Template literals → `std::format` | ✅ Conditional `<format>` include |
| `console.log` → `std::println` | ✅ Fast path for string/template |
| Async/await → C++20 coroutines (`morph::Task`, `morph::Result`) | ✅ |
| `fetch()` with `co_await` | ✅ |
| Top-level `await` → async `main()` wrapper | ✅ |
| `for...of` / `for...in` / loops | ✅ |
| **Semantic Analyzer** (escape analysis, type widening, async boundary, closure capture) | ✅ Built & integrated |
| **Optimized Emitter** (native types, unique_ptr, shared_ptr based on escape analysis) | ✅ Working |

### What's Broken (3 tests failing)

| Test | Issue |
|------|-------|
| `17_async.ts` | `Response` → `Result<JsString>` conversion in `co_return` |
| `18_promises.ts` | Same type mismatch in async chains |
| `19_fetch.ts` | `HttpAwaitable` not formattable; top-level `co_await` needs async main |

**Note**: The analyzer is now built and integrated in `crates/morph-js/src/codegen/analyzer.rs`. It runs during translation and produces `AnalysisResult` with escape kinds, widened types, and variable info. The optimized emitter (`--optimize` flag) uses escape analysis to emit:
- Stack allocation for non-escaping locals (`int`, `std::string`, `std::vector`)
- `unique_ptr` + `move` for single-owner escapes (returns, globals)
- `shared_ptr` for shared ownership (closures, multiple refs, async boundaries)
- Type widening when needed (`int` + dynamic source → `JsNumber`)

---

## Core Philosophy: Intent-Based Codegen

> **Translate like a human expert would** — understand semantic intent, generate minimal optimal C++, avoid template bloat.

### Pipeline

```
TypeScript Source
       │
       ▼
┌──────────────────┐
│  oxc Parse       │  →  AST with type annotations
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Semantic Analyzer│  →  Annotated AST (EscapeKind, WidenedType, AsyncBoundary)
│  - Escape analysis│
│  - Type widening │
│  - Async graph   │
│  - Ownership inf.│
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Intent Mapper    │  →  C++ IR (high-level, intent-based)
│  Pattern → Strategy
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ C++ Emitter      │  →  Optimized C++ (no templates unless needed)
│  - Native types  │
│  - Coroutines    │
│  - RAII          │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Runtime Linker   │  →  Minimal runtime includes
│  - Only used     │
└──────────────────┘
```

---

## Memory Management: Escape Analysis Strategy

### The Decision Tree

```
For each variable in function:
│
├─► Does it escape the function?
│   │
│   ├─► NO → Stack allocation (native type)
│   │       int x = 5;
│   │       std::string s = "hi";
│   │       std::vector<int> v = {1,2,3};
│   │
│   └─► YES → Why does it escape?
│       │
│       ├─► Returned (single owner moves out)
│       │   → unique_ptr + std::move
│       │   User createUser() { auto u = make_unique<User>(); return std::move(u); }
│       │
│       ├─► Stored in global/container (ownership transferred)
│       │   → unique_ptr + std::move
│       │   global.push_back(std::move(u));
│       │
│       ├─► Captured by closure (shared ownership)
│       │   → shared_ptr
│       │   auto c = make_shared<int>(0); return [c](){ return ++*c; };
│       │
│       ├─► Multiple simultaneous references (shared mutable)
│       │   → shared_ptr
│       │   let a = {x:1}; let b = a; b.x = 2; // both see change
│       │
│       └─► Crosses async boundary (await/co_return)
│           → shared_ptr (coroutine frame may outlive caller)
│
└─► Type widening needed? (int → bigint, unknown API)
    ├─► Static annotation `int` + only arithmetic → int64_t
    ├─► Assigned from dynamic source (fetch, JSON, unknown) → JsNumber
    └─► User calls .toString() on native → emit std::to_string()
```

### Smart Pointer Selection

| Escape Kind | Pointer Type | Reason |
|-------------|--------------|--------|
| `None` | Stack (value) | Zero overhead, auto cleanup |
| `Return` / `Global` | `unique_ptr` + `std::move` | Single owner, move semantics |
| `ClosureCapture` / `MultipleRefs` / `AsyncBoundary` | `shared_ptr` | Shared ownership required |

### `shared_ptr` Only Where Semantically Required

- **NOT** everywhere — only for: closures, shared mutable refs, async boundaries
- **NOT** for simple returns → `unique_ptr` + move
- **NOT** for stack-local objects → value semantics

---

## Type Widening Strategy

### Problem: User writes `int` but code assigns dynamic value

```ts
let x: int = 5;
x = await fetchBigNumber();  // Could overflow int64, or be string
```

### Solution: Static Annotation = Intent, Usage = Reality

| Annotation | Only Arithmetic | Assigned from Dynamic | `.toString()` Called |
|------------|-----------------|----------------------|---------------------|
| `int` | `int64_t` | `JsNumber` | `std::to_string(x)` |
| `int32` | `int32_t` | `JsNumber` | `std::to_string(x)` |
| `float` | `float` | `JsNumber` | `std::to_string(x)` |
| `string` | `std::string` | `JsString` | `s` (native) |
| `number` | `JsNumber` | `JsNumber` | `x.as_string()` |

### Widening Rules

```rust
// In SemanticAnalyzer
fn analyze_type_widening(&mut self, var: &str, usage: &Usage) {
    match usage {
        Usage::ArithmeticOp => {}, // Keep native
        Usage::DynamicAssign => self.widen_to_js_type(var),
        Usage::ToStringCall => self.emit_native_to_string(var),
        Usage::PropertyAccess => self.check_js_type_required(var),
    }
}
```

---

## Intent Mapping: TS Pattern → C++ Strategy

| TS Pattern | Human Intent | C++ Translation |
|------------|--------------|-----------------|
| `let x: int = 5` | Native integer | `int64_t x = 5;` |
| `let x = 5` (only `+`, `-`, `*` used) | Native integer | `int64_t x = 5;` (inferred) |
| `let s = "hello"` | String value | `std::string s = "hello";` |
| `let a = [1,2,3]` + `for (x of a)` | Iterable sequence | `std::vector<int> a = {1,2,3};` |
| `let o = {a:1}` + `o.a` | Struct-like | `struct { int a; } o{1};` or `std::map` |
| `async function f() { await g() }` | Coroutine | `Task<Ret> f() { co_await g(); }` |
| `let r = await fetch()` | Async I/O | `auto r = co_await http_get(url);` |
| `fetch()` without await | Fire-and-forget | `http_post(url);` (no coroutine) |
| `class C { method() {} }` | Polymorphic object | `class C { virtual void method(); }` + `shared_ptr<C>` |
| `interface I { x: number }` | Abstract contract | `class I { virtual int getX() = 0; }` |
| `Promise.all([...])` | Parallel wait | `when_all(vec_of_tasks)` |
| Top-level `await` | Program entry | `int main() { run_async([]{ ... }); }` |

---

## Edge Cases Handled

### 1. Shared Mutable Reference

```ts
let user = { name: "Alice" };
let admin = user;
admin.name = "Bob";
console.log(user.name); // "Bob"
```

**Detection**: Variable assigned to another variable + mutation through either
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

---

## Implementation Phases

### Phase 1: Semantic Analyzer (Non-Breaking, 2-3 weeks)

**New crate**: `crates/morph-js/src/analyzer.rs`

```rust
pub struct SemanticAnalyzer {
    escapes: HashMap<String, EscapeKind>,
    widens: HashMap<String, WidenedType>,
    async_boundaries: HashSet<String>,
    closure_captures: HashMap<String, Vec<String>>,
}

pub enum EscapeKind {
    None,              // Stack
    Return,            // unique_ptr + move
    Global,            // unique_ptr + move
    ClosureCapture,    // shared_ptr
    MultipleRefs,      // shared_ptr
    AsyncBoundary,     // shared_ptr
}

pub enum WidenedType {
    None,
    ToJsNumber,
    ToJsString,
    ToJsValue,
}
```

**Deliverables**:
- Escape analysis pass (returns `EscapeMap`)
- Type widening analysis
- Async boundary detection
- Closure capture detection
- Unit tests for each edge case

### Phase 2: Parallel Codegen Behind Flag (2 weeks)

**Modify**: `crates/morph-js/src/codegen/cpp.rs`

```rust
// Feature flag
if self.ctx.config.optimize {
    let hints = self.analyzer.get_hints();
    emit_optimized(hints);
} else {
    emit_current(); // Existing Js*-everywhere codegen
}
```

**Deliverables**:
- `--optimize` flag in `morph translate`
- Optimized emitter reading `EscapeKind` + `WidenedType`
- All 24 tests pass with `--optimize`
- Benchmark: measure binary size / runtime overhead reduction

### Phase 3: Fix Remaining Async Tests (1 week)

- `Response` → `Result<T>` conversion in `co_return`
- `HttpAwaitable` formatter
- Top-level await async main wrapper edge cases

### Phase 4: Swap Default (1 week)

- Make `--optimize` the default
- Remove legacy `Js*`-everywhere codegen paths
- Update documentation

---

## Runtime Impact

### Headers Only Included When Needed

```cpp
// Analyzer tracks usage:
// needs_vector → <vector>
// needs_string → <string>
// needs_coroutine → <coroutine>, "task.h"
// needs_http → "net.h"
// needs_format → <format> (only for template literals, NOT console.log)
```

### Minimal Runtime Types

```cpp
// runtime/cpp/types/native.h (new)
namespace morph {
    using String = std::string;
    template<typename T> using Vector = std::vector<T>;
    template<typename T> using Optional = std::optional<T>;
    template<typename T> using Unique = std::unique_ptr<T>;
    template<typename T> using Shared = std::shared_ptr<T>;
    
    // Only Js* types when semantics require
    // JsNumber: bigint, double, int64 union
    // JsString: JS string methods (toUpperCase, etc.)
    // JsArray: push/pop/index with shared storage
    // JsObject: map-backed dynamic properties
    // JsValue: variant for completely dynamic
}
```

---

## Performance Targets

| Metric | Current (Js* everywhere) | Target (Optimized) |
|--------|-------------------------|-------------------|
| Binary size (simple logic) | ~500 KB | ~150 KB |
| Compile time (logic.ts) | ~1.5s | ~0.5s |
| Runtime overhead (primitives) | Variant + heap | Zero (stack) |
| `int` arithmetic | `JsNumber` variant | Native `int64_t` |
| String concat | `JsString` heap | `std::string` SSO |
| Vector push | `JsArray` refcount | `std::vector` native |

---

## Open Questions

1. **Async boundary default**: Should `await` always imply `shared_ptr` for captured vars, or can we use `unique_ptr` if coroutine frame is sole owner?

2. **Type widening granularity**: Per-variable (current) vs per-assignment (narrowing after dynamic assign)?

3. **Closure detection reliability**: Current oxc AST — can we reliably detect `return () => x` captures in all cases?

4. **Move vs copy for structs**: When JS object literal doesn't escape but is "large", should we use `unique_ptr` anyway to avoid stack copy?

---

## Validation Checklist

- [ ] All 24 existing tests pass with `--optimize`
- [ ] New edge case tests added for each pattern above
- [ ] Binary size reduced ≥ 50% for logic-heavy code
- [ ] Compile time reduced ≥ 50%
- [ ] No regressions in `.mx` UI component compilation
- [ ] Documentation updated with migration guide

---

## Links

- [Working on morphc — Current Status](../../docs/story/working-on-morphc.md)
- [Full Rust Rewrite Plan](./morphc-rust-rewrite-plan.md)
- [Architecture Overview](./architecture.md)