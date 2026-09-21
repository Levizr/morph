# mx-style-prop — Unsupported Inline Style Property

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

An inline `style` object uses a property Morph does not support:

```
error : mx-style-prop : Unsupported style property `float` on <div>
  hint: See the supported CSS list — use Flexbox (`display: flex`) for layout
  Learn more: https://morph.levizr.com/docs/errors/mx-style-prop
```

## Why Morph raises it

Inline styles compile to fields on the native style struct consumed by the
layout engine. An unsupported property has no field — there is nowhere to
store it. Ignoring it would silently produce a different layout than the one
you coded (the classic `float`-shaped hole). Morph fails loudly and points at
the supported alternative. See
[which CSS properties Morph supports](../css/properties.md).

## Example that triggers it

```tsx
// ❌ float does not exist in Morph layout — mx-style-prop
export default function App() {
  return <div style={{ float: "left", width: 200 }}>Hello</div>;
}
```

## How to fix

```tsx
// ✅ Flexbox layout with supported properties
export default function App() {
  return (
    <div style={{ display: "flex", flexDirection: "row" }}>
      <div style={{ width: 200 }}>Hello</div>
    </div>
  );
}
```

Strategies when your property is unsupported:

- **Layout** (`float`, `position: absolute` tricks) → Flexbox
  ([guide](../css/flexbox.md)).
- **Effects** (`box-shadow`, `outline`) → check the
  [future CSS plans](../future/more-css.md); restyle with borders/backgrounds.
- **Typos** (`backgroudColor`) → fix the spelling; property names are
  camelCase in `style={{...}}` (`backgroundColor`, not `background-color`).

Steps:

1. Read the hint and the [supported properties list](../css/properties.md).
2. Replace the property with its supported equivalent, or move the styling to
   a Tailwind class.
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. Unsupported style properties cannot affect native
rendering — silencing the error only hides dead declarations.

## See also

- [mx-style-value](mx-style-value.md) — property fine, value invalid
- [mx-css-prop](mx-css-prop.md) — the stylesheet-file equivalent
- [Which CSS Properties Morph Supports](../css/properties.md)

