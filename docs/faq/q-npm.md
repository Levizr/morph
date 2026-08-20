# Can I Use npm Packages in Morph?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

Not yet — and honestly, this is one of the biggest open questions.

Morph has its own import model and no Node runtime. Most npm packages won't work as-is, because:

- **They expect a browser or Node environment** — `document`, `window`, `process`, Node's module system
- **They're often shipped as JS** — Morph compiles *your* JS to C++; it doesn't interpret arbitrary third-party JS at runtime
- **The package manager story is young** — there's no `npm install` pipeline wired into `.mx` yet

What works today:

- **Your own code** — imports, state, effects, all compiled natively
- **Native interop** — call C++ functions you write from component logic ([Native C++ Interop](../guides/native-cpp.md))
- **Built-in modules** — the `morph` module: `CSS`, `morphState`, `morphEffect`, etc.

The plan is a **build bridge**: [Package Build Bridge](../future/packages.md) — a system that resolves packages at build time and compiles what's compatible, with a curated set of JS libraries that make sense in a compiled native world. It won't be "everything on npm"; it'll be "the useful subset, done properly."

For now, if a library is a thin utility (math, date formatting, string helpers), the answer is usually "write the 20 lines yourself" — it's a nice win to have it native.

## The plan: a Morph registry

Instead of relying on npm, we're thinking of **our own registry** — a place to find UI-related stuff that makes sense in Morph's world:

- Components, themes, and UI building blocks (`.mx` packages)
- **Even native C++ modules** — real native functionality that installs and links straight into your app
- Installed directly through the Morph CLI:

```bash
morph install morphui-table     # UI components
morph install native-audio      # native C++ modules
```

One command, resolved at build time, compiled into your binary — no runtime to load, no Node involved.

## The security problem (honest, and we're asking for ideas)

There's a serious problem we're thinking about, and I want to be straight about it: **third-party native code is not automatically secure.**

A JavaScript library runs inside a sandbox — worst case, it does what a script can do. A **native C++ library runs with your app's full privileges**. Someone could publish a library that looks useful and quietly include malware you'd never know about. Your app would ship it, and it could do anything your app can do — read files, call the network, everything.

So a registry of native modules needs real answers, not vibes. Things we're considering:

- **Code signing** — every package must be signed by its author, verified before install
- **Verified builds** — packages built on our infrastructure from public source, so the binary provably matches the published source
- **Permission manifests** — a package declares what it needs (network, specific file paths) and the app can audit or deny it
- **Trusted authors / review** — a curated tier of community-reviewed packages

**If you have ideas for solving this, we genuinely want to hear them** — [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com). It's a hard problem, and good answers will make Morph's ecosystem safe in a way npm never was.

## And maybe npm itself, someday

And there's another path: if full Node.js support ever lands in Morph, you'd be able to **use npm packages via npm** — the whole ecosystem, the normal way. That's a much bigger project (a Node-compatible runtime is a big deal), but it's on the table. The [Package Build Bridge](../future/packages.md) page tracks how packages get resolved and compiled either way.