# morphEvent

Creates a typed event channel for fire-and-forget messaging between components — **within a window and across windows**. Unlike state, events don't hold a value — they trigger callbacks synchronously on the emitter's thread.

> **Scope:** `morphEvent` channels are **app-global**. They are the cross-window bus: an `emit` in one window reaches subscribers in every open window. (Companion rule: [`morphShared`](morphShared.md) is per-window isolated.)

## Basic Usage

```tsx
// cart-events.ts
import { morphEvent } from 'morph'

export const cartUpdated = morphEvent<{ delta: number }>()
```

```tsx
// AddButton.mx
import { cartUpdated } from './cart-events'

export function AddButton() {
  return (
    <button onClick={() => cartUpdated.emit({ delta: 1 })}>
      Add to Cart
    </button>
  )
}
```

```tsx
// CartBadge.mx
import { cartUpdated } from './cart-events'

export function CartBadge() {
  cartUpdated.on((payload) => {
    console.log('Cart changed by', payload.delta)
  })
  return <text>Cart</text>
}
```

## API

### `morphEvent<Payload>()`

Creates an event channel. The generic type parameter defines the payload shape.

```tsx
export const userLoggedIn = morphEvent<{ userId: string; timestamp: number }>()
export const toastEvent = morphEvent<{ message: string; type: 'info' | 'error' }>()
export const simpleEvent = morphEvent<void>() // no payload
```

Returns an object with two methods:

| Method | Signature | Description |
|--------|-----------|-------------|
| `.emit(payload)` | `(payload: Payload) => void` | Fires the event, calling all subscribers synchronously |
| `.on(handler)` | `(handler: (payload: Payload) => void) => void` | Subscribes a handler; returns void |

### `.emit(payload)`

Synchronously invokes all handlers registered with `.on()`. Use inside event handlers, effects, or callbacks.

```tsx
function handleSubmit() {
  // Valid: called inside a callback
  formSubmitted.emit({ formId: 'login', success: true })
}
```

### `.on(handler)`

Registers a handler. **Must be called at component top level** (not inside callbacks, loops, or conditionals). The handler receives the payload as its only argument.

```tsx
export function NotificationBell() {
  // ✓ Correct: top-level, inline arrow
  toastEvent.on((payload) => {
    console.log(payload.message, payload.type)
  })

  // ✗ Wrong: inside a callback
  // button.onClick = () => toastEvent.on(...)

  // ✗ Wrong: conditional
  // if (enabled) toastEvent.on(...)

  return <text>Bell</text>
}
```

## Patterns

### Colocated UI Events (Component-Owned)

The component that displays the UI also owns the event channel. Other components import and emit.

```tsx
// Toast.mx
import { morphEvent, morphState } from 'morph'

export const toastEvent = morphEvent<{ message: string; type: 'success' | 'error' }>()

export default function Toast() {
  const [current, setCurrent] = morphState<{ message: string; type: string } | null>(null)

  toastEvent.on((payload) => {
    setCurrent(payload)
    setTimeout(() => setCurrent(null), 3000)
  })

  return (
    <div>
      {current && <text className={current.type}>{current.message}</text>}
    </div>
  )
}
```

```tsx
// Form.mx
import { toastEvent } from './Toast'

export function Form() {
  const save = () => {
    toastEvent.emit({ message: 'Saved!', type: 'success' })
  }
  return <button onClick={save}>Save</button>
}
```

### Centralized System Events (Domain-Owned)

For app-wide events not tied to a specific component, create a dedicated file.

```tsx
// authEvents.ts
import { morphEvent } from 'morph'

export const loginEvent = morphEvent<{ userId: string }>()
export const logoutEvent = morphEvent<{ reason: 'timeout' | 'manual' }>()
export const sessionExpiredEvent = morphEvent<void>()
```

```tsx
// Header.mx
import { logoutEvent } from './authEvents'

export function Header() {
  return <button onClick={() => logoutEvent.emit({ reason: 'manual' })}>Log Out</button>
}
```

```tsx
// SessionMonitor.mx
import { sessionExpiredEvent } from './authEvents'
import { morphState } from 'morph'

export function SessionMonitor() {
  const [expired, setExpired] = morphState(false)

  sessionExpiredEvent.on(() => setExpired(true))

  return (
    <div>
      {expired && <text>Session expired — please log in again</text>}
    </div>
  )
}
```

## Type Safety

The payload type is enforced at compile time:

```tsx
// ✓ Valid
cartUpdated.emit({ delta: 5 })

// ✗ Error: missing required field 'delta'
cartUpdated.emit({})

// ✗ Error: wrong type
cartUpdated.emit({ delta: 'five' })

// ✓ Valid handler
cartUpdated.on((payload) => {
  // payload.delta is number
  console.log(payload.delta.toFixed(2))
})
```

## Rules & Linting

| Code | Trigger | Fix |
|------|---------|-----|
| `mx-event-scope` | `morphEvent` called inside a component body | Move to module scope (top level) |
| `mx-api-removed` | Using old string-channel APIs (`morphOn`, `morphEmit`, `channel.emit`) | Migrate to `morphEvent` |

## Cross-Window Communication

Because `morphShared` is per-window, events are the way windows talk to each other. Pair them: write the window-local value, then broadcast so siblings catch up.

```tsx
// themeStore.ts
import { morphShared, morphEvent } from 'morph'

export const [theme, setTheme] = morphShared<'light' | 'dark'>('light')
export const themeChanged = morphEvent<'light' | 'dark'>()

// Header.mx (any window)
import { theme, setTheme, themeChanged } from './themeStore'
import { morphState } from 'morph'

export function Header() {
  themeChanged.on((next) => setTheme(next)) // keep this window in sync
  return <button onClick={() => {
    const next = theme === 'light' ? 'dark' : 'light'
    setTheme(next)          // what (this window, persists)
    themeChanged.emit(next) // when (all windows, notifies)
  }}>Theme: {theme}</button>
}
```

Rules of thumb:

- `morphShared` alone → window-local (two windows diverge).
- `morphEvent` alone → transient nudge, no readable value for late windows.
- Both together → every window holds its own copy and stays in sync.

## How It Works

- **Identity**: The event's identity is `modulePath::bindingName` (e.g., `src/events/cartEvents.ts::cartUpdated`). Importing the same binding from the same module always refers to the same channel.
- **Scope**: One channel per app, shared by all windows. Subscriptions from every window fire on each `emit`.
- **Synchronous**: Handlers run immediately on the emitter's thread, before `.emit()` returns.
- **No persistence**: Events don't store values. Late subscribers miss prior emissions — pair with `morphShared` if you need a "last value" semantic.
- **Lifetime**: Subscriptions are registered once at startup and live for the app lifetime (hot reload clears and re-registers them). There is no `.off()` in v1 — if a subscribing component can unmount while emitters still fire, guard inside the handler.