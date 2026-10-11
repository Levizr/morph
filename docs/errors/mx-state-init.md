# mx-state-init — Non-Literal State Initializer

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** reported by `morph check`.

## What this error means

A `morphState` / `morphShared` initializer is not a plain data literal:

```
error : mx-state-init : object state initializers cannot be translated to C++
  hint: use one morphState per field instead of an object
  Learn more: https://morph.levizr.com/docs/errors/mx-state-init
```

Signals lower to `Signal<T>` with the type read off the initializer, so
only plain literals have a native form: strings, numbers, booleans, arrays
of those, and negative integer literals. Anything else becomes
`Signal<auto>` (or names values invisible in component scope) and fails
the C++ compile.

## Why Morph raises it

- Objects have no signal struct — use one state per field.
- Variables and expressions are not visible where signals initialize —
  inline the literal value.
- Calls, templates, `null` and computed values have no literal form.

## Example that triggers it

```tsx
// ❌ Object state — mx-state-init
export default function App() {
  const [user, setUser] = morphState({ name: "Ada" });
  return <div>Hello</div>;
}
```

```tsx
// ✅ One state per field
export default function App() {
  const [userName, setUserName] = morphState("Ada");
  return <div>{userName}</div>;
}
```

## How to fix

Replace the initializer with a plain literal. If the value must be
computed, compute it in a `.cpp` helper and pass the result in as a
literal-compatible value. See [state](../javascript/state.md).

## See also

- [mx-state-scope](mx-state-scope.md) — state placement rules
- [mx-js-syntax](mx-js-syntax.md) — destructuring in component bodies
