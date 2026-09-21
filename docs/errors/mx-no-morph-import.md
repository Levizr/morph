# mx-no-morph-import — Morph API Used Without Import

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

You call a Morph API (`morphState`, `morphEffect`, `morphShared`,
`morphEvent`, `useWindow`, `Window`, `CSS`) without importing it from
`'morph'`:

```
error : mx-no-morph-import : `morphState` is used but never imported
  hint: Add `import { morphState } from 'morph'`
  Learn more: https://morph.levizr.com/docs/errors/mx-no-morph-import
```

## Why Morph raises it

Morph APIs are compiler intrinsics keyed off their **import binding**, not
bare globals. The import tells the compiler which calls are framework calls
(to lower to signals/effects/channels) versus ordinary user functions that
happen to share a name. Without the import, `morphState(0)` looks like a call
to an undefined function — and treating it as one
([mx-undefined](mx-undefined.md)) would give you the wrong fix. This dedicated
code gives you the right one: add the import.

## Example that triggers it

```tsx
// ❌ morphState never imported — mx-no-morph-import
export default function App() {
  const [count, setCount] = morphState(0);
  return <div>{count}</div>;
}
```

## How to fix

```tsx
// ✅ Import from 'morph' at the top of the file
import { morphState } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);
  return <div>{count}</div>;
}
```

Steps:

1. Add `import { <Name> } from 'morph'` for each flagged API.
2. Keep the import path exactly `'morph'` — not a relative path, not an npm
   package.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unimported APIs cannot be lowered to native code.

## See also

- [mx-import-morph](mx-import-morph.md) — importing a name that does not exist
- [mx-undefined](mx-undefined.md) — genuinely unknown names
- [Which Morph API Do I Need](../api/faq/choosing.md)

