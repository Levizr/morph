# Why JSX (and Web Syntax)

**Part of:** [The Story of Morph](index.md)

The next question is the UI syntax itself: why describe interfaces with **JSX** — a web thing — instead of an Android-like XML, a Qt-like system, or a custom DSL?

## Why web syntax at all

**Because you already know it.**

If you're building software, the probability is high that you know HTML, CSS, and JavaScript at least a little. That was the whole bet: when you open a Morph file, you're already home.

- **Android-like** (XML layouts + Java/Kotlin) — a separate markup language, separate logic language, separate mental model
- **Qt-like** (QML, or widget code in C++) — powerful, but its own universe with its own learning curve
- **Custom DSL** — I'd be inventing a language nobody knows, for no benefit

Every one of those means *learning a new UI language from scratch* before you can build anything. Web syntax means the UI language is already in your head. The learning curve for Morph is nearly zero if you've written any web code.

## Why JSX specifically

JSX is React's syntax — and it's the best developer experience I've found. I've used a lot of GUI frameworks, and none of them feel as direct as JSX. The reason is one thing above all:

**You don't tweak elements to change them.**

In most GUI frameworks, changing a value on screen is a hunt: find the element, call the setter, refresh the view, keep it in sync. With JSX plus state, you just *place a variable anywhere in the markup and it works*:

```tsx
function Counter() {
  const [count, setCount] = morphState(0)   // just like useState
  return (
    <button onClick={() => setCount(count + 1)}>
      {count}              {/* the value, just dropped in */}
    </button>
  )
}
```

No `element.setText(...)`. No manual refresh. No find-the-widget choreography. Declare the state, drop the variable into the JSX where you want it — `setCount(...)` updates it and the framework re-renders. That one property — *the UI updates itself because the value is just there* — is the biggest DX win I've ever had in a UI framework.

## Everything in one place

JSX also keeps a component whole. The structure is HTML-like markup, the logic is JavaScript/TypeScript in the same file, and styling can be inline — style objects or CSS right next to the element:

```tsx
function Card({ title, body }) {
  return (
    <div style={{ padding: "16px", borderRadius: "8px", background: "#fff" }}>
      <h2>{title}</h2>
      <p>{body}</p>
    </div>
  )
}
```

One component = one file = one mental unit. You don't jump between a layout file, a logic file, and a stylesheet to understand what a screen does.

## And it compiles

The best part for Morph specifically: JSX is just syntax. It's not a runtime — it's a description the compiler can read, turn into an IR, and compile down to a native node tree. Morph keeps the *best developer experience in the ecosystem* and compiles it away into a lean native binary. Web comfort on the outside, native performance on the inside.