# How JavaScript Comparisons Work in Morph

Morph compiles comparisons to native C++ that behaves like JavaScript — even when the variables involved are native types like `int64_t` or `std::string`. There are no wrapper types and no runtime type checks on the hot path: the analyzer reads what each comparison needs and generates exactly the helpers for it.

## The Problem

Intent-based codegen prefers native types (`bool`, `int64_t`, `std::string`) over `Js*` wrappers for speed. But native C++ comparison is not JavaScript comparison:

```tsx
let label: string = "";
let count: number = 0;
console.log(label == count);   // JavaScript prints: true (both are falsy)
```

A plain C++ `==` between `std::string` and `int64_t` does not compile — and where it does compile (`bool` vs `int`), it answers differently than JavaScript. The user wrote correct logic; the binary should honor it.

## The Solution: Analyze Intent, Generate Helpers

```
TypeScript Source
       │
       ▼
┌──────────────────┐
│ Semantic Analyzer│  →  ComparisonSignature for every ==, !=, ===, !==,
│                  │     <, >, <=, >=, if/while test, &&, ||, !, ternary
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ C++ Emitter      │  →  morph::js_cmp::loose_eq(a, b) only where C++ differs
│                  │     from JS; direct operators everywhere else
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│ Helper Block     │  →  morph::js_cmp namespace emitted inline in the TU —
│                  │     only the sections the file actually uses
└──────────────────┘
```

Same-class pairs keep direct operators (`int64_t == int64_t`, `std::string < std::string` already match JavaScript, including `NaN`). Everything else routes to a helper. Nothing is included or generated unless a call site needs it.

## Truthiness

`if`, `while`, `for` tests, ternary conditions, `!`, and the operands of `&&` / `||` go through `is_truthy`:

| Value | Truthy? |
|---|---|
| `false`, `0`, `0.0`, `NaN` | Falsy |
| `""` (empty string) | Falsy |
| `null`, `undefined` | Falsy |
| Empty array | Falsy |
| Everything else (non-empty strings, non-zero numbers, objects, functions) | Truthy |

```tsx
let label: string = "";
if (label) {
    console.log("never prints");
}
console.log(!label);   // true
```

One deliberate difference from JavaScript: an **empty array is falsy** in Morph (`[].length > 0` is false), while JavaScript treats it as truthy.

## Loose Equality (`==`, `!=`)

`loose_eq` follows JavaScript coercion:

| Expression | Result | Why |
|---|---|---|
| `"" == 0` | `true` | Both are falsy |
| `"42" == 42` | `true` | String coerces to number |
| `"  7  " == 7` | `true` | Surrounding whitespace is trimmed |
| `"0x10" == 16` | `true` | Hex, binary (`0b`), and octal (`0o`) parse |
| `"abc" == 1` | `false` | Unparseable text is `NaN` |
| `true == 1` | `true` | `true` coerces to `1` |
| `false == ""` | `true` | Both coerce to `0` |
| `null == undefined` | `true` | The only nullish match |
| `null == 0` | `false` | Nullish never coerces to a value |
| `[] == 0` | `true` | Array joins to `""`, then coerces |
| `[1] == 1` | `true` | Array joins to `"1"`, then coerces |
| `{} == "[object Object]"` | `true` | Objects stringify before comparing |

```tsx
let answer: number = 42;
console.log("42" == answer);   // true
```

compiles to:

```cpp
std::println("{}", morph::js_cmp::loose_eq("42", answer));
```

## Strict Equality (`===`, `!==`)

`strict_eq` answers at compile time when the C++ types already differ — `int64_t` vs `std::string` is `false` with no runtime work. Same-representation pairs compare directly; `5 === 5.0` is `true` because JavaScript has a single number type.

| Expression | Result |
|---|---|
| `"42" === 42` | `false` |
| `5 === 5.0` | `true` |
| `true === 1` | `false` |
| `null === undefined` | `false` |

## Relational Operators (`<`, `>`, `<=`, `>=`)

Both sides convert to numbers first — except two strings, which compare lexicographically, exactly like JavaScript:

| Expression | Result | Why |
|---|---|---|
| `"10" > 2` | `true` | Numeric coercion |
| `"10" > "9"` | `false` | Lexicographic, not numeric |
| `"a" < "b"` | `true` | Lexicographic |

## Logical Operators (`&&`, `||`)

Same-type operands keep their values with short-circuiting:

```tsx
console.log(userName && "fallback");   // the value of "fallback", or userName if falsy
console.log(tag || "untagged");        // tag if truthy, else "untagged"
```

compiles to:

```cpp
std::println("{}", (morph::js_cmp::is_truthy(userName) ? ("fallback") : (userName)));
```

Mixed-type operands (`1 && "x"`) combine to `bool` instead of returning the operand — JavaScript would return `"x"`. If the exact value matters there, write the ternary explicitly.

## Performance

| Operation | Runtime cost |
|---|---|
| `int == int`, `bool && bool`, same-string compare | **Zero** — direct C++ operator, helpers not even called |
| `===` across different types | **Zero** — folds to `false` at compile time |
| Truthiness of natives | **Zero** — inlines to a comparison |
| String-to-number coercion (`"42" == 42`) | One parse, only where the source compares text with numbers |

Compile time is the trade: the generated helpers use templates and `if constexpr` so every branch the program never takes disappears before codegen. Runtime pays nothing for that flexibility.

## Rules of Thumb

- Write comparisons the JavaScript way — the emitter preserves the semantics, whatever C++ types the analyzer picked underneath.
- Same-type comparisons cost nothing; cross-type comparisons with strings pay one parse.
- `Js*`-to-`Js*` comparisons of the same wrapper use the wrapper's own operators; mixed `Js*`/native pairs use the same helpers.
- If an `&&` / `||` chain mixes types and the resulting *value* (not just truthiness) is used downstream, prefer an explicit ternary.

## Related

- [Runtime Types](./types.md) — `JsValue`, `JsNumber`, `JsString`, `JsArray`, `JsObject`
- [Native C++ Types](./native-types.md) — when annotations stay native and what that gives up
- [Intent-Based Codegen](../guides/intent-based-codegen.md) — escape analysis, widening, and the full pipeline
