# mx-state-scope — morphState Outside a Component

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

`morphState` was called somewhere other than directly inside a component
function body — at module scope, inside a helper, or inside an event handler:

```
error : mx-state-scope : `morphState` may only be called inside a component body
  hint: Move the `const [value, setValue] = morphState(...)` declaration into a component
  Learn more: https://morph.levizr.com/docs/errors/mx-state-scope
```

## Why Morph raises it

`morphState` creates **per-instance** reactive state: each rendered instance
of the component gets its own independent slot, allocated by call order during
that component's render. Outside a component there is no instance and no
render — the call has nothing to attach to. This is the single most common
state bug (a "global" that silently behaves like a local or vice versa), so
Morph makes it a build error instead of a runtime mystery. Module-level shared
state exists — it is called [`morphShared`](../api/morphShared.md), and the
linter enforces the split from the other side
([mx-shared-scope](mx-shared-scope.md)).

## Example that triggers it

```tsx
import { morphState } from 'morph';

// ❌ Module scope has no component instance — mx-state-scope
const [count, setCount] = morphState(0);

export default function App() {
  return <div>{count}</div>;
}
```

```tsx
import { morphState } from 'morph';

export default function App() {
  const save = () => {
    // ❌ Inside a handler, not the component body — mx-state-scope
    const [draft, setDraft] = morphState("");
  };
  return <button onClick={save}>Save</button>;
}
```

## How to fix

Move the call into the component body (top level of the function, not nested
in handlers/conditions), or switch to `morphShared` for genuinely shared
state:

```tsx
// ✅ Fix 1: local state lives in the component body
import { morphState } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);
  return <button onClick={() => setCount(count + 1)}>{count}</button>;
}
```

```tsx
// ✅ Fix 2: shared state lives at exported module scope
import { morphShared } from 'morph';

export const [count, setCount] = morphShared(0);

export default function App() {
  return <div>{count}</div>;
}
```

Steps:

1. Decide: per-instance state (each `<Counter />` independent) → `morphState`
   inside the component; single app-wide value → exported `morphShared` at
   module scope.
2. Move the declaration — keep `morphState` calls unconditional and in the
   same order every render (hook rules apply).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. It guards the local-vs-shared distinction the whole
reactivity model rests on.

## See also

- [mx-shared-scope](mx-shared-scope.md) — the mirror rule for `morphShared`
- [mx-state-pattern](mx-state-pattern.md) — destructuring form
- [morphState API](../api/morphState.md) / [morphShared API](../api/morphShared.md)
- [Which Morph API Do I Need](../api/faq/choosing.md)

