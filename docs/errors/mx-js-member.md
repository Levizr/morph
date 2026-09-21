# mx-js-member — Unsupported Builtin Member Access

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

You access a member of a JS builtin object Morph does not implement — `Math.*`,
`JSON.*`, `Date.*`, `RegExp.*`, or `console.*` beyond
`log`/`warn`/`error`/`info`:

```
error : mx-js-member : Unsupported member `JSON.parse` — no native counterpart
  hint: Parse with explicit conversion code or a `.cpp` helper
  Learn more: https://morph.levizr.com/docs/errors/mx-js-member
```

## Why Morph raises it

Morph's runtime types (`JsNumber`, `JsString`, `JsArray`, `JsObject` — see
[runtime types](../javascript/types.md)) implement a curated subset of
JavaScript semantics with zero-overhead native backing. Members outside that
subset (`Math.hypot`, `JSON.parse`, `Date.now`, `RegExp`, `console.table`)
have no implementation to call into. Rather than stubbing them with wrong
semantics, Morph rejects the access and points at the replacement.

## Example that triggers it

```tsx
// ❌ JSON.parse is not implemented — mx-js-member
export default function App() {
  const data = JSON.parse('{"n": 1}');
  return <div>Hello</div>;
}
```

```tsx
// ❌ console.table is not implemented (log/warn/error/info are) — mx-js-member
export default function App() {
  console.table([1, 2, 3]);
  return <div>Hello</div>;
}
```

## How to fix

```tsx
// ✅ console.log/warn/error/info are supported
export default function App() {
  console.log("rendered");
  return <div>Hello</div>;
}
```

For the rest, options in order of preference:

1. **Supported equivalent** — many common operations exist on the runtime
   types or as operators; check [runtime types](../javascript/types.md) and
   [native types](../javascript/native-types.md).
2. **Explicit code** — date math with numbers, manual parsing for simple
   shapes.
3. **`.cpp` helper** — implement it natively and import it (see
   [native C++](../guides/native-cpp.md)).

```tsx
// ✅ Native helper for unsupported builtins
import { parseCount } from './helpers.cpp';

export default function App() {
  return <div>{parseCount("3")}</div>;
}
```

Steps:

1. Check whether a supported equivalent exists in the runtime types docs.
2. Otherwise extract the operation into a `.cpp` function and import it.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unimplemented members have no code to call.

## See also

- [mx-js-method](mx-js-method.md) — unsupported *methods* on values
- [mx-js-global](mx-js-global.md) — unsupported *globals*
- [JavaScript Runtime Types](../javascript/types.md)

