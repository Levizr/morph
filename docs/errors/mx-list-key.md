# mx-list-key — List Item Without key

**Severity:** warning | **Blocks `morph build`:** no

## What this error means

A `.map()` list renders its item template without a `key` on the root element:

```
warning : mx-list-key : List rendering without `key` prop — may cause state mismatches
  hint: Add `key={item.id}` to the root element inside `.map()`
  Learn more: https://morph.levizr.com/docs/errors/mx-list-key
```

This is a **warning**: the build proceeds, but reordering, inserting, or
removing items can attach state and effects to the wrong rows.

## Why Morph raises it

List reconciliation matches old native items to new data **by key**. Without
one, Morph falls back to position — so deleting the first of three rows keeps
three rows but shifts every row's state up by one (checkboxes, inputs, and
per-item effects all land on the wrong data). `key` must be stable and unique
per item (an id, not the loop index — indexes have the same positional problem
under reorder). See [rendering lists](../elements/lists.md).

## Example that triggers it

```tsx
// ⚠️ No key — mx-list-key (state mismatches on reorder/delete)
export default function App() {
  const items = ["a", "b", "c"];
  return (
    <div>
      {items.map((item) => (
        <div>{item}</div>
      ))}
    </div>
  );
}
```

## How to fix

```tsx
// ✅ Stable unique key on the item root
export default function App() {
  const users = [{ id: "u1", name: "Ada" }, { id: "u2", name: "Bob" }];
  return (
    <div>
      {users.map((u) => (
        <div key={u.id}>{u.name}</div>
      ))}
    </div>
  );
}
```

Rules:

- Put `key` on the **root** element returned from the `.map()` callback.
- Use a stable id from your data (`item.id`), never the array index.
- `key` is consumed by reconciliation and never passed as a prop (see
  [mx-key-misuse](mx-key-misuse.md)).

Steps:

1. Add `key={item.id}` (or equivalent stable id) to each list root.
2. If your data has no ids, add them at the data source — do not use indexes
   as a permanent fix.
3. Re-run `morph check`.

## Tuning this rule

Commonly disabled while prototyping static lists:

```json
{
  "lint": {
    "disable": ["mx-list-key"]
  }
}
```

Re-enable before shipping any list that mutates or reorders.

## See also

- [mx-key-misuse](mx-key-misuse.md) — `key` outside lists
- [How to Render Lists](../elements/lists.md)

