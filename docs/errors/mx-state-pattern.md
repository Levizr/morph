# mx-state-pattern — morphState Destructuring Pattern

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

The return value of `morphState` was not destructured as a `[getter, setter]`
pair:

```
error : mx-state-pattern : `morphState` must be destructured as `const [value, setValue] = morphState(...)`
  hint: Use array destructuring with exactly two bindings
  Learn more: https://morph.levizr.com/docs/errors/mx-state-pattern
```

## Why Morph raises it

`morphState` returns a fixed two-element tuple: the reactive getter and its
setter. The compiler recognizes the pair **syntactically** — position 0 is
wired as the subscription, position 1 as the invalidation trigger. Any other
shape (single variable, renamed spread, object destructuring) breaks the
wiring the codegen depends on: the component would read a tuple object as a
value or lose its setter. Enforcing the pattern keeps every state declaration
greppable and every codegen assumption intact.

## Example that triggers it

```tsx
import { morphState } from 'morph';

export default function App() {
  // ❌ Whole tuple in one variable — mx-state-pattern
  const count = morphState(0);
  return <div>Hello</div>;
}
```

```tsx
export default function App() {
  // ❌ Object destructuring — mx-state-pattern
  const { value, setValue } = morphState(0);
  return <div>Hello</div>;
}
```

## How to fix

```tsx
import { morphState } from 'morph';

export default function App() {
  // ✅ Exactly [getter, setter]
  const [count, setCount] = morphState(0);
  return <button onClick={() => setCount(count + 1)}>{count}</button>;
}
```

With an explicit type argument:

```tsx
// ✅ Typed pair
const [name, setName] = morphState<string>("");
```

Steps:

1. Rewrite the declaration as `const [x, setX] = morphState(init)`.
2. Name the setter `setX` by convention (any two names work, convention keeps
   reviews fast).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Non-pair bindings cannot be wired to the reactivity
runtime.

## See also

- [mx-state-scope](mx-state-scope.md) — *where* the call must appear
- [morphState API](../api/morphState.md) / [How State Works](../javascript/state.md)

