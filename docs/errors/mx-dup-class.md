# mx-dup-class — class and className Used Together

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

An element sets **both** `class` and `className`:

```
warning : mx-dup-class : `class` and `className` must not be used together on <div>
  hint: Keep `className` and merge both values into it
  Learn more: https://morph.levizr.com/docs/errors/mx-dup-class
```

This is a **warning**: the build proceeds, but only one of the two values
takes effect — half your styling silently disappears.

## Why Morph raises it

`class` is accepted as an alias for `className` (see [mx-prop](mx-prop.md)),
but they compile to the **same** native style-class slot. With both present
there is no merge order that every author would expect, so one value wins and
the other vanishes. The warning forces you to pick the single source of truth
instead of debugging invisible styles.

## Example that triggers it

```tsx
// ⚠️ Only one of the two applies — mx-dup-class
export default function App() {
  return <div class="card" className="highlight">Hello</div>;
}
```

## How to fix

```tsx
// ✅ Merge into className (the canonical prop)
export default function App() {
  return <div className="card highlight">Hello</div>;
}
```

With dynamic classes, use one template-literal `className` (see
[dynamic styles](../guides/dynamic-styles.md)):

```tsx
// ✅ One className, static + dynamic parts merged
import { morphState } from 'morph';

export default function App() {
  const [active, setActive] = morphState(false);
  return (
    <div className={`card ${active ? "highlight" : ""}`}>Hello</div>
  );
}
```

Steps:

1. Merge both class lists into a single `className`.
2. Use template literals for conditional classes — never a second attribute.
3. Re-run `morph check`.

## Tuning this rule

```json
{
  "lint": {
    "disable": ["mx-dup-class"]
  }
}
```

Prefer the fix above; suppression leaves two competing style sources.

## See also

- [mx-prop](mx-prop.md) — `class` vs `className`
- [mx-tailwind-class](mx-tailwind-class.md) — unresolvable class tokens
- [How to Change Styles at Runtime](../guides/dynamic-styles.md)

