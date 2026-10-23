# mx-img-src — img Without src

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** enforced by `morph check`. The rule reports at the source site
> before codegen, so unsupported code cannot reach the C++ compiler.

## What this error means

An `<img>` element has no `src` attribute:

```
error : mx-img-src : <img> requires a `src` attribute
  hint: Add `src="./photo.png"` (local path or URL)
  Learn more: https://morph.levizr.com/docs/errors/mx-img-src
```

## Why Morph raises it

Native image nodes decode and upload a texture at build/layout time from
`src`. An `<img>` without a source would create a texture node with nothing to
decode — a blank box that still costs layout and (on some drivers) logs
low-level errors. The web tolerates `<img>` without `src`; native code has no
sensible default, so Morph requires it up front.

The value must be a string/template literal or an expression that can produce
one at runtime. `src={null}`, `{undefined}`, `{NaN}`, `{Infinity}`, objects,
arrays, and inline function literals are rejected — none of them can name an
asset. A call that returns a path (`src={getSrc()}`) is allowed.

## Example that triggers it

```tsx
// ❌ No src — mx-img-src
export default function App() {
  return (
    <div>
      <img alt="photo" width={200} height={120} />
    </div>
  );
}
```

## How to fix

```tsx
// ✅ Local asset
export default function App() {
  return (
    <div>
      <img src="./photo.png" alt="photo" width={200} height={120} />
    </div>
  );
}
```

```tsx
// ✅ Remote URL
export default function App() {
  return (
    <div>
      <img src="https://example.com/photo.png" alt="photo" />
    </div>
  );
}
```

Steps:

1. Add `src` with a path to a bundled asset or a URL.
2. Verify the file exists at the given relative path (paths resolve relative
   to the source file).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. A sourceless image can never render.

## See also

- [mx-tag](mx-tag.md) — unknown element
- [mx-prop](mx-prop.md) — other invalid props
- [Which HTML Elements Morph Supports](../elements/overview.md)

