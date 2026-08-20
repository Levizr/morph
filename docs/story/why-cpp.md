# Why C++ First (Instead of Rust)

**Part of:** [The Story of Morph](index.md)

The runtime — layout, rendering, events, the window loop — is C++. People often ask why not Rust, so here's the honest answer: it came down to four things.

## 1. JS already looks like C++

Morph's whole bet is "compile TypeScript/JavaScript down to native code." That translation is only natural if the target language *feels* like the source.

JavaScript and C++ share the same shape:

```js
// JavaScript
function add(a, b) { return a + b; }
const total = items.reduce((sum, x) => sum + x, 0);
```

```cpp
// C++
auto add = [](int a, int b) { return a + b; };
int total = std::accumulate(items.begin(), items.end(), 0);
```

Same braces, same operators, same flow control, same mental model. The `TSToCppTranslator` maps one syntax onto the other with almost no impedance mismatch — every language feature compiles to something C++ expresses directly. Rust is a different mindset entirely: ownership, borrowing, lifetimes, traits. Those are *better* in many ways, but they're not a translation target for JS — they're a rewriting target. C++ is what the compiler could generate faithfully without fighting the language.

## 2. Binary size — the 1 MB promise

Morph's main selling point is a **hello-world app under 1 MB**. That only works if the runtime stays lean.

Rust's standard library and runtime machinery add real overhead to the final binary. It's not a flaw — it's the cost of everything Rust gives you — but it pushes binaries up. I don't just believe this; I measured it (see point 4).

C++ compiles to the smallest native output with the tools I use, and that size is a core part of what Morph is. Choosing Rust for the runtime would mean negotiating against the very promise that makes Morph interesting.

## 3. I know C++ better — and I like it

Honest point: I'm more familiar with C++ than Rust, and I genuinely enjoy writing it. That's not "I can't write Rust" — it's "I've spent years in C++ and I'm faster and more precise in it." For a solo project where *I* am the entire engineering team, the language I think in is a real advantage. Fewer mistakes, faster iterations, less fighting the compiler.

C++23 makes it pleasant too — smart pointers, ranges, the modern standard library. This isn't 90s C++; it's the modern language with memory safety where it matters and zero overhead where it counts.

## 4. I've shipped Rust — and saw the size with my own eyes

A few months before Morph's runtime, I built a project in Rust called **SonicPro** — it streams your PC's sound over your local network so you can play PC audio anywhere with the lowest latency possible. (My PC doesn't have a speaker — I *needed* this.)

The final release binary was **7 MB**. For a tool that captures and streams audio. 7 MB of binary for a sound pipe.

That number stuck with me. A lean utility ballooning to 7 MB felt wrong — and it's exactly the kind of thing Morph is supposed to *not* be. If a streaming helper can't stay small in Rust, a whole UI framework in Rust was going to blow way past the 1 MB promise.

**Fun fact: I still use SonicPro every single day.** It's how this PC plays sound — the audio goes over the network to whatever device I'm actually near. It works, it's fast, it's just… 7 MB of binary for what should be a 1 MB pipe. Every time I use it, I remember why Morph's runtime is C++.

So: C++ first. Rust stays on the roadmap as a second runtime target for components that want it — see [Rust Support](../future/rust.md) — but the core runtime earns its keep in C++.

## What this doesn't change

- **Performance with control** — C++ gives the performance and direct control a custom renderer needs. Nothing between the code and the OS.
- **Zero runtime dependencies** — no browser, no Node, no VM. Just a binary linked against GLFW and OpenGL.
- **Honest work** — what you write is what gets compiled. When something is slow, you can see exactly where.

## The future

Rust remains planned as a second runtime target (`--lang rust`) with cross-language interop between C++ and Rust components — [Rust Support](../future/rust.md) explains how that works without changing how you write Morph.