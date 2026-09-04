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

Everything else maps to Morph's runtime wrappers: `string` → `JsString`, `number` → `JsNumber`, `boolean` → `JsBoolean`, `any` → `JsValue`. See [Runtime Types](./types.md).

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

A raw C++ primitive is **not a JavaScript value** — it has no methods, no coercion, no wrapper.

### No JS Methods

In JavaScript, every value has methods — `(100).toString()` just works. In C++, `int` has no member functions. The translator emits method calls verbatim, so this:

```tsx
const a: int = 100;
a.toString();
```

generates:

```cpp
int a = 100;
a.toString();   // error: request for member 'toString' in 'a',
                // which is of non-class type 'int'
```

and the C++ compiler rejects it. Only the runtime wrappers carry JS methods — `JsNumber::toString()` exists, but there is no such thing on `int`.

What works and what doesn't:

| Expression | `const a: int = 100` | `const a = 100` (JsNumber) |
|---|---|---|
| `a * 2`, `a + 1` | works | works |
| `"count: " + a` | works (string concat is overloaded) | works |
| `a.toString()` | **compile error** | works |
| `a.toFixed(2)` | **compile error** | not implemented yet |

### The Linter Won't Catch It

`morphc check` flags JS-only methods on state getters and string literals, but it does not track variables annotated with native types. `a.toFixed(2)` on a `let a: int = 5` passes `morphc check` cleanly — and then fails during C++ compilation. When mixing native annotations into logic, expect the error surface to move from the linter to g++.

### `const` Is Not `const`

JavaScript `const` only becomes C++ `const` for the wrapper types (`JsNumber`, `JsString`, ...). A `const a: int = 100` compiles to a **mutable** `int a = 100;` — the compiler trusts the intent for a native variable, and native variables don't inherit JS semantics.

### State Signals Ignore Annotations

`morphState` picks its signal type from the initializer, not the annotation. `const [n, setN] = morphState(0)` gives `Signal<int>` no matter what is annotated — setters take the initializer's type.

## Rules of Thumb

- Use native annotations (`int`, `double`) at the boundaries: function parameters and returns for C++ interop, counters and indices inside hot loops.
- Use plain JavaScript types (`number`, `string`) everywhere JS behavior is wanted — methods, coercion, JSX text interpolation.
- Need a string from a native int? Concatenate (`"count: " + a`) or keep the value as `number` and call `.toString()`.
- If a value flows into JSX markup or state, leave it unannotated and let it stay a runtime type.

## Open Question: JS Methods on Native Types

Currently, methods on native primitives fail to compile (see [No JS Methods](#no-js-methods)). One possible fix: let JS methods work on native types by wrapping the call site automatically.

```tsx
let a: int = 100;
console.log(a.toString().toUpperCase());
```

would compile to:

```cpp
int a = 100;
std::println("{}", JsString(a).toUpperCase());   // wrapped only at the call site
```

The variable stays a raw `int` — only the method call produces a temporary wrapper.

**The problem:** implicit wrapping can mislead. A developer who wrote `: int` sees `JsString` appear in the compiled output and asks *"I used int — why is this a JsString?"* Silent conversions invite mistakes. Adding a lint warning on every wrapped call isn't great either — warnings that fire on normal code just train people to ignore warnings.

So the design isn't settled:

- **Wrap automatically?** Convenient, but hides where boxing happens.
- **Whitelist only `toString()` / `toFixed(n)`?** Predictable, but arbitrary.
- **Keep natives pure + explicit `String(a)`?** Honest, but noisy at interop boundaries.
- **Checker errors with fix hints, no sugar?** Safest, worst ergonomics.

**We want feedback.** If native annotations are used in Morph — or would be, if methods worked on them — what behavior is expected? Real usage decides this.

- **Email:** [`suggestions.morph@levizr.com`](mailto:suggestions.morph@levizr.com)
- **GitHub:** [open an issue](https://github.com/Levizr/morph/issues) on `Levizr/morph`