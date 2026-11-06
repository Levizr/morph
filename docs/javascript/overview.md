# JavaScript / TypeScript Overview

Morph compiles component logic from TypeScript/JavaScript to C++ at build time. There is no interpreter — code runs as native machine code.

## How It Works

When you write:

```tsx
const [count, setCount] = morphState(0)
```

Morph's TS→C++ translator (the `morpher` crate) converts this to equivalent C++ using Oxc. The translated code uses Morph's native runtime types (`JsValue`, `JsString`, `Signal<T>`, etc.) and runs directly in the compiled binary.

In **dev mode**, the translated logic is compiled to a shared library (`logic.<hash>.so`) loaded via `dlopen`. Hot reload re-wires signals and effects in place without restarting the window.

In **build mode**, the translated logic is compiled directly into the production binary.

## Importing the Morph Module

Every `.mx` file starts by importing from `morph`:

```tsx
import { CSS, morphState, morphEffect } from 'morph'
```

This module is provided by `node_modules/morph/index.d.ts` — it ships with every `morph new` project and gives the editor autocomplete and type checking.

## Supported TypeScript Surface

### Declarations

- `const`, `let`, `var` with type annotations
- Late-assigned bindings: `let x;` and `let x = null/undefined` read
  `undefined` until the first assignment, then hold any JS value — number,
  string, boolean, object, array, or window handle — with real operator
  semantics and no type annotation. Late-assigned window handles keep
  resolving the current window on every use (see
  [Window handles](../api/windows.md#late-assigned-handles))
- `function` declarations and expressions
- Arrow functions (`() => {}`)
- Arrow/function expressions assigned to consts (inside components and at
  module scope)
- `async` / `await` (C++20 coroutines; see [Async & Networking](async.md))
- Destructured object/array parameters in function declarations, expressions,
  arrows, methods, function-prop callbacks, and `.on()` event handlers
  (lowered to one synthetic argument plus local bindings)
- Destructuring assignments (`[a, b] = ...`, `({ c } = o)`) with defaults,
  rest elements, computed keys, nested patterns, and member targets
- `for...of` declaration and assignment patterns, such as
  `for (const [key, value] of Object.entries(object))`
- Object/array literal consts, including nested object member access
- Classes with constructors, methods, `super()`, `this`, inferred or annotated fields, `static` and `#private` members, constructor parameter properties, getters/setters, and module-scope class expressions in direct `const`/`let` initializers
- Interfaces and type aliases
- Type-only imports and exports (`import type`, inline `type`, `export type`)

### Type Annotations

| TypeScript | C++ |
|---|---|---|
| `int`, `int32`, `int64` | `int32_t` / `int64_t` (stack, proven range) |
| `float`, `double` | `float` / `double` (stack) |
| `string` | `std::string` (stack) or `JsString` if dynamic |
| `boolean` | `bool` (stack) |
| `any` | `JsValue` |
| `MouseEvent` | `MorphEvent*` |
| `Element`, `HTMLElement` | `MorphNode*` |
| `Promise<T>` | `morph::Result<T>` |
| `Promise<void>` | `morph::Task` |

### Statements

`if`/`else`, `while`, `for`, `do-while`, `switch`/`case`/`default` (numeric; string `switch` is rejected), `try`/`catch`/`throw`, `return`, `break`, `continue`

Inside a component body, statements written between the declarations (branches, loops, calls) lower into a setup effect that re-runs when the state it reads changes — see [Locals Reassigned in the Component Body](./state.md#locals-reassigned-in-the-component-body).

### Expressions

Binary (`+`, `-`, `*`, `/`, `**`, `===`, `!==`, `==`, `!=`, `<`, `>`, etc.), unary (`!`, `-`, `++`, `--`), ternary (`? :`), template literals, array/object literals (including object spread `{ ...base }` and computed keys `{ [key]: value }`), `new`, member access, function calls, optional chaining / nullish coalescing, and logical assignments `&&=`, `||=`, `??=`. Object literals hold data values only — keep functions as separate consts (`{ handler: save }` is [mx-js-syntax](../errors/mx-js-syntax.md)).

### JS Runtime Semantics

- Truthiness (same rules as JS — `0`, `NaN`, `""`, `null`, `undefined`, `false` are falsy)
- Exponentiation (`**`, `**=`) coerces both sides with `ToNumber`; logical assignments (`&&=`, `||=`, `??=`) short-circuit and only test `null`/`undefined` for `??=`
- `==` / `!=` with JS coercion rules, `===` / `!==`, relational operators — on native and `Js*` types alike (see [How JavaScript Comparisons Work in Morph](./js-comparisons.md))
- String concatenation (`"" + x`)
- Late-assigned bindings (`let x; … x = ...`) keep JS value semantics
  through every operator: `+` concatenates when either side is a string,
  `-` / `*` / `/` / `%` coerce with `ToNumber`, and `++` / `--`, compound
  assignment, unary `+` / `-` / `~`, and bitwise/shift operators follow JS
  conversions instead of requiring a native numeric slot
- Initialized bindings widen when a later assignment changes value class
  (`let x = 0; x = "hi"`), including `number | string`, `any`, and
  `unknown` annotations; mixed numeric classes widen to `JsNumber`
- Class fields infer their type from initializers and `this.field` /
  `ClassName.field` writes; conflicting assignments widen the field to
  `JsLateValue`, and static/private members and accessors keep JS access
  rules
- Array `push`/`pop`/index access, plus `map`, `filter`, `forEach`, `reduce`, `find`, `findIndex`, `some`, `every`, `sort`, `splice`, `shift`, `unshift`, `fill`, `flat`, `flatMap` on arrays and `JsArray` (callbacks receive `(item)`, `(item, index)`, or `(item, index, array)`)
- Object `has`/`keys`/index access
- String methods on both `JsString` and native `std::string`: `trim`, `toUpperCase`, `toLowerCase`, `indexOf`, `substring`, `slice`, `replace`, `charAt`, plus `split`, `startsWith`, `endsWith`, `includes`, `repeat`, `padStart`, `padEnd` — natives lower to `morph::str::*` helpers, chains nest (see [Native C++ Types](./native-types.md#js-methods-on-natives))
- Number `.toString()` on native `int`/`double` (lowers to `morph::str::to_string`)
- `Math` constants (`PI`, `E`, `LN2`, `LN10`, `LOG2E`, `LOG10E`, `SQRT1_2`, `SQRT2`) and functions (`min`, `max`, `abs`, `floor`, `ceil`, `round`, `trunc`, `sign`, `pow`, `sqrt`, `cbrt`, `exp`, `expm1`, `log`, `log2`, `log10`, `log1p`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `hypot`, `fround`, `random`); arguments coerce through `Number()` (see [Runtime Types](./types.md#math))
- `Array.push` on both `JsArray` (`.push`) and `std::vector` (`.push_back`); multi-argument `push`, `unshift`, and `splice` accept spread elements. `.length` on vectors, arrays, and strings alike (`.size()` where native)
- Console output: `console.log` / `console.warn` / `console.error` / `console.info` print natively (visible in the DevTools **Logs** tab)
- Destructuring declarations (including at the top of a component body) and callable/event payloads, including defaults (`{ a = 1 }`), rest elements (`{ ...rest }`, `[head, ...tail]`), and computed keys (`{ [key]: value }`). Defaults are lazy and apply only when the value is `undefined`; computed keys evaluate once.
- Destructuring assignments and `for...of` binding patterns. The right-hand side is evaluated once, so `[a, b] = [b, a]` swaps correctly; defaults, rest, computed keys, nested patterns, and member targets (`[box.label] = ...`) all lower. `for...in` destructuring is rejected — bind the key with a plain name and destructure inside the loop body.

## What's Not Supported Yet

- Top-level rest parameters (`...args`) and general call/constructor spread
  (`f(...args)`, `new C(...args)`). Spread arguments are supported on
  `push`/`unshift`/`splice`/`concat` and `Math.min`/`Math.max`.
- Promise static helpers (`Promise.resolve`, `Promise.all`, …) and
  `.then`/`.catch` chains (rejected by `morph check`); `await` and
  `new Promise(...)` lower to C++ coroutines
- Regular expression literals, `Date`, `Map`/`Set`, and `typeof`/`instanceof`
- Object/Array statics such as `Object.assign`, `Object.fromEntries`,
  `Array.from`, and `Array.of`
- TypeScript `enum` declarations
- `import * as ns` namespace imports and `import.meta`
- Browser globals (`document`, `window`, `localStorage`, …)
- Class expressions outside a module-scope direct `const`/`let` initializer
  (module-scope initializers lower to a named class; function-local and
  every other position are rejected)
- String `switch`, array `for...in`, destructuring in `for...in`,
  `new.target`, and trailing timer arguments
  (`setTimeout(fn, ms, ...args)`) — rejected by `morph check` with a
  precise error
- C++ keywords used as JS binding or class member names (`double`,
  `template`, `delete`, …)
- A few JS methods whose receiver is provably a native type (for example
  array `.toSorted()`, `.reduceRight()`, and string `.at()`)

The linter (`morph check`) rejects the constructs listed above with a
precise error and exits non-zero. A clean `morph check` is the preflight
for `morph build`; the two commands share the same parser, graph, lint, and
IR pipeline.

> **Known contract gaps:** some constructs still pass `morph check` and
> fail in generated C++ — per-iteration closures, `var` hoisting/TDZ,
> imported module side effects, JSON dot access, JSX spread props,
> function-local class metadata that escapes its declaring function, and
> others. The
> [TypeScript App-Logic Coverage](../future/javascript/coverage.md) plan
> tracks every case and gates them behind Part 0.
>
> **Scope:** every JavaScript language and standard-library gap in that
> plan (Parts 1C–5G) is committed to implementation. Browser DOM and
> browser globals (`document`, `window`, `location`, `history`,
> `navigator`, event constructors) remain out of scope; use Morph's native
> equivalents such as `useWindow` instead.

## Rules `morph check` Enforces

Beyond the supported-syntax list, these structural rules are checked before
codegen. Errors block `morph build`; warnings are reported but do not:

- **One component entry point** — a UI module exports exactly one
  `export default function` (`mx-export`).
- **Images need a source** — every `<img>` has a non-empty `src`
  (`mx-img-src`).
- **Handlers are functions** — event props receive a function reference or
  arrow, not a call, string, or boolean (`mx-event-value`).
- **`key` only in lists** — `key` belongs on the root of a `.map()` item;
  outside a list it is dead markup (`mx-key-misuse`, warning).
- **One class source** — do not combine `class` and `className`
  (`mx-dup-class`, warning).
- **Real imports** — names imported from `'morph'` must be actual runtime
  exports imported under their canonical names, and module imports must
  target `.css`, `.cpp`, `'morph'`, or a source module
  (`mx-import-morph`, `mx-import-type`).
- **Literal window config** — `windowConfig` keys are known and values are
  literals of the documented type (`mx-windowconfig-key`,
  `mx-windowconfig-type`).
- **Effects receive no-argument functions** — the first `morphEffect`
  argument is a function, never a call result or a receiver-qualified method,
  and its callback declares no parameters (`mx-effect-cb`).
- **State is declared at component top level** — `morphState` inside a
  helper, handler, loop, conditional, or nested block is rejected
  (`mx-state-scope`).

Each rule links to its full explanation and fix in the
[error reference](../errors/index.md).

## Codegen Mode

| Aspect | Command | Behavior |
|---|---|---|
| **Default** | `morph file.ts` | Intent-based: escape analysis → stack/`unique_ptr`/`shared_ptr`, native types (`int32_t`, `std::string`, `std::vector`), type widening only when needed |
| **Types** | `morph file.ts --type strict` / `--type infer` | Respect annotations, or infer from code (default) |

Orthogonal to the mode, `--type` controls annotations:

| Flag | Behavior |
|---|---|
| `--type infer` (default) | Ignore annotations, infer the cheapest native type from initializer and usage (trusted number annotations excepted) |
| `--type strict` | Respect annotations (`number` stays `JsNumber`); unannotated variables are still inferred |

```bash
morph app.ts --to cpp --type infer   # native types + escape analysis
morph app.ts --to cpp --type strict  # annotations exactly as written
```

See [Intent-Based Codegen](../guides/intent-based-codegen.md) for the full memory management strategy, and [Native C++ Types](./native-types.md#how---type-picks-native-vs-wrapper) for how the two flags interact.