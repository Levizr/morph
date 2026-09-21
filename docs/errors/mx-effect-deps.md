# mx-effect-deps — Effect Dependencies Should Be State

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

The dependency array of `morphEffect` contains values that are not reactive
state (or is otherwise suspicious):

```
warning : mx-effect-deps : Effect dependencies should be state variables
  hint: List the morphState/morphShared values the effect reads
  Learn more: https://morph.levizr.com/docs/errors/mx-effect-deps
```

This is a **warning**: the build proceeds, but the effect may run too often,
too rarely, or loop.

## Why Morph raises it

Effects re-run when their declared dependencies change, where "change" means
a reactive signal firing. A non-state value (a plain const, a prop-derived
computation, a fresh object literal) never fires — or fires every render —
so the effect's timing silently differs from what the array promises:

- **Missing state dep** → effect uses a stale value forever.
- **Non-state dep** (e.g. `{}` literal) → effect re-runs every render,
  potentially an infinite fetch loop.
- **Empty array with state reads inside** → effect never refreshes.

The warning nudges the array to match what the callback actually reads.

## Example that triggers it

```tsx
import { morphState, morphEffect } from 'morph';

export default function App() {
  const [userId, setUserId] = morphState("1");
  const options = { retry: true }; // plain object, never reactive

  // ⚠️ options is not state — mx-effect-deps (re-runs every render)
  morphEffect(() => {
    fetchUser(userId, options);
  }, [userId, options]);

  return <div>{userId}</div>;
}
```

## How to fix

```tsx
import { morphState, morphEffect } from 'morph';

export default function App() {
  const [userId, setUserId] = morphState("1");

  // ✅ Deps are exactly the reactive values read inside
  morphEffect(() => {
    fetchUser(userId, { retry: true });
  }, [userId]);

  return <div>{userId}</div>;
}
```

Steps:

1. List every reactive value the callback reads — those are the deps.
2. Move plain values (options objects, constants) inside the callback or to
   module scope.
3. If a value must be reactive, promote it to `morphState`/`morphShared`.
4. Re-run `morph check`.

## Tuning this rule

```json
{
  "lint": {
    "disable": ["mx-effect-deps"]
  }
}
```

Prefer fixing the array — stale-closure and loop bugs are the hardest runtime
bugs to diagnose.

## See also

- [mx-effect-cb](mx-effect-cb.md) — the callback itself
- [morphEffect API](../api/morphEffect.md) / [morphEffect FAQ](../api/faq/morphEffect.md)

