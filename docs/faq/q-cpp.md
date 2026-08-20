# Do I Need to Know C++ to Use Morph?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

No.

You write `.mx` files — JSX-like syntax with TypeScript/JavaScript logic and CSS styling:

```tsx
function App() {
  const [count, setCount] = morphState(0)
  return (
    <div style={styles.container}>
      <p>Count: {count}</p>
      <button onClick={() => setCount(count + 1)}>+1</button>
    </div>
  )
}
```

That's all you write. The compiler turns it into C++ and calls `g++` for you. C++ is an implementation detail, not a requirement.

If you *do* know C++, you can go deeper:

- **Custom native nodes** — add your own C++ rendering/interaction primitives ([Custom C++ Nodes](../guides/custom-cpp-nodes.md))
- **C++ / JSX interop** — call C++ functions from component logic ([Native C++ Interop](../guides/native-cpp.md))
- **Peeking under the hood** — read the generated C++ to understand exactly what your app compiles to

But the core promise is: **web developers build native apps.** No C++ required.