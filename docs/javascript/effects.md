# Effects

`morphEffect` registers a side effect that runs after render and re-runs when dependencies change.

## Basic Usage

```tsx
import { morphEffect, morphState } from 'morph'

export default function App() {
  const [count, setCount] = morphState(0)

  morphEffect(() => {
    console.log("Count changed:", count)
  })

  return (
    <body>
      <div>Count: {count}</div>
      <button onClick={() => setCount(count + 1)}>+1</button>
    </body>
  )
}
```

The effect runs once after the initial render, then re-runs every time `count` changes.

## Dependencies

Pass a dependency array as the second argument:

```tsx
morphEffect(() => {
  console.log("name or age changed:", name, age)
}, [name, age])
```

- **No array** — runs after every render
- **Empty array `[]`** — runs once after initial render only
- **With deps** — runs after initial render, then when any dep changes

## Cleanup

Return a cleanup function from the effect:

```tsx
morphEffect(() => {
  const timer = setInterval(() => {
    console.log("tick")
  }, 1000)

  return () => {
    clearInterval(timer)
  }
}, [])
```

The cleanup runs before the effect re-runs and when the component unmounts.

## Common Patterns

### Log state changes

```tsx
morphEffect(() => {
  console.log("State:", { count, name, active })
})
```

### Side effect on mount

```tsx
morphEffect(() => {
  fetchInitialData()
}, [])  // empty deps = run once
```

### Respond to specific changes

```tsx
morphEffect(() => {
  if (userId) {
    loadUserProfile(userId)
  }
}, [userId])  // only re-runs when userId changes
```

### Cleanup timers

```tsx
morphEffect(() => {
  const id = setInterval(poll, 5000)
  return () => clearInterval(id)
}, [])
```

## Implementation Notes

- Effects are implemented via `create_effect` in the C++ runtime (`runtime/cpp/reactivity/signal.h`)
- Each effect tracks its dependencies by reading signal values during execution
- When a dependency signal changes, the effect is scheduled to re-run on the next frame
- Cleanup functions are stored and called before the next effect execution or on unmount
- Effects run after layout + paint, so DOM measurements are accurate