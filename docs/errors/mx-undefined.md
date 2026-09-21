# mx-undefined — Undefined Name

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

You reference a name that is not declared, imported, a component, or a
supported native global:

```
error : mx-undefined : `cout` is not defined here and not imported
  hint: Did you mean `count`?
  Learn more: https://morph.levizr.com/docs/errors/mx-undefined
```

Lowercase JSX tags (`<div>`) and type positions (`x: number`) are excluded —
only real value references are checked, with full scope awareness (shadowing,
params, and imports all count as declared).

## Why Morph raises it

Every identifier in `.mx` code must lower to something: a local, a prop, an
import, a Morph API, or a supported runtime global (`fetch`, `console`,
`setTimeout`, ...). An unresolved name has no lowering — the C++ backend would
hit an undeclared identifier. Failing here gives you the file, line, column,
and a `Did you mean ... ?` suggestion instead of a page of C++ errors.

## Example that triggers it

```tsx
import { morphState } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);
  // ❌ `cout` is a typo for `count` — mx-undefined
  return <div>{cout}</div>;
}
```

Other common causes:

- Missing import (`Hero` used but never imported — though capitalized tags
  usually surface as [mx-component-unknown](mx-component-unknown.md) first).
- Using a variable outside its scope (inside a different component or block).
- Browser globals (`document`, `localStorage`) — those surface as
  [mx-js-global](mx-js-global.md), not here.

## How to fix

```tsx
import { morphState } from 'morph';

export default function App() {
  const [count, setCount] = morphState(0);
  // ✅ Reference the declared name
  return <div>{count}</div>;
}
```

Steps:

1. Read the suggestion — for typos it is usually exact.
2. If the name lives in another file, import it (components, helpers, stores).
3. If it is a Morph API, import it from `'morph'` — see
   [mx-no-morph-import](mx-no-morph-import.md).
4. If it is a browser/Node global, replace it with a Morph-native equivalent
   — see [mx-js-global](mx-js-global.md).
5. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unresolved names can never compile.

## See also

- [mx-no-morph-import](mx-no-morph-import.md) — the name is a Morph API
- [mx-component-unknown](mx-component-unknown.md) — the name is a component tag
- [mx-js-global](mx-js-global.md) — the name is a browser global

