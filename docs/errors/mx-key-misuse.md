# mx-key-misuse — key Used Outside a List

**Severity:** warning | **Blocks `morph build`:** no

> **Status:** documented rule — `morph check` does not report this code yet. The underlying problem still breaks your build or your UI; the cause and fix below apply when you hit it.

## What this error means

A `key` prop appears on an element that is **not** the root of a `.map()`
list template:

```
warning : mx-key-misuse : `key` is only meaningful inside `.map()` lists
  hint: Remove `key` here, or move it to the list item root
  Learn more: https://morph.levizr.com/docs/errors/mx-key-misuse
```

This is a **warning**: the build proceeds and `key` is ignored outside lists.

## Why Morph raises it

`key` exists for one job: letting list reconciliation match old items to new
items when an array changes (see [mx-list-key](mx-list-key.md)). Outside a
`.map()` template there is no reconciliation — nothing ever compares keys —
so the prop does nothing. Its presence almost always means the author thought
`key` was passed to the component as data (it is not — `key` is stripped
before props, like React) or pasted it in the wrong place.

## Example that triggers it

```tsx
// ⚠️ key does nothing here — mx-key-misuse
export default function App() {
  return (
    <div>
      <button key="save-btn" onClick={() => console.log("hi")}>Save</button>
    </div>
  );
}
```

## How to fix

Remove the stray `key`, or move it where reconciliation actually runs:

```tsx
// ✅ No key outside lists
export default function App() {
  return (
    <div>
      <button onClick={() => console.log("hi")}>Save</button>
    </div>
  );
}
```

```tsx
// ✅ key belongs on the list item root
export default function App() {
  const items = ["a", "b"];
  return (
    <div>
      {items.map((item) => (
        <div key={item}>{item}</div>
      ))}
    </div>
  );
}
```

If you need an identifier inside the component, pass a real prop
(`id="save-btn"`) — `key` never reaches component code.

Steps:

1. Delete `key` from non-list elements, replacing it with `id` or a custom
   prop if you needed the value.
2. Keep `key` only on the root element inside `.map()` callbacks.
3. Re-run `morph check`.

## Tuning this rule

Safe to suppress per-file while prototyping, but the warning is pointing at
dead markup — removing it is better:

```json
{
  "lint": {
    "disable": ["mx-key-misuse"]
  }
}
```

## See also

- [mx-list-key](mx-list-key.md) — the reverse: list *missing* its `key`
- [How to Render Lists](../elements/lists.md)

