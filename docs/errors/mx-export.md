# mx-export — Missing or Duplicate Default Export

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

Every `.mx` file that renders UI must contain exactly one
`export default function` component. `mx-export` fires in two cases:

- **No component found** — the file has no default-exported component, so Morph
  has nothing to render or compile for it.
- **Multiple default exports** — the file has more than one `export default`,
  so Morph cannot tell which component is the file's UI.

```
error : mx-export : No component found — expected `export default function App()`
  hint: Add `export default function App() { return (<div>...</div>) }`
  Learn more: https://morph.levizr.com/docs/errors/mx-export
```

## Why Morph raises it

Morph compiles each `.mx` file to a C++ translation unit with a single entry
point. The default export *is* that entry point: the entry file's default
export becomes the window contents, and any other file's default export is what
`import Hero from './Hero.mx'` binds to. Zero or two entry points means the
compiler cannot generate the module — so this is a hard error, not a warning.

Logic-only modules are exempt: `.ts` files, or files that only declare shared
stores (`morphShared`), event channels (`morphEvent`), or helper functions, are
not expected to export a component.

## Example that triggers it

```tsx
// ❌ No default export — mx-export: No component found
export function App() {
  return <div>Hello</div>;
}
```

```tsx
// ❌ Two default exports — mx-export: Multiple default exports
export default function App() {
  return <div>One</div>;
}

export default function Other() {
  return <div>Two</div>;
}
```

## How to fix

Export exactly one component as the default:

```tsx
// ✅ Exactly one default export
export default function App() {
  return <div>Hello</div>;
}
```

Named helper components in the same file are fine — only `export default` must
appear once:

```tsx
// ✅ Default export + named helpers
export function Card(props: { title: string }) {
  return <div>{props.title}</div>;
}

export default function App() {
  return <div><Card title="hi" /></div>;
}
```

Steps:

1. If the message is **No component found**, add
   `export default function App() { ... }` to the file — or, if the file is
   intentionally logic-only (stores/helpers), make sure it actually declares a
   `morphShared` / `morphEvent` binding or an exported function so it is
   recognized as a logic module.
2. If the message is **Multiple default exports**, keep one `export default`
   and change the rest to named exports (`export function ...`).
3. Re-run `morph check` — the error clears without any config change.

## Tuning this rule

This rule cannot be meaningfully disabled: without a single entry point the
file cannot compile. It has no `disable` use case.

## See also

- [mx-component-unknown](mx-component-unknown.md) — using a component that
  does not exist
- [How a Morph Project Is Structured](../getting-started/project-structure.md)

