# mx-effect-cb — morphEffect First Argument Must Be a Function

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

The first argument to `morphEffect` is not a function:

```
error : mx-effect-cb : `morphEffect` first argument must be a function
  hint: Pass `morphEffect(() => { ... }, [deps])`
  Learn more: https://morph.levizr.com/docs/errors/mx-effect-cb
```

## Why Morph raises it

`morphEffect` registers a side effect to run **after render** and re-run when
its dependencies change. The compiler lifts that callback into a native effect
closure bound to the component lifecycle. A non-function first argument (or a
function *call* instead of a function) leaves nothing to lift — and the common
variant `morphEffect(fetchData(), [])` runs the effect immediately during
render while registering its *return value* as the effect. Both break the
lifecycle contract, so the call is rejected.

## Example that triggers it

```tsx
import { morphEffect } from 'morph';

export default function App() {
  // ❌ Calls fetchData during render, registers undefined — mx-effect-cb
  morphEffect(fetchData(), []);
  return <div>Hello</div>;
}
```

## How to fix

```tsx
import { morphEffect } from 'morph';

export default function App() {
  // ✅ Pass the function; call it inside
  morphEffect(() => {
    fetchData();
  }, []);
  return <div>Hello</div>;
}
```

Cleanup functions follow the same rule — return the cleanup from the callback:

```tsx
// ✅ Effect with cleanup
morphEffect(() => {
  const timer = setTimeout(() => console.log("tick"), 1000);
  return () => clearTimeout(timer);
}, []);
```

Steps:

1. Wrap the work in `() => { ... }` as the first argument.
2. If the effect needs arguments, close over them or read state inside — do
   not call the function in the argument position.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. A non-function effect cannot be scheduled.

## See also

- [mx-effect-deps](mx-effect-deps.md) — the dependency array
- [morphEffect API](../api/morphEffect.md) / [How Effects Work](../javascript/effects.md)

