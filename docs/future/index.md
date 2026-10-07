# Future Plans

Everything on Morph's roadmap, documented in detail. Each feature page covers what it is, why it matters, how it will work, and the current state of any scaffolding already in the codebase.

> **Note:** These are future plans, not commitments. The syntax and APIs shown here are proposals — they can be completely different when actually implemented.

Plans are grouped by area. Each group is a folder under `docs/future/` and a section in the docs sidebar, so a page stays put when its neighbours move.

## CSS

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| Full CSS cascade | [CSS Cascade](css/cascade.md) | High | — |
| `box-shadow`, `outline`, margin collapse | [More CSS](css/more-properties.md) | Low | CSS Cascade |
| Magical CSS — trick-free effects & Morph-only ideas (possibility showcase, basics first) | [Magical CSS](css/magical.md) | Low | More CSS |
| **Dynamic Styles & Classes** (state-driven keyword styles, dynamic className) | [Dynamic Styles](css/dynamic-styles.md) | Medium | Kill Strings |
| Transitions, springs & shared-element animation | [Animations](css/animations.md) | Medium | Forge Renderer |
| Animation engine fixes — timing functions, property support cost, Chrome/Qt comparison | [Animation Engine](css/fixing-animation.md) | Medium | Animations |

## Windows

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| `<morph-viewport>` — embedded OpenGL canvas | [Viewport](windows/viewport.md) | High | — |
| Imperative `Window` / `App` API | [Window API](windows/api.md) | High | — |
| File-based windows & pages (`route.mx` convention) | [File Routing](windows/routing.md) | High | Window API |
| Route mounts & per-instance state | [Route Mounts](windows/route-mounts.md) | Medium | File Routing |
| Multi-window navigation (`useWindow`) | [Multi-Window](windows/multi-window.md) | High | File Routing |
| Desktop integration (hotkeys, deep links, single-instance, drag-drop, theme, badge) | [Desktop Integration](windows/desktop-integration.md) | High | Window API |
| Cross-window event listeners (`win.on` / `win.off`) | [Window Events](windows/events.md) | Medium | Window API |

## JavaScript

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| Broader TS→C++ translator coverage | [JS Coverage](javascript/coverage.md) | High | — |
| Full Node.js support (`morph/*` imports with `node:*` aliases, npm packages, servers as native binaries) | [Node.js Support](javascript/nodejs.md) | Medium | JS Coverage |
| **State, Events & Native C++ Interop** | [State/Events/Native](javascript/native-interop.md) | High | Rust Compiler |
| **Universal Module Bindings** (import/export/native calls for functions, vars, classes) | [Module Bindings](javascript/module-bindings.md) | High | State/Events/Native |
| Full Rust runtime (`--lang rust`, cross-language interop) | [Rust Support](javascript/rust.md) | High | — |

## Rendering

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| Forge tile pool, retained layers, scroll-shift | [Forge Renderer](rendering/forge-tile-pool.md) | Medium | Shipped (forge opt-in) |
| Vulkan / Metal / DirectX backends (pluggable graphics) | [Graphics APIs](rendering/graphics-apis.md) | High | — |
| Hidden classes, compositor-safe properties | [Performance](rendering/performance.md) | Low | — |
| Virtualized lists (100k rows) | [Virtualized Lists](rendering/virtualized-lists.md) | Medium | Forge Renderer |

## Elements

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| Text input (caret, focus, selection) | [Text Input](elements/text-input.md) | High | — |
| SVG element & `morph-icons` system | [Icons & SVG](elements/icons-svg.md) | Medium | Packages |
| SVG engine — own renderer, Chrome-look parity, pay-for-what-you-use math | [SVG Engine](elements/svg.md) | Medium | Icons & SVG |

## Platform

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| `Menu` / `Tray` / `Dialog` / `Notification` modules | [Native Modules](platform/native-modules.md) | Medium | Window API |
| OS accessibility reader (screen readers, focus, keyboard nav) | [Accessibility](platform/accessibility.md) | Medium | Platforms |
| WebView — embed HTML via the OS webview (Tauri-style hybrid apps) | [WebView](platform/webview.md) | Medium | — |
| Code signing, notarization, secure updates & store packaging | [Security & Commercial Release](platform/security.md) | Medium | Platforms |
| Android & iOS from the same `.mx` codebase | [Mobile](platform/mobile.md) | Medium | Platforms |

## Tooling

| Feature | Page | Priority | Depends on |
|---|---|---|---|
| VSCode extension, `morph-icons`, `morph-animate` | [Tooling](tooling/vscode.md) | Low | — |
| **Package ecosystem** (one dependency: components, native C++, compiler plugins, registry) | [Package Ecosystem](tooling/package-ecosystem.md) | High | Packages |
| Package JS→C++ build bridge | [Packages](tooling/packages.md) | Medium | Package Ecosystem |
| Morph lean binaries — 150KB budget | [Lean Binaries](tooling/lean-binaries.md) | Medium | — |
| Time-travel debugger (multi-window record/replay) | [Time-Travel](tooling/time-travel.md) | Medium | State/Events/Native |
| `morph test` headless E2E runner | [Test Runner](tooling/test-runner.md) | Medium | Time-Travel |
| **Have an idea?** | [Suggestions](tooling/suggestions.md) | — | — |

## Shipped

Landed already — kept for the design record.

| Feature | Page | State | Depends on |
|---|---|---|---|
| Window ownership (`parent` / `modal` / `role`) | [Window Ownership](shipped/window-ownership.md) | ✅ Shipped | Window API |
| Windows / macOS support | [Platforms](shipped/platform.md) | ✅ Shipped | — |
| Rust compiler (SWC/Oxc) + native CLI, Python removed | [Rust Compiler](shipped/compiler.md) | ✅ Shipped (Sept 2026) | — |
| **Kill All Runtime Strings** (CSS/node/value enums, hash lookups) | [Kill Strings](shipped/kill-runtime-strings.md) | ✅ Complete (Sept 2026) | — |

**Status meanings:** `production` — shipped and stable · `beta` — shipped, known bugs · `development` — under active construction · `future` — planned, not built yet.

## How to influence the roadmap

Open an issue or PR — see [Contributing](../../CONTRIBUTING.md). The most impactful areas right now are the **CSS cascade**, **TS→C++ translator coverage**, and the **Forge tile pool**.

**Have an idea or a feature you need?** See [Suggestions](tooling/suggestions.md) — or email us directly at [suggestions.morph@levizr.com](mailto:suggestions.morph@levizr.com).