# mx-style-value — Invalid Inline Style Value

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

An inline style property is supported, but its value is invalid:

```
warning : mx-style-value : Invalid value `"huge"` for style property `width`
  hint: Use a number (pixels) or a supported unit string like `"50%"`
  Learn more: https://morph.levizr.com/docs/errors/mx-style-value
```

This is a **warning**: the build proceeds and the declaration falls back to
the property default — the element renders, but not as sized/colored.

## Why Morph raises it

Style values compile to typed native values (floats, color channels, enums).
`"huge"` cannot become a float, so the declaration is dropped with a fallback.
Failing silently would leave you resizing a box that never changes; the
warning tells you exactly which declaration died. See
[supported units](../css/properties.md) (`px`, `%`, `rem`, `vh`/`vw`, ...).

## Example that triggers it

```tsx
// ⚠️ "huge" is not a width — mx-style-value (falls back to default width)
export default function App() {
  return <div style={{ width: "huge" }}>Hello</div>;
}
```

## How to fix

```tsx
// ✅ Number (pixels) or unit string
export default function App() {
  return (
    <div>
      <div style={{ width: 200 }}>Fixed</div>
      <div style={{ width: "50%" }}>Half</div>
    </div>
  );
}
```

Common fixes:

| Got | Want |
|---|---|
| `width: "huge"` | `width: 200` or `width: "50%"` |
| `color: "blu"` | `color: "blue"` or `color: "#0000ff"` |
| `display: "block"` (unsupported value) | `display: "flex"` |
| `opacity: "50%"` | `opacity: 0.5` |

Steps:

1. Read which property/value pair the warning names.
2. Replace with a typed value from the [properties list](../css/properties.md).
3. Re-run `morph check`.

## Tuning this rule

Escalate in CI if silent fallbacks are unacceptable:

```json
{
  "lint": {
    "severities": { "mx-style-value": "error" }
  }
}
```

## See also

- [mx-style-prop](mx-style-prop.md) — the property itself is unsupported
- [Which CSS Properties Morph Supports](../css/properties.md)

