# morphState

Creates reactive local state bound to a component instance. Each render of the component gets its own independent state.

## Signature

```tsx
function morphState<T>(initialValue: T): [T, (value: T | ((prev: T) => T)) => void]
```

## Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `initialValue` | `T` | Initial state value. Can be any serializable type. |

## Returns

Tuple of `[state, setState]`:

| Element | Type | Description |
|---------|------|-------------|
| `state` | `T` | Current value (read-only, reactive) |
| `setState` | `(value: T \| (prev: T) => T) => void` | Updater function |

## Usage

```tsx
import { morphState } from 'morph'

export default function Counter() {
  const [count, setCount] = morphState(0)

  return (
    <div>
      <text>Count: {count}</text>
      <button onClick={() => setCount(count + 1)}>Increment</button>
      <button onClick={() => setCount(c => c + 1)}>Increment (fn)</button>
    </div>
  )
}
```

## Updater Forms

```tsx
// Direct value
setCount(5)

// Updater function (receives previous value)
setCount(prev => prev + 1)
```

Both forms trigger a re-render. Updater form is useful when updates depend on the previous value (e.g., in batched callbacks).

## TypeScript

```tsx
// Explicit type
const [count, setCount] = morphState<number>(0)

// Inferred from initial value
const [name, setName] = morphState('')           // string
const [active, setActive] = morphState(false)    // boolean
const [items, setItems] = morphState<string[]>([]) // string[]
```

## Rules

| Rule | Enforcement |
|------|-------------|
| Must be called inside a component function | `mx-state-scope` error |
| Cannot be called at module scope | `mx-state-scope` error |
| Cannot be called in loops/conditionals | Runtime error (hook rules) |
| Must be called on every render in same order | Runtime error (hook rules) |

## How It Works

1. Each `morphState` call allocates a `Signal<T>` in the C++ runtime with a unique ID derived from the component instance and call index.
2. Reading `state` subscribes the current component to the signal.
3. Calling `setState` marks the signal dirty, scheduling a re-render of all subscribed components.
4. Multiple `setState` calls in the same event tick are batched into a single re-render.

## Common Patterns

### Toggle Boolean

```tsx
const [open, setOpen] = morphState(false)
<button onClick={() => setOpen(o => !o)}>{open ? 'Close' : 'Open'}</button>
```

### Form Input

```tsx
const [text, setText] = morphState('')
<input value={text} onChange={e => setText(e.value)} />
```

### Array Updates

```tsx
const [items, setItems] = morphState<string[]>([])
const add = (item: string) => setItems(prev => [...prev, item])
const remove = (index: number) => setItems(prev => prev.filter((_, i) => i !== index))
```