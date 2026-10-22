# mx-js-syntax — Unhandleable Syntax Construct

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** reported by `morph check`. The gate exits non-zero, so a
> clean `morph check` means no rejected syntax remains. Only probe-verified
> breakage is rejected — common JS spellings that lower correctly stay
> allowed.

## What this error means

Your code uses a syntax construct the translator cannot represent in C++.
The current set is:

- generators / `yield`
- the `with` statement
- labeled `break` / `continue`
- `import.meta` and dynamic `import()`
- object spread
- `namespace` blocks
- decorators
- TypeScript `enum` declarations
- regular expression literals (`/ab+c/`)
- rest parameters (`...args`)
- `import * as ns` namespace imports
- C++ keywords used as binding names or class member names (`double`,
  `template`, `delete`, …)
- class expressions assigned to a const
- destructuring at the top of a component body (except the
  `const [value, setValue] = morphState(...)` pair)

```
error : mx-js-syntax : `enum` declaration cannot be translated to C++
  hint:  Use a plain object const with string/number literals
  Learn more: https://morph.levizr.com/docs/errors/mx-js-syntax
```

These lower correctly and are **not** errors: optional chaining `?.`,
nullish coalescing `??`, `for...in` / `for...of`, array spread, getters and
setters, plain-TS destructuring and loop-head destructuring, class
declarations / `new`, type-only imports and exports, default and optional
parameters, arrow/function consts, object-literal consts, and JSX list
expressions.

## Why Morph raises it

Each rejected construct needs hidden runtime machinery Morph omits on
purpose: generators need resumable frames; `with` needs dynamic scope;
labeled jumps have no C++ equivalent; `import.meta` / dynamic `import()`
need a module loader; object spread needs reflective copies; namespaces
and decorators vanish in translation; enums and regex literals have no
lowering yet. Supporting any of them approximately would change
program meaning — so each is rejected with an explicit rewrite. See
[translator coverage](../future/javascript/coverage.md) for what is planned.

## Examples that trigger it

```tsx
// ❌ TypeScript enum — mx-js-syntax
export default function App() {
  enum Color { Red, Green }
  return <div>{Color.Red}</div>;
}
```

```tsx
// ❌ Object spread — mx-js-syntax
export default function App() {
  const base = { a: 1 };
  const ext = { ...base, b: 2 };
  return <div>Hello</div>;
}
```

```tsx
// ❌ Namespace import — mx-js-syntax
import * as lib from "./lib.ts";

export default function App() {
  return <div>{lib.token}</div>;
}
```

```tsx
// ❌ C++ keyword as a binding name — mx-js-syntax
export default function App() {
  const double = 2;
  return <div>{double}</div>;
}
```

## How to fix

| Instead of | Write |
|---|---|
| `enum Color { Red }` | `const Color = { Red: "Red" }` |
| `/ab+c/` | plain string checks or a `.cpp` helper |
| `import * as lib` | `import { token, greet } from "./lib.ts"` |
| `...args` | an explicit array parameter |
| `const C = class { … }` | a class declaration |
| `{...o, k: v}` | an explicit object literal |
| destructuring in a component body | one const per binding (`const [v, setV]` state pairs are fine) |
| generator / `yield` | a state machine with `morphState` |
| `with (o) { ... }` | explicit `o.` qualification |
| labeled `break`/`continue` | a flag variable or early return |
| `import.meta` / `import()` | static imports and constants |
| `namespace N { ... }` | exported functions and constants |
| a C++ keyword binding or member (`double`, `template`, `delete`, …) | rename the binding/field/method |
| decorators | inline the decorator's effect |

```tsx
// ✅ Object const instead of an enum
const Color = { Red: "red", Green: "green" };

export default function App() {
  return <div>{Color.Red}</div>;
}
```

Steps:

1. Match the flagged construct to the table rewrite.
2. For object-heavy or string-heavy logic, consider moving it to `.cpp`
   (see [native C++](../guides/native-cpp.md)).
3. Re-run `morph check`; only a clean check should be followed by a build.

## Tuning this rule

Do not disable this rule. These constructs have no native representation.

## See also

- [mx-js-op](mx-js-op.md) — unsupported operators
- [mx-js-global](mx-js-global.md) — browser globals and timers
- [mx-transpile](mx-transpile.md) — the general rule
- [How Morph Compiles JavaScript](../javascript/overview.md)
