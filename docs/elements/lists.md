# List Rendering

Morph supports rendering collections with JSX `.map()` expressions. Lists are reactive: when the state array changes, only the affected items are updated (keyed reconciliation).

## Basic List

Render an array of items with `{array.map(item => <JSX>)}`:

```tsx
import { morphState } from 'morph'

export default function App() {
  const [items, setItems] = morphState(["a", "b", "c"])

  return (
    <div>
      {items.map(item => <div>{item}</div>)}
    </div>
  )
}
```

Each item in the array produces one element. The map body must be a JSX element (wrap multiple elements in a fragment):

```tsx
{items.map(item => (
  <>
    <span>{item.name}</span>
    <span>{item.price}</span>
  </>
))}
```

## Keyed Lists

Add a `key` prop to help Morph match items across updates:

```tsx
const [items, setItems] = morphState([
  { id: 1, name: "Apple" },
  { id: 2, name: "Banana" },
])

<div>
  {items.map(item => <div key={item.id}>{item.name}</div>)}
</div>
```

Keys let Morph reuse existing nodes when items are inserted, removed, or reordered — the items that didn't change are not re-created. Without a `key`, items are matched by position.

> **Note:** `key` must be a string or number. `key={item.id}` and `key={item.name}` both work.

## Adding Items

State arrays are immutable. Create a new array with spread syntax:

```tsx
<button onClick={() => setItems([...items, { id: items.length, name: "Item" + items.length }])}>
  Add
</button>
```

`[...items, x]` appends `x` to the end of the array.

## Removing Items

Use spread plus `slice`:

```tsx
// remove the last item
<button onClick={() => setItems([...items].slice(0, -1))}>
  Remove
</button>

// remove the first item
<button onClick={() => setItems([...items].slice(1))}>
  Remove First
</button>
```

`slice(start, end)` supports negative indices — `slice(0, -1)` keeps everything except the last element.

## Updating Items

Map to a new array:

```tsx
// mark a single item done
<button onClick={() =>
  setItems(items.map(item =>
    item.id === id ? { ...item, done: true } : item
  ))
}>
  Mark Done
</button>
```

## Conditionals Inside Items

Per-item conditionals work inside the map body:

```tsx
const [show, setShow] = morphState(true)

<div>
  <button onClick={() => setShow(!show)}>Toggle</button>
  {items.map(item => (
    <div key={item.id}>
      {item.name}
      {show ? <span> +</span> : null}
    </div>
  ))}
</div>
```

`show ? <span> +</span> : null` renders the `<span>` only when `show` is true, and removes it when false. The `&&` form also works:

```tsx
{item.done && <span> Done</span>}
```

## Empty Lists

An empty array renders nothing:

```tsx
const [items, setItems] = morphState([])
```

For an empty-state message, use a conditional alongside the list:

```tsx
{items.length === 0 && <div>No items yet</div>}
{items.map(item => <div key={item.id}>{item.name}</div>)}
```

## Complete Example

```tsx
import { morphState } from 'morph'

export default function App() {
  const [items, setItems] = morphState([])
  const [show, setShow] = morphState(true)

  return (
    <div>
      <button onClick={() => setItems([...items, { id: items.length, name: "Item" + items.length }])}>Add</button>
      <button onClick={() => setItems([...items].slice(0, -1))}>Remove</button>
      <button onClick={() => setShow(!show)}>Toggle</button>
      {items.map(item => (
        <div key={item.id}>
          {item.name}
          {show ? <span> +</span> : null}
        </div>
      ))}
    </div>
  )
}
```

## How It Works

- The map expression is compiled into a keyed list container in the runtime.
- When the array changes, Morph reconciles the container: nodes for unchanged keys are reused, new keys create new nodes, and missing keys are removed.
- Conditionals and text bindings inside items are updated individually when their state changes, without re-creating the whole list.
- Items are laid out top-to-bottom in document order (block flow).