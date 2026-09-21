# mx-img-src — img Without src

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

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

