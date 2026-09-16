# morphEvent FAQ

## Colocated event or central events file — what goes where?

Same ownership rule as [`morphShared`](../morphShared.md): the event lives with whoever *handles* it. The difference from state is that an event has two sides — the listener (owner) and the emitters (consumers) — so ask "who listens?" instead of "who renders?".

**Colocate with the component** when one UI handles the event and others just fire it:

```tsx
// Toast.mx — owns the toast UI, so it owns the channel
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
// Form.mx, Settings.mx, … — any emitter, no ownership
import { toastEvent } from './Toast.mx'

const save = () => {
  toastEvent.emit({ message: 'Saved!', type: 'success' })
}
```

Benefits mirror colocated state: delete the Toast component and its channel goes with it — no orphaned event firing into the void. Discovery is natural: "who handles toasts? the Toast file."

**Extract to an events file** when there is no single handling UI — the event is a system occurrence with several unrelated listeners:

```tsx
// authEvents.ts — no UI; login/logout concern the whole app
import { morphEvent } from 'morph'

export const loginEvent = morphEvent<{ userId: string }>()
export const logoutEvent = morphEvent<{ reason: 'timeout' | 'manual' }>()
```

```tsx
// Header.mx emits; SessionMonitor.mx, Analytics.mx, Cache.mx listen —
// none of them owns "logout", so none of their files should declare it
import { logoutEvent } from './authEvents'
```

Parking a system event inside one listener's component file creates fake ownership: every other listener imports from a component it has nothing to do with, and deleting that component silently orphans the rest. A neutral `.ts` file keeps system events independent of any UI's lifetime.

**Decision rule:** one handling UI → declare the event in that component's file. Multiple unrelated listeners, or listeners that may come and go → central `.ts` events module. Same channel mechanics either way — the only question is which file a future reader will blame when the event misbehaves.

## Why does `morphEvent` exist if `morphShared` already covers cross-component communication?

They solve different problems. `morphShared` is a **noun**: it holds a value, late readers see the latest value, and every write re-renders subscribers. `morphEvent` is a **verb**: it holds nothing, late subscribers miss prior emissions, and emitting never triggers a re-render by itself.

Use shared state for *what things are* (cart total, theme, user) and events for *what just happened* (saved, logged out, toast requested). Crossed over, both misbehave: a transient trigger modeled as shared state re-renders every new subscriber once with a stale value; persistent data modeled as an event is invisible to late components.

## A component mounts after the event already fired and misses it. What now?

By design — events don't persist. If the newcomer needs the last value, the emitter should also write it to a `morphShared` store and the component should read the store. Pattern: events notify *when*, shared state holds *what*.

```tsx
// Emitter does both
const save = () => {
  setLastSaved({ at: Date.now() })          // what (persists)
  savedEvent.emit({ at: Date.now() })       // when (notifies)
}
```

## Why must `.on()` be called at component top level?

Subscriptions are registered once at startup. A conditional or callback-nested `.on()` would subscribe unpredictably — zero times, or once per click, stacking duplicate handlers. Top-level-only keeps subscription count equal to component count. (Same reason hooks have ordering rules.)

## Can one component both emit and listen to the same event?

Yes. Emitting runs all current subscribers synchronously, including the emitter's own handler if it subscribed. Just be careful not to emit from inside your own handler unconditionally — that recurses.

## If I call `.on()` twice for the same event, do both handlers run?

Yes — each `.on()` is an independent subscription, and emit invokes all of them in subscription order. (Order between *different* components' subscriptions follows registration order; don't architect around it — keep handlers independent.)

## In what order do handlers run? Is emit async?

Synchronously, in subscription order, on the emitter's thread — `.emit()` returns after every handler has run. There is no queue and no next-tick deferral. Keep handlers short; heavy work belongs in an effect or behind a state write.

## What if I emit with no subscribers? Do I need to unsubscribe?

Emitting to an empty channel is a safe no-op — no guard needed. And there is nothing to unsubscribe: subscriptions live for the app lifetime (hot reload clears and re-registers them), and there is no `.off()` in v1. The one case to think about is a handler from a component that can unmount while emitters keep firing — guard inside the handler.

## Can an event carry no payload?

Yes: declare `morphEvent<void>()`, emit `undefined`, and subscribe with a parameterless handler:

```tsx
export const ping = morphEvent<void>()
ping.emit(undefined)
ping.on(() => console.log('pong'))
```

## Same event name in two files — collision?

No, for the same reason as shared state: the channel id is module path + event name. `toast.ts::submitted` and `form.ts::submitted` are different channels. Importing both `submitted`s into one file without renaming is an ambiguity error — rename on import.
