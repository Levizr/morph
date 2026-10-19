# mx-effect-nested — `morphEffect` Must Be Top Level

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** reported by `morph check`.

## What this error means

A `morphEffect(...)` call sits somewhere other than the top level of a
component body — inside an `if`/`else`, a loop, a `switch`, a nested
block, or a helper function:

```
error : mx-effect-nested : `morphEffect` must be a top-level statement of the component body
  hint: Move the effect out of the branch/loop and guard its work with `if` on state
  Learn more: https://morph.levizr.com/docs/errors/mx-effect-nested
```

## Why Morph raises it

Effects are lifted at **build time**: each top-level `morphEffect` in a
component body becomes one native effect closure with a fixed creation
point and a stable dependency set. A call inside a branch or loop has no
such lift point — recreating the effect on every pass would stack
subscribers and leak them when the branch flips away. Because the
translator cannot schedule it, the call would otherwise reach the C++
compiler as an unknown function and fail with an opaque native error, so
`morph check` rejects it up front.

The same rule applies to effects nested inside a JSX callback or a helper
function declared in the component: they run at unpredictable times and
are not owned by the component lifecycle.

## Example that triggers it

```tsx
import { morphState, morphEffect } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);

  // ❌ No build-time lift point for a conditional effect — mx-effect-nested
  if (count > 0) {
    morphEffect(() => {
      console.log("positive");
    });
  }

  return <div>{count}</div>;
}
```

## How to fix

Register **one** top-level effect and put the condition inside it:

```tsx
import { morphState, morphEffect } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);

  // ✅ One effect; the branch lives in its body and re-runs on `count`
  morphEffect(() => {
    if (count > 0) {
      console.log("positive");
    }
  });

  return <div>{count}</div>;
}
```

Need setup/teardown per branch? Return a cleanup from the single effect —
it runs before the next run and on unmount:

```tsx
morphEffect(() => {
  if (count <= 0) return;
  const timer = setInterval(() => console.log("tick"), 500);
  return () => clearInterval(timer);
});
```

For work that must happen in a loop, keep the loop *inside* the effect:

```tsx
morphEffect(() => {
  for (const row of rows) {
    console.log(row);
  }
});
```

Steps:

1. Move every `morphEffect(...)` call out of the branch, loop, or helper
   function to the top level of the component body.
2. Put the original condition at the start of the callback (an early
   `return` is fine).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. An effect without a lift point cannot be owned
by the component lifecycle, so it cannot be cleaned up.

## See also

- [mx-effect-cb](mx-effect-cb.md) — the first argument must be a function
- [mx-effect-deps](mx-effect-deps.md) — the dependency array
- [morphEffect API](../api/morphEffect.md) / [How Effects Work](../javascript/effects.md)
