# What Happens to My JavaScript at Runtime?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

It becomes C++ — then machine code.

At build time, the `TSToCppTranslator` converts your component logic into C++ methods. Those methods are compiled with `g++` into the app's logic module and linked into the binary. At runtime:

- **No interpreter** — nothing reads your JS as JS
- **No JIT** — there's no VM warming up at launch
- **No VM** — no V8, no Node, no JavaScript engine of any kind

```tsx
// what you write:
const doubled = items.map(x => x * 2)

// becomes, effectively:
for (auto& x : items) { doubled.push_back(x * 2); }
```

The practical result: your component logic runs at the speed of compiled C++. Property access, loops, arithmetic, function calls — all direct machine instructions, not interpreted dispatch. That's the performance story behind "your logic is native code."

This only works because the compiler covers the TypeScript/JavaScript you actually write. That's why [TS→C++ Translator Coverage](../future/js-coverage.md) is a priority — every language feature the translator covers is one more thing you can use and still get native speed.