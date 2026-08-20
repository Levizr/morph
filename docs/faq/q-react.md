# Is Morph React? Can I Use React?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

It's **React-inspired** — but it's not React, and you can't drop in the React library.

Morph shares the ideas that made React popular: components, JSX, state, effects, declarative UI:

```tsx
function App() {
  const [count, setCount] = morphState(0)   // just like useState
  morphEffect(() => { ... })                 // just like useEffect
  return <button onClick={() => setCount(count + 1)}>{count}</button>
}
```

But the machinery underneath is different in a fundamental way. React's core is the **virtual DOM** — a JS representation of the UI that reconciles changes against the real DOM. Morph has no DOM and no virtual DOM. The layout tree *is* the real thing:

- **No reconciliation** — state changes mark exactly the affected nodes in the layout tree and re-render them
- **No diffing** — you don't diff two trees; you invalidate and redraw the changed nodes directly
- **No fibers** — the runtime is native; scheduling and rendering are the runtime's job, not a JS scheduler's

So: if you know React, you'll feel at home with the primitives. But it's a native system wearing familiar clothes — not React ported to the desktop.