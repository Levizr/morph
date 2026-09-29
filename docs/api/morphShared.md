# morphShared

Creates a reactive module-scoped store shared across all components **within the same window** that import it. No string keys — identity comes from the exporting module path + binding name.

> **Window scope:** each window gets its own isolated copy of every `morphShared` store. Two windows showing the same route do **not** see each other's values. For cross-window communication, use [`morphEvent`](morphEvent.md) — events are the app-wide bus.

## Signature

```tsx
function morphShared<T>(initialValue: T): [T, (value: T | ((prev: T) => T)) => void]
```

## Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `initialValue` | `T` | Initial state value. |

## Returns

Tuple of `[getter, setter]`:

| Element | Type | Description |
|---------|------|-------------|
| `getter` | `T` | Current value (read-only, reactive) |
| `setter` | `(value: T \| (prev: T) => T) => void` | Updater function |

## Usage

```tsx
// cart.ts
import { morphShared } from 'morph'

export const [count, setCount] = morphShared<number>(0)
```

```tsx
// CartBadge.mx
import { count } from './cart'

export function CartBadge() {
  return <text>Cart: {count}</text>
}
```

```tsx
// AddButton.mx
import { setCount } from './cart'

export function AddButton() {
  return <button onClick={() => setCount(c => c + 1)}>Add</button>
}
```

## Rules

| Rule | Enforcement |
|------|-------------|
| Must be at module scope (top level) | `mx-shared-scope` error |
| Must be exported | `mx-shared-scope` error |
| Cannot be inside a component | `mx-shared-scope` error |
| Single initial value (no string key) | `mx-api-removed` error |

## Identity Model

```
Identity = "absolute/path/to/cart.ts::count"  (+ owning window at runtime)
```

- Import `count` from `./cart` in multiple files → **same signal within one window**
- Create another `morphShared` named `count` in `other.ts` → **different signal**
- Import `count` from both `cart.ts` and `other.ts` in same file → **ambiguity error**
- Open the same route in two windows → **two independent copies** of `count`

## Window Scope

| Situation | Behavior |
|-----------|----------|
| Two components in the same window import the same store | Same value, both re-render on `set` |
| Two windows open the same route | Independent values — editing in one never touches the other |
| `win.navigate()` to another route and back (cache `0`) | Store re-initializes on fresh mount; page-cache restores keep the window's copy |
| Closing a window | Its copies are destroyed with it — no leak into other windows |

```tsx
const a = new Window("/shop", { data: { userId: 1 } })
const b = new Window("/shop", { data: { userId: 42 } })
// cart in `a` and cart in `b` are fully independent.
```

Need genuinely app-wide data (theme, auth session)? Write it to a store **and** broadcast the change over `morphEvent` so every window updates its own copy — see [Event + shared pattern](morphEvent.md#cross-window-communication). A dedicated app-global store API is intentionally deferred until that pattern proves insufficient.

## Why per-window isolation? Why not global?

Short answer: a window is an **instance**, not a view. `new Window("/shop")` twice must behave like opening two tabs — same code, independent values. Global `morphShared` would break that contract silently.

What breaks if shared were global:

1. **Cross-talk by default.** Two `/shop` windows share one cart; typing a draft in one window overwrites the other. Auth, forms, modal flags, tab selection — all leak. The bug is invisible at the call site (`setCart(...)` looks local) and only reproduces with two windows open.
2. **Lifetime has no owner.** If window A writes and window B reads, whose close destroys the value? Global stores outlive every window, so closed-window state lingers and reopens stale. Per-window stores die with their window — close is cleanup, not a leak decision.
3. **Navigation and cache stop making sense.** `navigate()` destroys page state but must keep window state; with global stores there is no "window state" left to reason about — everything is app state, and the page-cache rules in `windows-and-routing.md` become special cases.
4. **Opt-in vs opt-out.** Cross-window sync should be explicit (`emit` + `on`), not the default side effect of every setter. Isolation makes sharing a conscious line of code; global makes privacy impossible.

That is why the split is `morphShared` (noun, per-window: *what this window holds*) + `morphEvent` (verb, app-global: *what just happened everywhere*). Events carry no value, so late windows miss nothing they were owed — and when they do need the value, the emitter writes its own window's store and broadcasts, letting each sibling update its own copy.

### Open question — we want your suggestion

Per-window `morphShared` + app-global `morphEvent` is the current design, but window scoping is still settling. One idea on the table: let the declaration say what it means, so global is always explicit and never the accident:

```tsx
// Proposal only — not implemented. Default stays per-window.
export const [cart, setCart] = morphShared(0)          // isolated (today, unchanged)
export const [theme, setTheme] = morphShared.global('light') // app-wide (proposed)
```

Same identity model either way (`modulePath::bindingName`); only the runtime scope differs — one signal per window vs one signal for the app. The `.global` spelling is deliberate: today's single-argument form keeps working untouched, and `global` at the call site makes cross-window sharing impossible to miss in review.

If this split fits your app — or fights it — tell us:

- Is per-window isolation what you expected, or did you want a global store?
- Would `morphShared.global(...)` (or a similar scope hint) help, or does event + shared cover you?
- What are you building that needs cross-window state?

See [Suggestions](../future/tooling/suggestions.md) for how to reach us — one paragraph (what you're building, what's missing, what you tried) is enough. Real apps decide this API.

```tsx
// ✓ Same identity — imports from same module
import { count } from './cart'
import { setCount } from './cart'

// ✗ Ambiguity error — same local name from different modules
import { count } from './cart'
import { count } from './other' // Error: "ambiguous import 'count'"
```

## TypeScript

```tsx
// Explicit type
export const [theme, setTheme] = morphShared<'light' | 'dark'>('light')

// Inferred
export const [count, setCount] = morphShared(0)          // number
export const [name, setName] = morphShared('')           // string
export const [items, setItems] = morphShared<string[]>([]) // string[]
```

## Updater Forms

```tsx
// Direct value
setCount(5)

// Updater function
setCount(prev => prev + 1)
```

## How It Works

1. At module scope, `morphShared<T>(initial)` declares a store with a stable identity derived from the module path + binding name.
2. At runtime each **window owns one `Signal<T>` per store** — the declaration is shared, the value is per-window.
3. The getter/setter pair are exported; importing them gives direct access to the current window's signal.
4. Reading the getter in a component subscribes that component to its window's signal.
5. Calling the setter marks that window's signal dirty, re-rendering subscribers **in that window only**.
6. Multiple setters in the same tick are batched.

## Patterns

### Colocated State (Component-Owned)

State lives with the component that primarily owns it.

```tsx
// Modal.mx
import { morphShared } from 'morph'

export const [isOpen, setIsOpen] = morphShared(false)

export default function Modal() {
  return (
    <div>
      {isOpen && (
        <div className="modal">
          <text>Dialog open</text>
          <button onClick={() => setIsOpen(false)}>Close</button>
        </div>
      )}
    </div>
  )
}
```

```tsx
// Trigger.mx
import { setIsOpen } from './Modal'

export function Trigger() {
  return <button onClick={() => setIsOpen(true)}>Open Modal</button>
}
```

### Centralized Store (Domain-Owned)

For window-wide state not tied to a specific UI component (one copy per window).

```tsx
// themeStore.ts
import { morphShared } from 'morph'

export const [theme, setTheme] = morphShared<'light' | 'dark'>('light')
export const [user, setUser] = morphShared<User | null>(null)
```

```tsx
// Header.mx
import { theme, setTheme } from './themeStore'

export function Header() {
  return <button onClick={() => setTheme(t => t === 'light' ? 'dark' : 'light')}>
    Theme: {theme}
  </button>
}
```