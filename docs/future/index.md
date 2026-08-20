# Future Plans

Everything on Morph's roadmap, documented in detail. Each feature page covers what it is, why it matters, how it will work, and the current state of any scaffolding already in the codebase.

> **Note:** These are future plans, not commitments. The syntax and APIs shown here are proposals — they can be completely different when actually implemented.

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| `<morph-viewport>` — embedded OpenGL canvas | [Viewport](viewport.md) | High | — |
| Imperative `Window` / `App` API | [Window API](window-api.md) | High | — |
| File-based windows & pages (`route.mx` convention) | [File Routing](file-routing.md) | High | Window API |
| `Menu` / `Tray` / `Dialog` / `Notification` modules | [Native Modules](native-modules.md) | Medium | Window API |
| Multi-window navigation (`useWindow`) | [Multi-Window](multi-window.md) | High | File Routing |
| Forge tile pool, retained layers, scroll-shift | [Forge Renderer](forge-renderer.md) | Medium | Forge (beta) |
| Full CSS cascade | [CSS Cascade](css-cascade.md) | High | — |
| Broader TS→C++ translator coverage | [JS Coverage](js-coverage.md) | High | — |
| Text input (caret, focus, selection) | [Text Input](text-input.md) | High | — |
| `box-shadow`, `outline`, margin collapse | [More CSS](more-css.md) | Low | CSS Cascade |
| Package JS→C++ build bridge | [Packages](packages.md) | Medium | — |
| Windows / macOS support | [Platforms](platform.md) | Medium | — |
| OS accessibility reader (screen readers, focus, keyboard nav) | [Accessibility](accessibility.md) | Medium | Platforms |
| Code signing, notarization, secure updates & store packaging | [Security & Commercial Release](security.md) | Medium | Platforms |
| VSCode extension, `morph-icons`, `morph-animate` | [Tooling](tooling.md) | Low | — |
| Hidden classes, compositor-safe properties | [Performance](performance.md) | Low | — |
| Full Rust runtime (`--lang rust`, cross-language interop) | [Rust Support](rust.md) | High | — |
| Vulkan / Metal / DirectX backends (pluggable graphics) | [Graphics APIs](graphics-api.md) | High | — |
| Rust compiler (SWC/Oxc) + native CLI, Python removed | [Rust Compiler](compiler.md) | High | Rust runtime (optional) |

**Status meanings:** `production` — shipped and stable · `beta` — shipped, known bugs · `development` — under active construction · `future` — planned, not built yet.

## How to influence the roadmap

Open an issue or PR — see [Contributing](../../CONTRIBUTING.md). The most impactful areas right now are the **CSS cascade**, **TS→C++ translator coverage**, and the **Forge tile pool**.

**Have an idea or a feature you need?** See [Suggestions](suggestions.md) — or email us directly at [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com).