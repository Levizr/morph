# mx-js-global — Browser Global With No Native Counterpart

**Severity:** error | **Blocks `morph build`:** yes

## What this error means

Your code references a browser/JS global that does not exist in the native
runtime — `document`, `window`, `localStorage`, `sessionStorage`, `navigator`,
`location`, `history`, `screen`, `alert`, `prompt`, `confirm`, or
`requestAnimationFrame`:

```
error : mx-js-global : `document` is not available in native runtime
  hint: Use Morph state / C++ instead of browser `document`
  Learn more: https://morph.levizr.com/docs/errors/mx-js-global
```

## Why Morph raises it

There is no DOM, no `window` object, and no browser storage inside a Morph
binary — your UI *is* native nodes, not HTML. These globals therefore have no
lowering target at all. Each one is rejected with a direction, not silence:
state and effects replace reactivity needs, `fetch` + `setTimeout` exist
natively, and anything else drops to C++.

Supported native globals (usable without import): `console`, `fetch`,
`setTimeout`, `setInterval`, `clearTimeout`, `clearInterval`, `Promise`,
`Error`, `undefined`, `NaN`, `Infinity`.

## Example that triggers it

```tsx
// ❌ No DOM in native runtime — mx-js-global
export default function App() {
  const save = () => {
    localStorage.setItem("draft", "hello");
  };
  return <button onClick={save}>Save</button>;
}
```

```tsx
// ❌ document has no meaning here — mx-js-global
export default function App() {
  const w = document.body.clientWidth;
  return <div>{w}</div>;
}
```

## How to fix

| Instead of | Use |
|---|---|
| `document` / `window` queries | Morph layout, state, and refs |
| `localStorage` / `sessionStorage` | `morphShared` (+ native file I/O via `.cpp` for persistence) |
| `alert` / `prompt` / `confirm` | a Morph dialog component with `morphState` |
| `requestAnimationFrame` | CSS animations/transitions or `morphEffect` + timers |
| `location` / `history` | Morph windows and routing |
| `navigator` / `screen` | window size from `windowConfig` / runtime info |

```tsx
// ✅ State instead of localStorage for in-app data
import { morphShared } from 'morph';

export const [draft, setDraft] = morphShared("");

export default function App() {
  return (
    <div>
      <button onClick={() => setDraft("hello")}>Save</button>
      <div>{draft}</div>
    </div>
  );
}
```

Steps:

1. Find the flagged global and pick its native replacement from the table.
2. For persistence or OS integration with no Morph equivalent, write a small
   `.cpp` helper and import it (see
   [native C++](../guides/native-cpp.md)).
3. Re-run `morph check`.

## Tuning this rule

Do not disable this rule. These globals cannot exist in the binary.

## See also

- [mx-js-member](mx-js-member.md) / [mx-js-method](mx-js-method.md) — unsupported
  members and methods
- [mx-undefined](mx-undefined.md) — names that are simply unknown
- [Async and Fetch](../javascript/async.md)

