# Future Plans

The roadmap for Morph. Items are ordered roughly by how they build on each other — earlier items unblock later ones.

## v0.1.0 (Next Up)

- [ ] **Full CSS cascade** — merge all matched rules by specificity and origin into a computed style (selector engine exists; see [Under Construction](under-construction.md))
- [ ] **Broader TS→C++ translator surface** — more built-ins and array/object methods so real-world JS logic compiles without workarounds
- [ ] **`position: relative` / `fixed` / `sticky`** — offset and viewport-relative positioning, then sticky (parse + runtime fields already exist)
- [ ] **Forge tile pool** — content-keyed tile caching with an LRU budget and scroll-shift remap; the goal is to make `forge` a production-ready alternative to `flash`
- [ ] **Margin collapse** — collapsing vertical margins between siblings per CSS 2.1
- [ ] **Multi-window navigation** — wire up `WindowManager::open()` / `navigate()` so apps can show, close, and navigate between windows at runtime

## Imperative API Layer

Today windows are declared declaratively (`windowConfig` export or `<morph-window>`). The future adds an imperative core API on top of it:

```ts
const win = new Window({ title: "Settings", width: 400, height: 300 })
win.show()
win.on('close', () => { ... })
```

- [ ] **`Window` class** — instantiable, config-object constructor, owns one GL context + layout tree
- [ ] **`App` singleton** — lifecycle: `App.quit()`, `App.on('ready' | 'before-quit', ...)`
- [ ] **File-based window/page convention** — `windows/login.tsx`, `windows/settings.tsx` compiled into `Window` instances with a generated manifest (Next.js-style sugar **on top of** the imperative API, not a replacement)
- [ ] **`Menu` / `Tray` / `Dialog` / `Notification` modules** — same pattern: config-object constructors, hand-written `.d.ts`, work both imperatively and via file conventions

## Platform & Tooling

- [ ] **Windows support** — MSVC / MinGW build, bundled GLFW/FreeType/HarfBuzz (dev runtime and build pipeline)
- [ ] **macOS support** — clang++ toolchain (system requirements already list it)
- [ ] **VSCode extension** — syntax highlighting and IntelliSense for `.mx` files
- [ ] **`morph-icons`** — first-party icon package
- [ ] **`morph-animate`** — animation library built on top of the CSS animation engine

## Components & Rendering

- [ ] **`<morph-viewport>`** — embedded OpenGL canvas element for custom native rendering inside a window (runtime scaffold exists in `morph/runtime/viewport/`, marked planned)
- [ ] **Forge scroll-shift remap** — reusing cached tiles when content scrolls instead of re-rasterizing (Phases 4/6 of `help/renderer-flash-forge.md`)

## Performance

- [ ] **Hybrid object shapes (hidden classes)** — replace pure `std::map` object storage with a shape descriptor + indexed slot fast path so `app.config.theme.darkMode` chains stop doing repeated map lookups and `shared_ptr` refcount bumps. Deferred until profiling shows it matters (object counts > 10k, > 100k property accesses/frame) — the current design is correct and sufficient until then.

---

The best way to influence the roadmap is to open an issue or PR — see [Contributing](../../CONTRIBUTING.md).