# mx-js-method — Unimplemented Method

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

You call a method the runtime types do not implement — for example `.map()` /
`.split()` / `.toUpperCase()` in a position or on a value kind without support:

```
error : mx-js-method : Method `.toUpperCase()` is not implemented for this value
  hint: Check the runtime type docs for supported methods, or use a `.cpp` helper
  Learn more: https://morph.levizr.com/docs/errors/mx-js-method
```

## Why Morph raises it

String, array, and object operations in Morph lower to `morph::str` helpers
and container methods on the native types. Coverage is broad but not total
(see [translator coverage](../future/js-coverage.md)): exotic or highly
dynamic methods have no helper to call. Emitting a guessed equivalent would
change string semantics silently — the worst kind of bug in UI text — so the
call is rejected with the exact method named.

## Example that triggers it

```tsx
import { morphState } from 'morph';

export default function App() {
  const [name, setName] = morphState("ada");
  // ❌ If the method/position lacks support — mx-js-method
  return <div>{name.toFancyCase()}</div>;
}
```

## How to fix

1. Check the [runtime types](../javascript/types.md) and
   [comparisons](../javascript/js-comparisons.md) docs — the method you need
   may exist under a supported form.
2. Rewrite with supported operations (concatenation, comparisons, array
   helpers that exist).
3. Otherwise implement it in C++ and import it:

```tsx
// ✅ Native helper for the missing method
import { shout } from './helpers.cpp';

import { morphState } from 'morph';

export default function App() {
  const [name, setName] = morphState("ada");
  return <div>{shout(name)}</div>;
}
```

Steps:

1. Confirm the exact method in the error message.
2. Look for a supported equivalent; if none, write the `.cpp` helper.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unimplemented methods have no native code to call.

## See also

- [mx-js-member](mx-js-member.md) — unsupported builtin *members*
- [mx-transpile](mx-transpile.md) — the general untranslatable-code rule
- [JavaScript Runtime Types](../javascript/types.md)
- [How to Call C++ from Morph](../guides/native-cpp.md)

