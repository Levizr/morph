# Does Morph Use a Virtual DOM like React?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

**No.** Morph has no virtual DOM — and it doesn't need one. This page covers how React's model works, what Morph does instead, and why the virtual DOM wasn't the right tool here.

## What a virtual DOM is and why React has one

React's core trick is a JavaScript mirror of your UI. When your state changes, React rebuilds that JS mirror, **diffs** the old mirror against the new one, figures out the smallest set of DOM operations that get from one to the other, and applies just those operations to the real DOM.

That mirror exists for a reason. The browser DOM was never designed for fast, high-frequency updates — reading and writing it is expensive. So React keeps a cheap, in-JS copy of it, computes changes against that copy, and touches the real DOM as little as possible. The virtual DOM is a **layer that exists to protect the real DOM from change**.

Morph was never in that situation.

## What Morph does instead

Morph is a **compiler + native runtime**, not a JS library running on a DOM. Your `.mx` source compiles into a real, native node tree that the runtime owns directly. That tree has no browser to protect — it *is* the actual UI resource, held in native memory and drawn with OpenGL.

In Morph, a state change flows through three native mechanisms instead of a diff pass:

### 1. Signals

`morphState` values are signals ([`reactivity/signal.h`](../../runtime/cpp/reactivity/signal.h)). Each signal keeps a list of subscriber effects, and `set()` notifies them directly. `notify_all()` marks each subscribed effect as pending and appends it to a pending queue ([`reactivity/effect.cpp`](../../runtime/cpp/reactivity/effect.cpp)):

```cpp
void set(T v) {
    std::lock_guard<std::mutex> lock(m_mutex);
    value_ = v;
    notify_all();
}
```

Notice what's *not* here: no tree rebuild, no diff. Only the effects that actually subscribed to that value are queued — and subscription is automatic while an effect runs.

### 2. Dirty flags

Each effect that writes the UI marks nodes instead of rebuilding them. Every node carries a set of flags ([`core/node.h`](../../runtime/cpp/core/node.h)):

```cpp
enum DirtyFlag : uint8_t {
    Clean        = 0,
    StyleDirty   = 1 << 0,
    LayoutDirty  = 1 << 1,
    PaintDirty   = 1 << 2,
    ScrollDirty  = 1 << 3,
    SubtreeDirty = 1 << 4,
};
```

A style change marks `PaintDirty`; a geometry change marks `LayoutDirty`; any change propagates `SubtreeDirty` up to ancestors. Then the frame only redoes what's flagged:

- **Layout** — `layoutIfNeeded()` skips clean subtrees entirely ([`core/node/node.cpp`](../../runtime/cpp/core/node/node.cpp)) and counts the skips, so you can see how much work was avoided. Dirty nodes relayout; the rest don't move.
- **Paint** — only paint-dirty nodes are re-recorded into the flattened draw list.

### 3. Damage tracking (the render side)

Downstream of dirty flags, the renderers decide what reaches the screen:

- **flash** — the simple, default renderer: full clear, replay the flattened `RenderFrame`, present. Pixel-correct by construction ([rendering/flash](../rendering/flash.md)).
- **forge** — a retained compositor that keeps the window's pixels on the GPU and builds a **damage set** of the rectangles that actually changed. It scissor-clears and re-rasterizes only nodes touching the damage, then blits the surface ([`renderers/forge/forge.cpp`](../../runtime/cpp/renderers/forge/forge.cpp)). The same dirty-flag system drives it — `PaintDirty` nodes become damage rects each frame.

Nothing in that chain compares one tree to another, because the tree is never duplicated in the first place. The node you mutate *is* the node that lays out and draws.

## Why not a virtual DOM

The virtual DOM solves a problem Morph doesn't have, and it would cost Morph things it can't afford:

- **There's nothing to protect.** A virtual DOM buys you cheap updates against an expensive, hostile DOM. Morph's tree is native memory — mutating it directly is already cheap. Adding a mirror would be *adding* the expense React built its layer to *remove*.
- **No double bookkeeping.** A virtual DOM means two representations of the UI that must stay in sync, and yet another pass (the diff) to compute the gap between them. Signals don't need the gap — they know exactly which node changed and why.
- **No diff cost.** On big UIs the diff itself is the hidden tax: build a full new tree, walk it against the old one, reconcile, then let the old one be garbage-collected. Morph's cost scales with the nodes actually affected, not the size of the tree.
- **Native scheduling, not fibers.** Fibers exist so React can reprioritize work inside a JS event loop. A native runtime schedules its own frames — the compositor owns the GL context on its own thread, and the main thread swaps a lock-free `RenderFrame`. There's no JS scheduler to thread through.
- **A compiler can be smarter than a reconciler.** Because Morph knows the tree at compile time, it can prune whole features away. A virtual DOM is a runtime generality — it must handle any UI any code could build. Morph's dirty-flag and damage systems are compiled for the actual app.

## Where the two models agree

Morph *is* React-inspired at the API level — JSX, components, `morphState`, `morphEffect`, declarative UI. The familiar bits are deliberate. It's only the reconciliation machinery underneath that's absent, and deliberately so: that machinery exists for the browser, and Morph doesn't run in one.

See also [Q: Is it React?](q-react.md), [How It Works](../concepts/how-it-works.md), and [Rendering](../rendering/index.md).