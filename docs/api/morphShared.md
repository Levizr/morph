# morphShared

Creates a reactive module-scoped store shared across all components that import it. No string keys — identity comes from the exporting module path + binding name.

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
Identity = "absolute/path/to/cart.ts::count"
```

- Import `count` from `./cart` in multiple files → **same signal**
- Create another `morphShared` named `count` in `other.ts` → **different signal**
- Import `count` from both `cart.ts` and `other.ts` in same file → **ambiguity error**

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

1. At module scope, `morphShared<T>(initial)` creates a `Signal<T>` in the C++ runtime with a stable identity derived from the module path + binding name.
2. The getter/setter pair are exported; importing them gives direct access to that signal.
3. Reading the getter in a component subscribes that component to the signal.
4. Calling the setter marks the signal dirty, re-rendering all subscribers.
5. Multiple setters in the same tick are batched.

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

For app-wide state not tied to a specific UI component.

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