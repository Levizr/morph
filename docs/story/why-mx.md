# Why .mx Instead of .tsx / .jsx / .js / .ts

**Part of:** [The Story of Morph](index.md)

A question people ask early: why invent a new file extension? Why not just use `.tsx` or `.js`? Honest answers below — including the reason I'm not even sure about.

## The primary reason: confusion

JSX and TSX come with expectations. When someone opens a `.tsx` file, they expect **React** — `useEffect`, `useState`, the React ecosystem, the whole mental model. Morph is React-*inspired*, but its primitives are Morph's: `morphState`, `morphEffect`, `morph-open`, `morph-window`. They're similar — but not identical.

The failure mode is real:

```tsx
// A user writes this in a .tsx file, expecting React:
useEffect(() => { ... })   // ❌ doesn't exist in Morph

// What they needed was:
morphEffect(() => { ... })  // ✅ this is Morph's effect
```

A wrong-but-close API call is the worst kind of error — the code *looks* right, so the confusion is "why isn't this working?" instead of "what API do I use?"

The `.mx` extension is a **signal**: *this is Morph, not React. Different framework, different APIs, check the docs.* It sets expectations before you write a single line.

And honestly? I know that's not a great reason. I'm not fully sure why I picked `.mx` — it just felt right, and it turned out to have a useful side effect. I'll take it.

## Beyond JSX

JSX describes components — but Morph has things JSX doesn't. Declarative windows (`<morph-window>`), embedded native canvases (`<morph-viewport>`), navigation attributes (`morph-open`, `morph-navigate`), file-based routing, CSS loaded from `.mx` files. A `.mx` file is Morph's own dialect — it can grow its own syntax and elements without colliding with what everyone expects a JSX file to be.

## You're not stuck with .mx

`.mx` is the default and the recommendation — but it's not a cage. The framework also supports **`.ts` and `.tsx`**. If you'd rather write plain TypeScript, or keep your existing TS/TSX files, Morph handles them. `.mx` is where the full experience lives; the others are first-class citizens, not an afterthought.

## Why TypeScript and not JavaScript

This one I'm very sure about.

**JavaScript's dynamism is hard to track.** The compiler is Python — it reads your source and turns it into C++. If the source is dynamically typed JS, the compiler has to *infer* what a variable can be, chase types through branches and callbacks, and guess at runtime shapes. Generating C++ from that is genuinely hard — the type of `x` might change mid-function, and C++ needs a concrete type at every step.

**TypeScript removes the guessing.** Types are written down, in the file, where the compiler can read them directly. The translation from TS to C++ is mechanical and predictable: `let count: number = 0` is a number, everywhere, always.

That predictability is why Morph can promise "your logic runs as native code" — the compiler always knows what it's generating.

## Native C++ types when you want them

Because it's TypeScript compiled to C++, you can reach for **native C++ datatypes** whenever you need real performance — right in your `.mx` file, anywhere you want:

```ts
let a: int = 100;          // native C++ int — zero overhead
let price: double = 99.5;  // native C++ double
let name: string = "morph"; // native std::string
```

For the JS-compatible types, Morph ships its own runtime types — `JsNumber`, `JsString`, `JsArray`, `JsObject` — so normal JS-style code behaves like JavaScript at runtime (truthiness, coercion, property access). But the moment you annotate with a native type, you get the native thing: direct, unboxed, no overhead. Best of both worlds — JS ergonomics when you want them, C++ performance when you need it.