# What Is Reactivity in Morph?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

Reactivity is what makes a UI update itself. You write a variable, the screen changes — with no manual wiring telling it to. This page explains what reactivity means in Morph, how it's implemented, what the alternatives were, and why this design was chosen.

## What "reactivity" means

Without reactivity, updating a UI is manual: you change a value, then you *find* the widgets that show it and update them one by one. Miss one, and the UI lies about the state — the most common bug in non-reactive apps.

Reactive systems invert this. You say *what* depends on *what* — a text element depends on a counter, a button's enabled state depends on a form field — and the system keeps that wiring for you. When the counter changes, every dependent updates automatically. The name comes from React's own logic: it *reacts* to state changes.

## What reactivity looks like in Morph

```tsx
function Counter() {
  const [count, setCount] = morphState(0)     // a signal, not a plain variable
  return (
    <body>
      <div>You clicked {count} times</div>     // this expression subscribes
      <button onClick={() => setCount(count + 1)}>Click</button>
    </body>
  )
}
```

`setCount(count + 1)` runs, and the `<div>` re-renders with the new number. Nothing in your code says "update the div" — the subscription is automatic. One line of state, one expression in the markup, and the dependency is created for you.

## How it's implemented

Reactivity in Morph is a chain of four native pieces. Each one is a real file you can open:

### 1. Signals — `morphState` is a `Signal<T>`

`morphState` compiles down to a `morph::Signal<T>` ([`reactivity/signal.h`](https://github.com/Levizir/morph/blob/main/runtime/cpp/reactivity/signal.h)). A signal holds a value and a list of subscriber effects. In production builds the signals are declared directly in the generated main ([`codegen/templates/app_main.cpp.tera`](https://github.com/Levizir/morph/blob/main/crates/morph-codegen/templates/app_main.cpp.tera)); in dev mode they live in a name-keyed `SignalStore` so they survive hot-reload ([`dev/signal_store.h`](https://github.com/Levizir/morph/blob/main/runtime/cpp/dev/signal_store.h)).

### 2. Effects — `morphEffect` is a `create_effect`

Every expression in your JSX and every `morphEffect` body is wrapped in a `create_effect()`. When an effect runs, it sets a **thread-local active context** ([`reactivity/effect.cpp`](https://github.com/Levizir/morph/blob/main/runtime/cpp/reactivity/effect.cpp)); any signal it reads while that context is set records the effect as a subscriber. That's the "automatic" part — no manual subscribe/unsubscribe calls in your code.

### 3. Dirty flags — the tree is marked, not rebuilt

When an effect writes a node, it marks it dirty — `StyleDirty`, `LayoutDirty`, `PaintDirty`, `SubtreeDirty` ([`core/node.h`](https://github.com/Levizir/morph/blob/main/runtime/cpp/core/node.h)). The next frame only lays out and repaints what's flagged ([`core/node/node.cpp`](https://github.com/Levizir/morph/blob/main/runtime/cpp/core/node/node.cpp)); clean subtrees are skipped and counted in the frame stats.

### 4. Damage — only changed pixels reach the screen

The renderer turns dirty nodes into work: flash replays the flattened frame; forge accumulates a damage set of only the changed rectangles and repaints those ([`renderers/forge/forge.cpp`](https://github.com/Levizir/morph/blob/main/runtime/cpp/renderers/forge/forge.cpp)).

So the pipeline is: **signal → effect → dirty flag → damage → present.** Every step is native, and every step is traceable with the frame's `DirtyStats` counters.

## Is this the best approach?

Depends what "best" means. For a native UI runtime, **fine-grained signals with a native tree** is on the short list of genuinely great options — it's the same family of ideas behind SolidJS and Preact Signals, which are considered the modern gold standard for frontend reactivity. Morph pairs that with node-level dirty flags and rectangle damage tracking, which is unusually precise for even a signal-based system.

But "best" also depends on the constraints, and those always come with trade-offs:

- **Signals are more machinery than a simple redraw.** If your app is a static UI that changes once a second, the subscription graph is overhead you barely use.
- **Node-level dirty flags are a middle choice.** Marking whole nodes is simpler than subscribing signals *per property*. It can repaint a node even when the changed property didn't touch it — a property-level system would be even finer, at the cost of more bookkeeping and more failure modes.
- **There's no scheduler or priority system.** React's concurrent mode can interrupt and reorder work. Desktop apps don't have the same 60fps deadline fights as loaded web pages, so Morph doesn't need it — but a scheduler *would* let it do more work in the gaps between frames.

So: the approach is excellent for what Morph is, with honest room to tune precision later.

## Could anything have been better?

Let's be honest about the alternatives that were considered and rejected:

| Alternative | Why it wasn't the choice |
|---|---|
| **Manual events / observers** | Works, but the wiring lives in your code: subscribe in one place, forget to unsubscribe in another, and the UI drifts from state. This is the bug reactor exists to delete. |
| **Virtual DOM diffing (React)** | A mirror tree + a full diff pass each update. It exists to protect a slow browser DOM from change — Morph has no DOM to protect ([Q: Virtual DOM?](q-virtual-dom.md)). It would add a cost, not remove one. |
| **Coarse re-render (whole app redraw)** | Trivial to build and impossible to miss a dependency — but it scales with app size instead of change size. Fine for tiny UIs, wrong for 5,000–20,000-node editors (the target [forge](../rendering/forge.md) is built for). |
| **Property-level signal subscriptions** | The most precise option — only the exact property updates. Better precision, but far more subscription edges to get wrong, and the win over node-level flags is small. Not worth the complexity today. |

The closest "could have been better" candidates are the precision tune-ups above, not a different architecture. The core choice — push-based signals into a native tree — is one the best reactive frameworks converged on independently.

## Why this choice fits Morph

Four reasons this is the right architecture *here*, specifically:

1. **It matches a compiler.** Morph knows state variables, JSX expressions, and the node tree at compile time. It can generate signals statically and wire effects directly — no runtime trampoline, no interpreter, no genericity tax.
2. **It survives hot reload.** Dev mode keeps the app running and reloads the logic module; the `SignalStore` keeps signal state across the reload by name ([`dev/signal_store.h`](https://github.com/Levizir/morph/blob/main/runtime/cpp/dev/signal_store.h)). A scheduler or a virtual DOM would have made live-reload much harder.
3. **It keeps binaries small.** Signals + dirty flags are a few hundred lines of native code. A reconciler plus an entire JS engine is not in the same galaxy as Morph's footprint goal.
4. **It's debuggable.** Every frame records layout/paint/skip/damage counts in `DirtyStats` ([`core/node.h`](https://github.com/Levizir/morph/blob/main/runtime/cpp/core/node.h)). You can see reactivity working — which nodes were skipped, which pixels were damaged — instead of trusting a black-box diff.

The short version: reactivity in Morph is **signals → effects → dirty flags → damage**, chosen because a compiler-plus-native-runtime can own its tree, keep state across hot reloads, and tell you exactly what changed — and because the models that would have been "better" solve problems Morph doesn't have.

See also [Q: Virtual DOM?](q-virtual-dom.md), [Q: Is it React?](q-react.md), [How It Works](../concepts/how-it-works.md), and [Rendering](../rendering/index.md).