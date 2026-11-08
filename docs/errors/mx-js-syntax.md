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
- `namespace` blocks
- decorators
- TypeScript `enum` declarations
- regular expression literals (`/ab+c/`)
- rest parameters (`...args`)
- general call/constructor spread (`f(...args)`, `new C(...args)`; the
  variadic array helpers `push`/`unshift`/`splice`/`concat` and
  `Math.min`/`Math.max` are supported)
- `import * as ns` namespace imports
- C++ keywords used as binding names or class member names (`double`,
  `template`, `delete`, …)
- class expressions outside a module-scope direct `const`/`let` initializer
  (module-scope initializers lower to a named class; function-local and
  every other position are rejected)
- function-valued object properties (`{ handler: save }` — `JsObject`
  fields cannot store closures)
- `for...in` over an array (object `for...in` is supported)
- `for...in` with a destructuring binding (bind the key with a plain name,
  then destructure the value inside the loop body)
- Promise `.then` / `.catch` chains (use `async`/`await` and `try`/`catch`)
- `JSON.parse` reviver and `JSON.stringify` replacer/space arguments
- trailing timer arguments (`setTimeout(fn, ms, ...args)`)
- `new.target`

```
error : mx-js-syntax : `enum` declaration cannot be translated to C++
  hint:  Use a plain object const with string/number literals
  Learn more: https://morph.levizr.com/docs/errors/mx-js-syntax
```

These lower correctly and are **not** errors: optional chaining `?.`,
nullish coalescing `??`, object `for...in` / `for...of`, `switch` on
strings, numbers, and mixed case values (JavaScript `===` semantics with
fallthrough), array spread, object spread and computed keys, getters and setters,
plain-TS destructuring declarations, destructuring assignment, `for...of`
declaration and assignment patterns, destructuring at the top of a
component body, class declarations / `new`, module-scope class expressions
directly assigned to a `const`/`let`, type-only imports and exports,
default and optional parameters, arrow/function consts, data-only
object-literal consts, and JSX list expressions. `for...in` destructuring
is rejected; bind the key with a plain name and destructure inside the
loop body. Every JavaScript language and standard-library gap is committed
for implementation in the
[coverage plan](../future/javascript/coverage.md); browser globals remain
out of scope.

## Why Morph raises it

Each rejected construct needs hidden runtime machinery Morph omits on
purpose: generators need resumable frames; `with` needs dynamic scope;
labeled jumps have no C++ equivalent; `import.meta` / dynamic `import()`
need a module loader; namespaces
and decorators vanish in translation; enums and regex literals have no
lowering yet. `.then`/`.catch` needs a promise-chain runtime, and JSON/timer
extra arguments would be silently dropped. Supporting any of them
approximately would change program meaning — so each is rejected with an
explicit rewrite. See
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
// ❌ Closure stored in an object — mx-js-syntax
export default function App() {
  const save = () => console.log("saved");
  const handlers = { save };
  return <button onClick={handlers.save}>Save</button>;
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

```tsx
// ❌ Promise chain — mx-js-syntax (use async/await)
async function load() {
  fetch("https://api.example.com/data")
    .then((r) => r.json())
    .catch((err) => console.log(err));
}
```

## How to fix

| Instead of | Write |
|---|---|
| `enum Color { Red }` | `const Color = { Red: "Red" }` |
| `/ab+c/` | plain string checks or a `.cpp` helper |
| `import * as lib` | `import { token, greet } from "./lib.ts"` |
| `...args` | an explicit array parameter |
| `f(...args)` / `new C(...args)` | pass each argument explicitly, or use `push`/`unshift`/`splice`/`concat` and `Math.min`/`Math.max` for variadic arrays |
| `const C = class { … }` | a module-scope direct `const`/`let` initializer (supported), or a class declaration |
| `{ handler: fn }` | keep `fn` as its own const and call it directly |
| `for (const i in array)` | `for (let i = 0; i < array.length; i++)`, or `for...of` for values |
| `for (const [k] in object)` | `for (const k in object) { … }` — bind the key plainly; destructure the value inside the body |
| `fetch(...).then(...)` | `const r = await fetch(...)` inside `try`/`catch` |
| `JSON.parse(text, reviver)` | parse once, then transform the value |
| `JSON.stringify(value, replacer, 2)` | stringify the value, then format the result |
| `setTimeout(fn, ms, a, b)` | `setTimeout(() => fn(a, b), ms)` |
| `new.target` | an explicit constructor-identity check |
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
