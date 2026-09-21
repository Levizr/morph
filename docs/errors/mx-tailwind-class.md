# mx-tailwind-class — Unresolvable Tailwind Class

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

A token in `className` does not resolve to any Tailwind utility Morph knows:

```
warning : mx-tailwind-class : Unknown Tailwind class `flxe`
  hint: Did you mean `flex`? See the 500+ supported utilities
  Learn more: https://morph.levizr.com/docs/errors/mx-tailwind-class
```

This is a **warning**: the build proceeds, the unknown token is ignored, and
the element loses that piece of styling.

## Why Morph raises it

Morph resolves Tailwind classes **at compile time** to native style values —
there is no Tailwind runtime in the binary. A token with no mapping compiles to
nothing, so a typo (`flxe`, `bg-red-999`) silently drops styling. The warning
carries a suggestion because the intended class is usually one edit away. No
Node.js or Tailwind install is involved — the resolver is built into the
compiler (see [Tailwind support](../css/tailwind.md)).

## Example that triggers it

```tsx
// ⚠️ `flxe` resolves to nothing — mx-tailwind-class
export default function App() {
  return <div className="flxe items-center">Hello</div>;
}
```

## How to fix

```tsx
// ✅ Correct utility name
export default function App() {
  return <div className="flex items-center">Hello</div>;
}
```

When the class genuinely does not exist in Morph's set:

- Check the [Tailwind reference](../css/tailwind.md) — Morph covers 500+
  utilities, not the entire Tailwind catalog.
- Express it with an inline `style` or a CSS file rule instead.
- For dynamic class names, see
  [dynamic styles](../guides/dynamic-styles.md) — fully dynamic tokens cannot
  resolve at compile time either.

Steps:

1. Read the suggestion and fix the token spelling.
2. If no suggestion fits, verify against the supported list and rewrite with
   `style` or CSS.
3. Re-run `morph check`.

## Tuning this rule

```json
{
  "lint": {
    "disable": ["mx-tailwind-class"]
  }
}
```

Escalate instead if dropped utilities keep slipping through review:

```json
{
  "lint": {
    "severities": { "mx-tailwind-class": "error" }
  }
}
```

## See also

- [mx-dup-class](mx-dup-class.md) — competing class sources
- [mx-style-prop](mx-style-prop.md) — inline-style equivalent
- [How to Use Tailwind Classes](../css/tailwind.md)

