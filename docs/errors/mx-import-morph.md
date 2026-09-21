# mx-import-morph — Invalid Import from 'morph'

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

You import a name from `'morph'` that the module does not export:

```
error : mx-import-morph : `morphStore` is not exported by 'morph'
  hint: Available: morphState, morphShared, morphEvent, morphEffect, useWindow, Window, CSS
  Learn more: https://morph.levizr.com/docs/errors/mx-import-morph
```

## Why Morph raises it

`'morph'` is not an npm package — it is the compiler's own API surface, and
every export maps to generated native code (`morphState` → signal allocation,
`morphEvent` → channel, `CSS` → stylesheet import). A name with no export has
no mapping, so any use of it would compile to a reference to nothing. The
check fails at the import line, before the phantom name spreads through your
code as [mx-undefined](mx-undefined.md) noise.

The exported surface is: `morphState`, `morphShared`, `morphEvent`,
`morphEffect`, `useWindow`, `Window`, `CSS`.

## Example that triggers it

```tsx
// ❌ morphStore does not exist — mx-import-morph
import { morphStore } from 'morph';

export default function App() {
  return <div>Hello</div>;
}
```

Typical cause: guessing a React-style name (`useState`, `useEffect`) or a
removed API (`morphOn`, `morphEmit` — see
[mx-api-removed](mx-api-removed.md)).

## How to fix

```tsx
// ✅ Import a real export
import { morphState } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);
  return <div>{count}</div>;
}
```

Steps:

1. Read the hint's export list and pick the API you meant
   ([choosing guide](../api/faq/choosing.md)).
2. If you need removed string-channel APIs (`morphOn`/`morphEmit`), migrate to
   `morphEvent` instead — see [mx-api-removed](mx-api-removed.md).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Phantom imports always fail later with worse
messages.

## See also

- [mx-no-morph-import](mx-no-morph-import.md) — using the API without importing
- [mx-api-removed](mx-api-removed.md) — removed APIs
- [Morph API Reference Overview](../api/overview.md)

