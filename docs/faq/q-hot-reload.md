# How Does Hot Reload Work with a Compiler?

**Part of:** [Questions & Answers](index.md) · [The Story of Morph](../story/index.md)

Good question — you'd think "compiled" means "restart to see changes." It doesn't.

Dev mode works like this:

1. **The app stays running** — the window and the layout tree live in a dev runtime binary (`morph_devrt`)
2. **Your component logic lives in a shared library** — `logic.so` — compiled separately from the runtime
3. **You save** — Morph detects the change, recompiles only the logic into a fresh `logic.so`
4. **The runtime swaps it** — `dlopen` loads the new library and the live-reload machinery swaps the changed nodes in the tree, **without restarting the window**

So the compiled core (the renderer, the window, the layout engine) never restarts — only your logic module is hot-swapped. It's the same technique dynamic plugin systems use, applied to your own code.

CSS and JSX structure changes re-run the front of the pipeline (parse → IR → node tree) and re-mount the affected part of the tree. Your app state persists across the swap, and the loop is near-instant for small changes.

The [Dev Mode](../concepts/dev-mode.md) page has the full pipeline diagram. And when the compiler moves to Rust ([Rust Compiler & Native CLI](../future/compiler.md)), this loop gets dramatically faster — compile time is the only slow part of the cycle.