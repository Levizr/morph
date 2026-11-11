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
- `for...in` with a destructuring binding (bind the key with a plain name,
  then destructure the value inside the loop body)
- Promise `.then` / `.catch` chains outside the supported statement-level
  `fetch(...).then(...).catch(...)` form inside an async function (one
  `.then` and/or one `.catch`, non-async callback with at most one plain
  parameter; use `async`/`await` and `try`/`catch` otherwise)
- `JSON.parse` reviver and `JSON.stringify` replacer/space arguments
- trailing timer arguments on identifier, function-expression, or
  rest-parameter callbacks, or on arrow callbacks that declare more
  parameters than supplied arguments (use an inline arrow callback with no
  more parameters than trailing arguments; extra arguments are then
  evaluated and ignored)
- `let`/`const` reads before the declaration line (the temporal dead zone),
  including self-referential initializers such as `let x = x`
- destructured `var` declarations (`var { a } = obj`) and late `var`
  initializers that cannot start as `undefined` (arrow, function, class,
  or `new` expressions)

```
error : mx-js-syntax : `enum` declaration cannot be translated to C++
  hint:  Use a plain object const with string/number literals
  Learn more: https://morph.levizr.com/docs/errors/mx-js-syntax
```

These lower correctly and are **not** errors: optional chaining `?.`,
nullish coalescing `??`, `for...in` over objects and arrays / `for...of`, `new.target`
in constructors and functions, `switch` on
strings, numbers, and mixed case values (JavaScript `===` semantics with
fallthrough), array spread, object spread and computed keys, getters and setters,
plain-TS destructuring declarations, destructuring assignment, `for...of`
declaration and assignment patterns, destructuring at the top of a
component body, `var` hoisting with `undefined` initialization (including
block, loop, and component-body declarations), class declarations / `new`, module-scope class expressions
directly assigned to a `const`/`let`, type-only imports and exports,
default and optional parameters, arrow/function consts, data-only
object-literal consts, statement-level `fetch(...).then(...).catch(...)`
chains inside async functions, trailing timer arguments on inline arrow
callbacks, and JSX list expressions. `for...in` destructuring
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
lowering yet. `.then`/`.catch` outside the supported statement-level
`fetch` chain needs a general promise-chain runtime, JSON extra arguments
would be silently dropped, and timer arguments need an inline arrow
callback whose parameters can receive them. Supporting any of them
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
// ❌ Promise chain used as a value — mx-js-syntax
async function load() {
  const pending = fetch("https://api.example.com/data").then((r) => r.text())
  console.log(pending)
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
| `for (const [k] in object)` | `for (const k in object) { … }` — bind the key plainly; destructure the value inside the body |
| `fetch(...).then(...)` as a value | a statement-level chain inside an async function (supported), or `const r = await fetch(...)` inside `try`/`catch` |
| `JSON.parse(text, reviver)` | parse once, then transform the value |
| `JSON.stringify(value, replacer, 2)` | stringify the value, then format the result |
| `setTimeout(handler, ms, a, b)` (identifier callback) | `setTimeout((a, b) => handler(a, b), ms, a, b)` |
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
