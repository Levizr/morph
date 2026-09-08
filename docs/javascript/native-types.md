# Native C++ Types

Morph lets you annotate variables, parameters, and return values with real C++ primitive types directly in `.mx` logic. When you annotate with a native type, you get the native thing: direct, unboxed, no overhead.

## The Native Types

These annotations map straight to C++:

| Annotation | C++ |
|---|---|
| `int` | `int` |
| `int32` / `int64` | `int32_t` / `int64_t` |
| `uint` / `uint32` / `uint64` | `unsigned int` / `uint32_t` / `uint64_t` |
| `float` / `double` | `float` / `double` |
| `bool` | `bool` |
| `char` | `char` |
| `size_t` | `size_t` |
| `byte` | `uint8_t` |

Everything else maps to Morph's runtime wrappers under `--type strict`: `string` → `JsString`, `number` → `JsNumber`, `boolean` → `JsBoolean`, `any` → `JsValue`. Under the default `--type infer`, the compiler ignores those annotations and picks natives (`std::string`, `int32_t`, `bool`) wherever the usage allows it. See [Runtime Types](./types.md) and [How `--type` Picks Native vs Wrapper](#how---type-picks-native-vs-wrapper).

## What the Compiler Generates

```tsx
const a: int = 100;
let price: double = 99.5;
let x = 5;              // no annotation
```

compiles to:

```cpp
int a = 100;            // raw C++ int — unboxed
double price = 99.5;
JsNumber x = 5;         // runtime wrapper
```

Annotated values are plain C++ primitives. Unannotated values become runtime wrapper types that behave like JavaScript.

Function signatures work the same way:

```tsx
function jsxHelper(x: int): int {
  return x * 10 + 1
}
```

becomes a true native signature:

```cpp
int jsxHelper(int x)
```

## Why This Exists

**Zero-overhead arithmetic.** A `JsNumber` is a wrapper around a variant — safe, but not free. A raw `int` is a register. Hot loops (physics ticks, particle counts, pixel math) run at full native speed with no boxing or unboxing.

**Direct C++ interop.** Native functions callable from C++ (see [C++ / JSX Interop](../guides/native-cpp.md)) need real C++ signatures. Annotating `x: int` produces `int jsxHelper(int x)` — something native code can call without knowing anything about Morph's runtime types.

**Precision and memory control.** `int64` for IDs beyond 2^53, `float` over `double` when memory matters, `byte` for raw buffers. JavaScript's single `number` type can't express these choices; C++ annotations can.

## Trade-offs

A raw C++ primitive is **not a JavaScript value** — it has no methods and no wrapper. Two things are the exception: comparisons (`==`, `===`, `<`, truthiness tests, and `&&` / `||` all follow JavaScript rules on native types through generated `morph::js_cmp` helpers — see [How JavaScript Comparisons Work in Morph](./js-comparisons.md)), and JS *methods*, which the translator rewrites to native equivalents at the call site (see [JS Methods on Natives](#js-methods-on-natives)). Everything below is about mutability, not comparisons or methods.

### JS Methods on Natives

In JavaScript, every value has methods — `(100).toString()` just works. In C++, `int` has no member functions. Instead of rejecting the call or silently boxing the variable, the translator rewrites the call site to a native equivalent and leaves the variable untouched:

```tsx
const a: int = 100;
console.log(a.toString());
```

generates:

```cpp
int64_t a = 100;
std::println("{}", morph::str::to_string(a));
```

`a` stays a raw integer — only the call produces a temporary `std::string`. The equivalents live in `runtime/cpp/types/js_string_helpers.h` (`namespace morph::str`) and are reused everywhere; nothing is generated per call site:

| JavaScript | Native equivalent |
|---|---|
| `s.toUpperCase()` / `s.toLowerCase()` | `morph::str::to_upper(s)` / `to_lower(s)` |
| `s.trim()` / `trimStart()` / `trimEnd()` | `morph::str::trim(s)` / `trim_start` / `trim_end` |
| `s.charAt(i)`, `s.indexOf(x)`, `s.slice(a, b)` | `morph::str::char_at`, `index_of`, `slice`, `substring`, `substr` |
| `s.replace(a, b)`, `s.split(sep)` | `morph::str::replace`, `split` |
| `s.startsWith(x)`, `endsWith`, `includes` | `morph::str::starts_with`, `ends_with`, `includes` |
| `s.repeat(n)`, `padStart`, `padEnd` | `morph::str::repeat`, `pad_start`, `pad_end` |
| `n.toString()` (`int`, `double`, …) | `morph::str::to_string(n)` |

Chained calls nest the helpers, so the whole chain stays native:

```tsx
console.log(s.toUpperCase().toLowerCase());
console.log(n.toString().charAt(0).toUpperCase() + n.toString().slice(1));
```

```cpp
std::println("{}", morph::str::to_lower(morph::str::to_upper(s)));
std::println("{}", morph::str::to_upper(morph::str::char_at(morph::str::to_string(n), 0)) + morph::str::slice(morph::str::to_string(n), 1));
```

When a chain is detected, the analyzer keeps the base variable native on purpose — wrapping it in `JsString` would box every step. `JsString` variables keep their own direct methods (`s.toUpperCase()` where `s: JsString` is untouched), as do `JsValue` receivers (`obj["name"].toUpperCase()` works because `JsValue` forwards string methods).

Reassignment back into the same variable works the same way — both sides stay `std::string`:

```tsx
let a = "Hello, World";
a = a.toUpperCase();
console.log(a);   // HELLO, WORLD
```

```cpp
static std::string a = "Hello, World";
(a = morph::str::to_upper(a));
std::println("{}", a);
```

What works and what doesn't:

| Expression | `const a: int = 100` | `const a = 100` (inferred `int32_t`) |
|---|---|---|
| `a * 2`, `a + 1` | works | works |
| `"count: " + a` | works (string concat is overloaded) | works |
| `a.toString()` | works (`morph::str::to_string(a)`) | works |
| `a.toFixed(2)` | **compile error** | **compile error** (not implemented yet) |

`toFixed` / `toPrecision` / `toLocaleString` are still unimplemented on both native and wrapper types.

### The Linter Won't Catch Everything

`morph check` flags JS-only methods on state getters and string literals, but it does not track every native-typed variable. An unimplemented method like `a.toFixed(2)` on a `let a: int = 5` passes `morph check` cleanly — and then fails during C++ compilation. When mixing native annotations into logic, expect the error surface for *unimplemented* methods to move from the linter to g++.

### `const` Is Not `const`

JavaScript `const` only becomes C++ `const` for the wrapper types (`JsNumber`, `JsString`, ...). A `const a: int = 100` compiles to a **mutable** `int a = 100;` — the compiler trusts the intent for a native variable. (Comparisons are unaffected: they follow JavaScript rules on `const` and mutable natives alike.)

### State Signals Ignore Annotations

`morphState` picks its signal type from the initializer, not the annotation. `const [n, setN] = morphState(0)` gives `Signal<int>` no matter what is annotated — setters take the initializer's type.

## How `--type` Picks Native vs Wrapper

Direct file morphing has a `--type` flag that controls how annotations are treated:

```bash
morph app.ts --to cpp --type infer    # default: ignore annotations, infer the best native type
morph app.ts --to cpp --type strict   # respect annotations: `number` stays JsNumber, unannotated still inferred
```

- **`infer` (default)** — the analyzer looks at the initializer and how the variable is used (arithmetic, printing, method calls, data size) and picks the cheapest type that works. `let num: number = 42` used only in `console.log` becomes `int32_t`, not `JsNumber`.
- **`strict`** — an annotated `number`/`string`/`boolean`/`any` keeps its `Js*` wrapper exactly as written; only unannotated variables are inferred.

## Rules of Thumb

- Use native annotations (`int`, `double`) at the boundaries: function parameters and returns for C++ interop, counters and indices inside hot loops.
- Use plain JavaScript types (`number`, `string`) everywhere wrapper identity matters (stored in `JsArray`/`JsObject`, passed to generic `JsValue` APIs) — comparisons and most methods behave the same either way.
- Need a string from a native int? Call `.toString()` directly — it lowers to `morph::str::to_string(a)`.
- If a value flows into JSX markup or state, leave it unannotated and let it stay a runtime type.

## Resolved: JS Methods on Native Types

The open question from earlier revisions is settled: methods on natives compile by **translating the call to a native equivalent** — not by boxing the variable.

```tsx
let a: int = 100;
console.log(a.toString().toUpperCase());
```

compiles to:

```cpp
int64_t a = 100;
std::println("{}", morph::str::to_upper(morph::str::to_string(a)));
```

The variable stays a raw `int` — only the call site changes, and no `JsString` ever appears in the output. This answers the old option list:

- **Wrap automatically?** Rejected — it hid boxing; developers asked *"I used int — why is this a JsString?"*
- **Whitelist only `toString()`?** Rejected as arbitrary — the helper table above covers the whole common `String.prototype` surface instead.
- **Keep natives pure + explicit `String(a)`?** Unnecessary now — the translator inserts the equivalent itself.
- **Checker errors with fix hints?** Reserved for genuinely unimplemented methods (`toFixed`, `toPrecision`).

`JsString` appears only when the user writes it (`--type strict` with a `string` annotation) or when a value is genuinely dynamic (`any`, `fetch`, `JSON.parse`).