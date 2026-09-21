# mx-js-syntax — Unhandleable Syntax Construct

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

Your code uses a syntax construct the translator cannot handle: `?.`
(optional chaining), destructuring (outside the `morphState` pair pattern),
generators / `yield`, `for...in` / `for...of`, object spread, spread in call
arguments, or object methods/getters:

```
error : mx-js-syntax : `?.` optional chaining cannot be translated — use an explicit check
  hint: Write `user ? user.name : ""` instead of `user?.name`
  Learn more: https://morph.levizr.com/docs/errors/mx-js-syntax
```

## Why Morph raises it

Each of these constructs needs hidden runtime machinery Morph omits on
purpose: `?.` needs short-circuit control flow the expression emitter does not
build; destructuring needs binding analysis beyond the state-pair pattern;
generators need resumable frames; `for...in/of` need iterators with JS
enumeration semantics; spreads need reflective copies; getters need hidden
call sites. Supporting any of them approximately would change program meaning
— so each is rejected with its explicit rewrite. See
[translator coverage](../future/js-coverage.md) for what is planned.

## Examples that trigger it

```tsx
// ❌ Optional chaining — mx-js-syntax
export default function App() {
  const user = { name: "Ada" };
  return <div>{user?.name}</div>;
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
// ❌ for...of — mx-js-syntax
export default function App() {
  let total = 0;
  for (const n of [1, 2, 3]) {
    total += n;
  }
  return <div>{total}</div>;
}
```

## How to fix

| Instead of | Write |
|---|---|
| `a?.b` / `a?.()` | `a ? a.b : fallback` |
| `const {x} = obj` | `const x = obj.x` |
| `{...o, k: v}` | explicit object literal |
| `f(...args)` | pass arguments explicitly |
| `for...in` / `for...of` | indexed `for` / `.map()` rendering |
| generator / `yield` | state machine with `morphState` |
| object method/getter | plain function property or helper |

```tsx
// ✅ Explicit checks and access
export default function App() {
  const user = { name: "Ada" };
  const name = user ? user.name : "";
  return <div>{name}</div>;
}
```

```tsx
// ✅ Indexed loop instead of for...of
export default function App() {
  const nums = [1, 2, 3];
  let total = 0;
  for (let i = 0; i < nums.length; i++) {
    total += nums[i];
  }
  return <div>{total}</div>;
}
```

Steps:

1. Match the flagged construct to the table rewrite.
2. For object-heavy logic, consider moving it to `.cpp` (see
   [native C++](../guides/native-cpp.md)).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. These constructs have no native representation.

## See also

- [mx-js-op](mx-js-op.md) — unsupported operators
- [mx-transpile](mx-transpile.md) — the general rule
- [How Morph Compiles JavaScript](../javascript/overview.md)

