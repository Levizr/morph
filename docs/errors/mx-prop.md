# mx-prop — Invalid or Misspelled Prop

**Severity:** warning | **Blocks `morph build`:** no

## What this error means

A prop on a native element is not valid for it. Three shapes exist:

```
warning : mx-prop : Use `className` instead of `class` on <div>
  hint: Replace `class` with `className`
```

```
warning : mx-prop : Unknown prop `onClik` on <button>
  hint: Check event name (onClick, onInput, etc.)
```

```
warning : mx-prop : Unknown prop `backgroudColor` on <div>
  hint: Did you mean `backgroundColor`?
```

This is a **warning**: the build proceeds, but the prop may be ignored — your
UI will not look or behave as written.

## Why Morph raises it

Props compile to style fields, layout parameters, or event wirings on the
native node. An invalid prop has no compilation target. Because JSX cannot know
whether `backgroudColor` was a style typo or a custom attribute, Morph warns
with the closest valid name instead of silently dropping it.

Two frequent causes:

- **`class` instead of `className`** — Morph follows the React convention:
  `class` is a reserved word in JS/TS, so the prop is `className`. See
  [mx-dup-class](mx-dup-class.md) for the both-at-once case.
- **Wrong event name** — Morph supports `onClick`, `onDoubleClick`,
  `onMouseDown`, keyboard events, and more (see
  [events](../elements/events.md)). `onClik`, `onclick` (lowercase c), or DOM
  names Morph does not implement (`onMouseOver`) all warn.

`data-*` and `aria-*` attributes are always allowed and never warn.

## Example that triggers it

```tsx
// ❌ class + misspelled handler — mx-prop (twice)
export default function App() {
  return (
    <div class="card">
      <button onClik={() => console.log("hi")}>Save</button>
    </div>
  );
}
```

## How to fix

```tsx
// ✅ className + correct event name
export default function App() {
  return (
    <div className="card">
      <button onClick={() => console.log("hi")}>Save</button>
    </div>
  );
}
```

Steps:

1. For `class`, rename to `className` (or convert everything to `class` —
   never mix; see [mx-dup-class](mx-dup-class.md)).
2. For `on*` props, check the [events list](../elements/events.md) for the
   exact name and casing.
3. For anything else, read the `Did you mean ... ?` hint.
4. Re-run `morph check`.

## Tuning this rule

Promote to error in CI to catch prop typos early:

```json
{
  "lint": {
    "severities": { "mx-prop": "error" }
  }
}
```

## See also

- [mx-dup-class](mx-dup-class.md) — `class` and `className` together
- [mx-component-prop](mx-component-prop.md) — unknown prop on a component
- [mx-event-value](mx-event-value.md) — handler is not a function
- [How to Handle Events](../elements/events.md)

