# mx-component-unknown — Unknown Component

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

You used a capitalized tag (`<Hero />`) that is neither defined in the current
file nor imported, and (for multi-file projects) cannot be resolved through the
module graph:

```
error : mx-component-unknown : Unknown component `<Hero>`
  hint: `Hero` is not defined here and not imported — add `import Hero from './...mx'`
  Learn more: https://morph.levizr.com/docs/errors/mx-component-unknown
```

Lowercase tags are checked as native elements instead
([mx-tag](mx-tag.md)); capitalized tags are always treated as components.

## Why Morph raises it

Capitalized JSX tags compile to component instantiations — C++ struct
construction with prop binding. If no declaration backs the tag, there is no
struct to construct and no props table to validate against. Morph fails at
`morph check` time so you never pay for a full native compile to discover a
typo or a missing import.

## Example that triggers it

```tsx
// ❌ Hero is not defined in this file and not imported
export default function App() {
  return (
    <div>
      <Hero title="hi" />
    </div>
  );
}
```

Common causes beyond typos:

- The component lives in another file but the `import` line is missing.
- The import path is wrong (`'./hero.mx'` vs `'./Hero.mx'` — paths are
  case-sensitive).
- You imported a **named** export as default or vice versa:
  `import Hero from './ui.mx'` when `ui.mx` only has `export function Hero`
  (needs `import { Hero } from './ui.mx'`).

## How to fix

Define the component locally or import it with the matching form:

```tsx
// ✅ Option 1: define it in the same file
export function Hero(props: { title: string }) {
  return <div>{props.title}</div>;
}

export default function App() {
  return (
    <div>
      <Hero title="hi" />
    </div>
  );
}
```

```tsx
// ✅ Option 2: import it (Hero.mx has a default export)
import Hero from './Hero.mx';

export default function App() {
  return (
    <div>
      <Hero title="hi" />
    </div>
  );
}
```

```tsx
// ✅ Option 3: named import (ui.mx has `export function Hero`)
import { Hero } from './ui.mx';

export default function App() {
  return (
    <div>
      <Hero title="hi" />
    </div>
  );
}
```

Steps:

1. Check the spelling and capitalization of the tag.
2. If the component is in another file, add the import — default form for the
   file's default export, `{ Braces }` for named exports.
3. Verify the import path and extension (`.mx`, `.tsx`, `.ts`).
4. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. An unresolvable component is always a build failure
later (`module for <Hero> is missing from the graph`) — this diagnostic only
moves it earlier with a file and line number.

## See also

- [mx-component-prop](mx-component-prop.md) — component exists, prop does not
- [mx-component-required](mx-component-required.md) — required prop missing
- [mx-tag](mx-tag.md) — the lowercase (native element) equivalent
- [Reusable Components](../elements/components.md)

