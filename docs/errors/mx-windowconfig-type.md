# mx-windowconfig-type — Invalid windowConfig Value Type

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

A `windowConfig` key is valid, but its value has the wrong type — for example a
string where a number is required:

```
error : mx-windowconfig-type : windowConfig `width` must be a number, got string
  hint: Use `width: 800` (a number), not `width: "800"`
  Learn more: https://morph.levizr.com/docs/errors/mx-windowconfig-type
```

## Why Morph raises it

`windowConfig` values flow straight into typed C++ window parameters
(`uint32_t` dimensions, `bool` flags, `std::string` title). A mistyped value
cannot be coerced safely — `"800"` as a width or `"yes"` as a boolean would
need JavaScript-style coercion inside native window creation. Morph rejects the
value at the config site instead of guessing.

## Example that triggers it

```tsx
// ❌ width/height must be numbers, visible must be boolean
export const windowConfig = { title: "App", width: "800", height: 600, visible: "yes" };

export default function App() {
  return <div>Hello</div>;
}
```

## How to fix

```tsx
// ✅ Correct value types
export const windowConfig = { title: "App", width: 800, height: 600, visible: true };

export default function App() {
  return <div>Hello</div>;
}
```

Type expectations:

| Key | Type |
|---|---|
| `title` | `string` |
| `width`, `height`, `minWidth`, `minHeight`, `maxWidth`, `maxHeight` | `number` |
| `visible`, `modal` | `boolean` |

Steps:

1. Read which key the error names and what type it expects.
2. Fix the literal — remove quotes around numbers, use `true`/`false` for
   flags.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. A mistyped window value cannot produce a correct
native window.

## See also

- [mx-windowconfig-key](mx-windowconfig-key.md) — unknown key name
- [How to Configure a Morph Project](../getting-started/configuration.md)

