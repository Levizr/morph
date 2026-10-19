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

## Shared module state

Use `morphShared` for state shared across components **within the same window**. Export the binding from
a `.mx`/`.ts`/`.tsx` module, then import its getter/setter wherever it is
used. Each window gets an isolated copy; use `morphEvent` for cross-window
communication:

```tsx
// cart.ts
import { morphShared } from 'morph'

export const [count, setCount] = morphShared<number>(0)
```

```tsx
// CartBadge.tsx
import { count } from './cart'

export function CartBadge() {
  return <text>{count}</text>
}
```

The store identity is the exporting module plus the binding name. The same
getter in another file is a different store, while importing the same local
name from two modules is an ambiguity error. Shared bindings must be declared
at module scope and exported.

### Rules

- Declare `morphShared` at module scope and export it (`mx-shared-scope`).
- Call it as `morphShared<T>(initialValue)`.
- Import the exported getter/setter names exactly.
- Every use of a binding must agree on its type.

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

## Locals Reassigned in the Component Body

Branches and loops written between a component's declarations run before
the UI is built and re-run whenever the state they read changes:

```tsx
const [count, setCount] = morphState(0)

let label = "zero"
if (count > 0) {
  label = "positive"
}

let total = 0
for (const n of [1, 2, 3]) {
  total += n
}

return (
  <div>
    {label} {total}
  </div>
)
```

A `let` that this flow (or a function in the body) reassigns is backed by
a signal: the flow runs in a setup effect that re-seeds it, JSX reads
subscribe to it, and flipping a branch it reads re-renders the text. A
`let` nothing reassigns, and every `const`, stays a plain stack value.

## Implementation Notes

- State is implemented via `Signal<T>` in the C++ runtime (`runtime/cpp/reactivity/signal.h`)
- Each `morphState` call creates a new signal with a unique ID
- Setting state marks dependent nodes as dirty; layout + paint runs on the next frame
- Batching: multiple `setState` calls in the same event handler tick are coalesced into a single re-render