# mx-css-load-deprecated — CSS.load() Is Deprecated

**Severity:** warning | **Blocks `morph build`:** no

## What this error means

You load a stylesheet with the old `CSS.load(...)` call instead of an
`import`:

```
warning : mx-css-load-deprecated : `CSS.load("./app.css")` is deprecated
  hint: Use `import "./app.css"` instead
  Learn more: https://morph.levizr.com/docs/errors/mx-css-load-deprecated
```

This is a **warning**: the build still works, but the API is on its way out.

## Why Morph raises it

Stylesheet imports are resolved **statically** into the module graph so the
bundler can order, parse, and bake them at compile time. `CSS.load()` is an
imperative runtime-ish call that hides the dependency from the graph —
ordering, caching, and missing-file detection
([mx-css-file-missing](mx-css-file-missing.md)) all work worse through it. The
bare `import "./x.css"` form declares the dependency where the compiler can
see it, so the old call is deprecated with a mechanical migration.

## Example that triggers it

```tsx
// ⚠️ Deprecated form — mx-css-load-deprecated
import { CSS } from 'morph';
CSS.load("./app.css");

export default function App() {
  return <div className="card">Hello</div>;
}
```

## How to fix

```tsx
// ✅ Static import — same stylesheet, visible to the module graph
import "./app.css";

export default function App() {
  return <div className="card">Hello</div>;
}
```

Steps:

1. Delete the `CSS.load(...)` statement (and the `CSS` import if unused).
2. Add `import "./app.css"` at the top of the file (path relative to the
   file, as before).
3. Re-run `morph check`.

## Tuning this rule

```json
{
  "lint": {
    "disable": ["mx-css-load-deprecated"]
  }
}
```

Only suppress as a stopgap during large migrations — new code should always
use `import`.

## See also

- [mx-css-file-missing](mx-css-file-missing.md) — imported file not found
- [mx-css-prop](mx-css-prop.md) — unsupported property in the file
- [Reusable Components](../elements/components.md)

