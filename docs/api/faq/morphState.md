# morphState FAQ

## Why didn't my component re-render after I updated state?

You probably mutated instead of setting. State updates only happen through the setter with a **new value**:

```tsx
// ✗ No re-render — same array, mutated in place
items.push(newItem)

// ✓ Re-renders — new array identity
setItems([...items, newItem])
```

The same applies to objects: `setUser({ ...user, name: 'Ada' })`, never `user.name = 'Ada'`.

## Direct value or updater function — which setter form?

```tsx
setCount(5)              // direct: you know the next value
setCount(c => c + 1)     // updater: next value depends on the previous one
```

Prefer the updater form inside callbacks, timers, and event handlers, where the `count` variable captured in the closure may be stale. Multiple setters in the same tick are batched into one re-render either way.

## Can state hold objects and arrays?

Yes — any TypeScript type works:

```tsx
const [user, setUser] = morphState<{ name: string; age: number } | null>(null)
const [todos, setTodos] = morphState<string[]>([])
```

Replace, don't mutate (see above).

## Is the initial value used on every render?

No — only on the first render. Later renders keep the current state even if the `morphState(...)` line runs again with a different argument. Don't compute the initial value from props and expect it to "reset" when props change; lift that logic into an effect or key the component instead.

## Can I call the setter during render?

No. Setters belong in event handlers, effects, and callbacks. Calling one unconditionally during render schedules another render while rendering — the linter/runtime will reject or loop. Derive values directly instead:

```tsx
// ✗ Setter during render
if (count > 10) setCount(10)

// ✓ Derived during render, no setter needed
const display = Math.min(count, 10)
```

## Why must hooks run in the same order every render?

State slots are matched to `morphState` calls by position, so early returns or conditional hooks shift every slot after them:

```tsx
// ✗ Conditional hook — breaks slot matching
if (ready) {
  const [x, setX] = morphState(0)
}

// ✓ All hooks first, conditions after
const [x, setX] = morphState(0)
return (
  <div>
    {ready && <text>{x}</text>}
  </div>
)
```

## Does each component instance get its own state?

Yes. Two `<Counter />` on one page each run their own `morphState` and never share a `count`. (Exception: components rendered inside a `.map()` list template currently share one state slot per template — per-instance state in lists is planned.) For deliberately shared state, see [`morphShared`](../morphShared.md).
