# mx-css-file-missing — Imported CSS File Not Found

**Severity:** error | **Blocks `morph build`:** yes

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

An `import "./app.css"` (or `url(...)` reference) points to a file that does
not exist on disk:

```
error : mx-css-file-missing : CSS file not found: `./styles/app.css`
  hint: Check the relative path — imports resolve from the importing file
  Learn more: https://morph.levizr.com/docs/errors/mx-css-file-missing
```

## Why Morph raises it

CSS is bundled at compile time: the compiler reads every imported stylesheet,
parses it, and bakes the rules into the binary. A missing file means the
bundle step has nothing to read — and the app would ship unstyled with no
indication why. Failing fast names the exact import to fix.

## Example that triggers it

```tsx
// ❌ ./styles/app.css does not exist — mx-css-file-missing
import "./styles/app.css";

export default function App() {
  return <div className="card">Hello</div>;
}
```

## How to fix

```tsx
// ✅ Point at the real file (relative to this source file)
import "./app.css";

export default function App() {
  return <div className="card">Hello</div>;
}
```

Steps:

1. Check the path spelling, directory, and extension (`.css`).
2. Remember imports resolve **relative to the importing file**, not the
   project root — `./app.css` from `src/App.mx` means `src/app.css`.
3. If the stylesheet was deleted on purpose, remove the import line too.
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. A missing stylesheet cannot be bundled.

## See also

- [mx-css-prop](mx-css-prop.md) — file found, property unsupported
- [mx-import-type](mx-import-type.md) — import kind not supported
- [How a Morph Project Is Structured](../getting-started/project-structure.md)

