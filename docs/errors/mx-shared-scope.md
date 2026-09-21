# mx-shared-scope — morphShared Not at Exported Module Scope

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

`morphShared` was called inside a component (or another function), or at
module scope without `export`:

```
error : mx-shared-scope : `morphShared` may only appear at module scope
  hint: Move `export const [value, setValue] = morphShared(...)` to the top of the file
  Learn more: https://morph.levizr.com/docs/errors/mx-shared-scope
```

```
error : mx-shared-scope : `morphShared` must be exported to be shared across modules
  hint: Add `export` to the `const [value, setValue] = morphShared(...)` binding
  Learn more: https://morph.levizr.com/docs/errors/mx-shared-scope
```

## Why Morph raises it

`morphShared` creates **one** store per app, shared by every importer — the
opposite of [`morphState`](../api/morphState.md). That contract only holds if
the binding lives at module scope (exactly one evaluation) and is exported
(importers bind to it by name). Inside a component it would run per render,
creating a fresh "shared" store every render — the worst of both worlds. The
linter enforces placement so the local/shared split is syntactic and
unmistakable. (Old string-key forms are a separate error:
[mx-api-removed](mx-api-removed.md).)

## Example that triggers it

```tsx
import { morphShared } from 'morph';

export default function App() {
  // ❌ Inside a component — mx-shared-scope (use morphState here)
  const [count, setCount] = morphShared(0);
  return <div>{count}</div>;
}
```

```tsx
import { morphShared } from 'morph';

// ❌ Module scope but not exported — mx-shared-scope
const [count, setCount] = morphShared(0);

export default function App() {
  return <div>{count}</div>;
}
```

## How to fix

```tsx
// ✅ Exported, module scope, single initial value
import { morphShared } from 'morph';

export const [count, setCount] = morphShared(0);

export default function App() {
  return <div>{count}</div>;
}
```

```tsx
// ✅ Per-instance instead — morphState inside the component
import { morphState } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);
  return <div>{count}</div>;
}
```

Steps:

1. If the value is shared across components, hoist the binding to the top of
   the file and add `export`.
2. If it is per-instance, switch the call to `morphState` inside the
   component body.
3. Keep the single-argument form `morphShared<T>(initial)` — two arguments is
   the removed API (see [mx-api-removed](mx-api-removed.md)).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Misplaced shared state is the most common state bug
Morph guards against.

## See also

- [mx-state-scope](mx-state-scope.md) — the mirror rule for `morphState`
- [mx-event-scope](mx-event-scope.md) — same placement rule for events
- [mx-api-removed](mx-api-removed.md) — old string-key form
- [morphShared API](../api/morphShared.md) / [morphShared FAQ](../api/faq/morphShared.md)

