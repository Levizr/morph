# morphEffect

Registers a side effect that runs after component render and re-runs when dependencies change. Analogous to React's `useEffect` but with Morph's reactive model.

## Signature

```tsx
function morphEffect(effect: () => void | (() => void), deps?: readonly any[]): void
```

## Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `effect` | `() => void` | Effect body function. Can return a cleanup function. |
| `deps` | `readonly any[]` | Optional dependency array. `[]` runs once after the first render. With deps, re-runs when a listed dependency changes. When omitted, subscribes to signals read in the body. |

## Returns

`void`

## Basic Usage (once-only)

```tsx
import { morphEffect } from 'morph'

export default function Component() {
  morphEffect(() => {
    // Empty deps = runs once after first render
    console.log("Component mounted")
  }, [])

  return <div>Hello</div>
}
```

## Usage (with dependencies)

```tsx
import { morphEffect, morphState } from 'morph'

export default function Counter() {
  const [count, setCount] = morphState(0)

  morphEffect(() => {
    // Runs after render, and re-runs when count changes
    console.log("Count changed:", count)
  }, [count])

  return (
    <div>
      <text>Count: {count}</text>
      <button onClick={() => setCount(count + 1)}>Increment</button>
    </div>
  )
}
```

## Cleanup Pattern

If the effect returns a function, it runs on cleanup (before re-run or unmount):

```tsx
import { morphEffect, morphState } from 'morph'

export default function Timer() {
  const [seconds, setSeconds] = morphState(0)

  morphEffect(() => {
    const id = setInterval(() => setSeconds(s => s + 1), 1000)
    // Cleanup: clear interval on unmount
    return () => clearInterval(id)
  }, [])

  return <text>{seconds}s</text>
}
```

## Rules

| Rule | Enforcement |
|------|-------------|
| Must be called inside a component function | convention (no scope lint in v1) |
| Must be called on every render in same order | convention (positional matching) |
| First argument must be a function | `mx-effect-cb` error |
| Deps should be state values | `mx-effect-deps` warning |

## How It Works

1. `morphEffect(fn, deps?)` creates a `morph::create_effect` in the C++ runtime.
2. If `deps` is `[]`, the effect runs once after the first render and never re-runs.
3. If `deps` lists state getters, the effect re-runs only when a listed dependency changes — other signals read in the body do not re-trigger it. If `deps` is omitted, the effect subscribes to whatever signals the body reads.
4. When any dependency changes, the effect is re-scheduled to run after the next render.
5. The cleanup function (if returned) runs before re-run or unmount, in a thread-local active context.
6. All effect bodies execute in the morph reactivity thread, coordinated with signal updates.

## Common Patterns

### Effect with State Sync

```tsx
const [theme, setTheme] = morphShared<'light' | 'dark'>('light')

morphEffect(() => {
  console.log("Theme changed:", theme)
}, [theme])
```

### Fetch on a changing id

```tsx
morphEffect(() => {
  if (userId) {
    loadUserProfile(userId)
  }
}, [userId])  // only re-runs when userId changes
```