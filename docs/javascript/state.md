# State

`morphState` creates reactive state that triggers re-renders when updated.

## Basic Usage

```tsx
import { morphState } from 'morph'

export default function App() {
  const [count, setCount] = morphState(0)

  return (
    <body>
      <div>Count: {count}</div>
      <button onClick={() => setCount(count + 1)}>+1</button>
    </body>
  )
}
```

`morphState(initial)` returns a tuple `[state, setState]`:

- **`state`** — the current value (read-only)
- **`setState(value)`** — updates the value and triggers a re-render

## Setting State

Pass a direct value:

```tsx
setCount(5)
setName("hello")
```

Or pass an updater function:

```tsx
setCount(prev => prev + 1)
```

## Multiple State Variables

Each `morphState` call is independent. Use multiple for different pieces of state:

```tsx
const [name, setName] = morphState("")
const [age, setAge] = morphState(0)
const [active, setActive] = morphState(false)
```

## State in Event Handlers

State updates are batched. Clicking a button that calls multiple setters will re-render once:

```tsx
function reset() {
  setName("")
  setAge(0)
  setActive(false)
  // re-renders once with all three values updated
}
```

## Typed State

TypeScript annotations work naturally:

```tsx
const [count, setCount] = morphState<number>(0)
const [name, setName] = morphState<string>("")
const [items, setItems] = morphState<string[]>([])
```

The type is usually inferred from the initial value:

```tsx
const [count, setCount] = morphState(0)    // inferred as number
const [open, setOpen] = morphState(true)   // inferred as boolean
```

## State in JSX Expressions

Use state values anywhere in JSX:

```tsx
<div className={active ? "active" : "inactive"}>
  {count > 0 && <span>Positive</span>}
  <div style={{ width: bodyWidth }}>
    {loading ? "Loading..." : "Done"}
  </div>
</div>
```

## State with Conditional Rendering

```tsx
const [showDetails, setShowDetails] = morphState(false)

return (
  <div>
    <button onClick={() => setShowDetails(!showDetails)}>
      Toggle
    </button>
    {showDetails && <div>Details here</div>}
  </div>
)
```
