# mx-route-unknown — Unknown Route ID

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

A route string passed to `new Window`, `navigate`, `load`, or an `<a href>`
does not exist in the route manifest (the set of `route.mx` files), or is not
a string literal:

```
error : mx-route-unknown : unknown route `/auth/loign` — did you mean `/auth/login`? Known routes: /, /auth/login, /settings
  Learn more: https://morph.levizr.com/docs/errors/mx-route-unknown
```

Unlike most codes, this fires at **build/codegen time**, not in
`morph check` — the manifest is assembled during the build.

## Why Morph raises it

Route ids compile to integer constants (`RID`s): the manifest pass interns
every literal id to a number and every call site emits the number — strings
never reach the runtime. An unknown id has no number to emit, and a typo would
otherwise navigate nowhere at runtime with no error. Failing with a
did-you-mean suggestion plus the full known-route list turns a blank screen
into a one-line fix. Non-literal ids (`navigate(someVar)`) cannot intern and
fail the same way — hoist the id to a literal.

## Example that triggers it

```tsx
// ❌ Typo: /auth/loign is not a route — mx-route-unknown
import { Window } from 'morph';

export default function App() {
  const open = () => {
    const w = new Window({ routeId: "/auth/loign" });
  };
  return <button onClick={open}>Login</button>;
}
```

```tsx
// ❌ Same check on links: internal hrefs validate against the manifest too
export default function App() {
  return <a href="/auth/loign">Login</a>;
}
```

## How to fix

```tsx
// ✅ Exact route id from the manifest (folder path of route.mx)
import { Window } from 'morph';

export default function App() {
  const open = () => {
    const w = new Window({ routeId: "/auth/login" });
  };
  return <button onClick={open}>Login</button>;
}
```

```tsx
// ✅ Fixed link href
export default function App() {
  return <a href="/auth/login">Login</a>;
}
```

Steps:

1. Read the `Known routes:` list in the error — it is the ground truth.
2. Fix the typo, minding leading `/`, exact folder nesting, and lowercase
   (route segments follow URL conventions).
3. If the route file is missing, create `src/<path>/route.mx` with a default
   export (see [mx-route-no-export](mx-route-no-export.md)).
4. If the id is dynamic (`navigate(path)`), restructure to literals — only
   literal ids intern.
5. Rebuild.

## Tuning this rule

Do not disable this rule. Unknown routes cannot navigate.

## See also

- [mx-route-no-export](mx-route-no-export.md) — route file without component
- [Window and useWindow API](../api/windows.md)
- [Windows and Routing Guide](../guides/windows-and-routing.md)

