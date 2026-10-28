# mx-effect-cb — Invalid morphEffect Callback

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** enforced by `morph check`. The rule reports at the source site
> before codegen, so unsupported code cannot reach the C++ compiler.

## What this error means

The first argument to `morphEffect` is not a valid effect callback. There are
two cases:

```
error : mx-effect-cb : `morphEffect` first argument must be a function
  hint: Pass `morphEffect(() => { ... }, [deps])`
  Learn more: https://morph.levizr.com/docs/errors/mx-effect-cb
```

```
error : mx-effect-cb : `morphEffect` callback must take no parameters
  hint: Remove the callback parameter; effects run after render with no arguments
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

Method references (`morphEffect(cleaner.cleanup, [])`) are also rejected: the
lifter emits a standalone callable and cannot bind the receiver object. Move
the effect body into an arrow that calls the method instead.

Effect callbacks also take **no parameters**. The runtime invokes the lifted
callback with no arguments, so a parameter would stay unbound in the generated
C++ closure (or produce an invalid native call). Read state directly inside
the effect, or close over a local value.

## Examples that trigger it

```tsx
import { morphEffect } from 'morph';

export default function App() {
  // ❌ Calls fetchData during render, registers undefined — mx-effect-cb
  morphEffect(fetchData(), []);

  // ❌ The effect runner has no argument to pass — mx-effect-cb
  morphEffect((value) => console.log(value), []);

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
2. Leave the callback parameter list empty. If the effect needs values,
   close over them or read state inside — do not call the function in the
   argument position.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. A non-function effect cannot be scheduled.

## See also

- [mx-effect-deps](mx-effect-deps.md) — the dependency array
- [morphEffect API](../api/morphEffect.md) / [How Effects Work](../javascript/effects.md)

