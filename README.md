<div align="center">

<br/>

```
███╗   ███╗ ██████╗ ██████╗ ██████╗ ██╗  ██╗
████╗ █████║██╔═══██╗██╔══██╗██╔══██╗██║  ██║
██╔████╔██║██║   ██║██████╔╝██████╔╝███████║
██║╚██╔╝██║██║   ██║██╔══██╗██╔═══╝ ██╔══██║
██║ ╚═╝ ██║╚██████╔╝██║  ██║██║     ██║  ██║
╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝
```

**Build native OpenGL Applications with HTML, CSS, and JavaScript.**

No browser. No Electron. No WebView. Just a lightweight native binary.

<br/>

[![License](https://img.shields.io/badge/license-Apache-7c6af5?style=flat-square?logo=apache)](LICENSE)
[![Python](https://img.shields.io/badge/python-3.10+-1dc98a?style=flat-square&logo=python&logoColor=white)](https://python.org)
[![C++](https://img.shields.io/badge/C++-23-4da6ff?style=flat-square&logo=cplusplus&logoColor=white)](https://isocpp.org)
[![OpenGL](https://img.shields.io/badge/OpenGL-3.3-f06449?style=flat-square)](https://opengl.org)
[![Version](https://img.shields.io/badge/version-0.0.6-7c6af5?style=flat-square)]()

<br/>

</div>

---

## What is Morph?

Morph is a UI framework that compiles `.mx` files (JSX-like syntax with TypeScript/JavaScript and CSS) directly into native OpenGL binaries. You write familiar web syntax — Morph produces a lean, native binary with zero browser overhead.

```tsx
// src/App.mx
import { CSS, morphState } from 'morph'

CSS.load("./style.css")

export const windowConfig = { title: "My App", width: 800, height: 600 }

export default function App() {
  const [count, setCount] = morphState(0)
  return (
    <body>
      <div className="app">
        <h1 style="color: #e0e0e0;">Hello from Morph</h1>
        <button className="btn" onClick={() => setCount(count + 1)}>
          Clicked {count} times
        </button>
      </div>
    </body>
  )
}
```

```bash
morph dev      # live window, hot reload via Unix socket + logic.so
morph build    # optimized native binary
```

---

## Why Morph?

| | Electron | Qt | Morph |
|---|---|---|---|
| Write UI in | HTML/CSS/JS | C++ / QML | TS/JSX/CSS |
| Runtime | Chromium (~150MB) | Qt libs | **Zero** |
| Binary size | ~80MB+ | ~20MB+ | **<1MB** |
| Native OpenGL access | ✗ | ✓ | ✓ |
| Hot reload | ✓ | ✗ | ✓ |
| Custom C++ nodes | ✗ | ✓ | ✓ |

---

## Quick Start

**1. Install**
```bash
pip install levizr-morph
morph doctor          # verify system dependencies
```

**2. Create a project**
```bash
morph init my-app
cd my-app
```

**3. Start dev mode**
```bash
morph dev
```

A native window opens. Edit `src/App.mx` — the window updates instantly without restarting.

**4. Ship**
```bash
morph run         # builds and runs the production binary
# or with --static to link GLFW/FreeType/HarfBuzz into a single self-contained file
morph run --static
```

---

## How It Works

Morph is a **compiler**, not an interpreter. Your source files never ship — only the compiled binary does.

```
src/App.mx ──► MorphParser ──► JSXWalker ──► IRBuilder ──► LayoutEngine ──► IRSerializer
                                                                              │
                                              ┌───────────────────────────────┴──────────────┐
                                              ▼                                               ▼
                                     [Dev: IPC Socket]                              [Build: C++ Codegen]
                                     morph_devrt binary                     node_emitter → g++ → binary
                                     + logic.so (dlopen)                    TS→C++ (logic) → g++ → logic
```

**Python** handles the entire toolchain — `.mx` parsing via tree-sitter, IR building, layout math, Jinja2-based C++ code generation, and **TypeScript→C++ translation** for your JS logic. **C++** handles the runtime — OpenGL rendering, window management, events, reactivity, and coroutines. The final binary contains zero Python and zero Node.

In **dev mode**, the pipeline produces an IR dict sent over a Unix socket to a pre-compiled renderer (`morph_devrt`) on every file save, while your component logic is compiled to a `logic.so` shared library that is `dlopen`ed at runtime. The window never closes — only the node tree swaps and the logic `.so` is re-wired in place (effects keep their subscriptions across reloads). In **build mode**, the same IR dict drives Jinja2 C++ code generation, producing a standalone binary via g++.

Since v0.0.6, rendering runs on a dedicated **compositor thread**: the main thread handles events, style, layout and paint, then flattens a lock-free `RenderFrame` snapshot that the compositor thread interpolates and draws at vsync. Two renderers ship — **flash** (lightweight full-clear) and **forge** (retained surface + damage tracking); production picks one at compile time, dev can toggle live.

---

## Current State (v0.0.6 — Early Development)

### ✅ Working

| Component | Status |
|---|---|
| **`.mx` file parsing** — tree-sitter-based JSX, imports, props | Complete |
| **CSS parsing** — local files, remote URLs, MD5-cached | Complete |
| **Tailwind CSS** — 500 common utility classes + arbitrary values | Complete |
| **IRBuilder** — walked AST → IR with inline CSS, Tailwind, color/unit conversion | Complete |
| **CLI** — `init` (interactive wizard), `dev`, `build` (`--static`), `run`, `pkg`, `doctor` (auto-install), `cache`, `translate` | Complete |
| **Config** — `morph.config.json` load/save | Complete |
| **IR data models** — `IRNode`, `IRWindow`, `IRPage`, `IRViewport`, `IRStyle`, `IREvent` | Complete |
| **IR serializer/deserializer** — JSON-safe dict for dev socket | Complete |
| **Layout engine** — box model (margin, padding), vertical stacking, gap, flexbox | Complete |
| **Dev file watcher** — watchdog-based with debounce | Complete |
| **Unix socket IPC** — sends IR to dev runtime | Complete |
| **C++ node emitter** — IR → C++ instantiation code from Jinja2 templates | Complete |
| **Build compiler** — g++ invocation with conditional FreeType/GLFW/OpenGL flags | Complete |
| **Package registry client** — fetch, install, manifest parsing | Complete |
| **OpenGL 3.3 batch renderer** — instanced VAO/VBO/IBO, uniform color + rounded rect SDF | Complete |
| **FreeType text rendering** — per-size glyph atlas, batch text with kerning | Complete |
| **Rounded rectangles** — SDF-based border-radius in fragment shader | Complete |
| **Font weight support** — bold/normal font selection, `font-weight` CSS property | Complete |
| **Style inheritance** — `color`, `font-size`, `font-weight`, `text-align` cascade from parent | Complete |
| **Transparent backgrounds** — only render when `background-color` set | Complete |
| **Event system** — `onClick`, `MouseDown`, `MouseUp`, `MouseMove` events | Complete |
| **Scrollbar** — browser-like drag, wheel, track-click, nested containers | Complete |
| **Viewport culling** — skip off-screen children in draw + event dispatch | Complete |
| **Feature-based compilation** — `#define MORPH_FEATURE_*` guards, linker GC | Complete |
| **Flexbox** — `justify-content`, `align-items`, two-pass layout, content-based sizing | Complete |
| **`text-align`** — left, center, right with correct container-relative centering | Complete |
| **`max-width`** — layout constraint for responsive sizing | Complete |
| **Margin/padding side properties** — `margin-top`, `padding-left`, etc. | Complete |
| **Cursor support** — `cursor: pointer` (hand), `cursor: text` (I-beam) | Complete |
| **`morph_devrt` binary** — dev mode C++ renderer, Unix socket IPC, hot reload | Complete |
| **Border rendering** — `border-width`, `border-color`, `border-style` with SDF shader on all elements (div, button, img); border ring batch on top of everything | Complete |
| **Image rendering** — `stb_image`-backed: PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC; per-texture batching, border-radius clipping | Complete |
| **Box-sizing** — `content-box` and `border-box` CSS property | Complete |
| **Dev mode auto-build** — CMake integration, automatic binary rebuild on missing | Complete |
| **Dirty incremental rendering** — layout/paint dirty flag propagation via `markDirty()`, incremental `layoutIfNeeded()`, selective `recordPaintTree()`, cached `m_displayList` replay via `executeDisplayList()`; compile-time `MORPH_FEATURE_DIRTY_RENDERING` gate, auto-enabled for dynamic features (scroll, hover, events, cursor) | Complete |
| **Window config hot reload** — title update on save, node tree swap without restart | Complete |
| **DevTools panel** — F12 toggle, element inspect (F2/click), box-model overlay (margin/border/padding/content), element info panel | Complete |
| **Nested border-radius clipping** — stencil-based (GL_INCR) so child clips properly intersect ancestor masks | Complete |
| **Runtime `margin: auto`** — dynamic horizontal centering re-resolved on window resize | Complete |
| **CSS `:hover` pseudo-class** — class-based hover rules from CSS files; resolved at runtime with snap-style swap or smooth transition; supports color, background-color, margin, padding, border, border-radius, font-size, gap, justify-content, align-items, width, height | Complete |
| **CSS transition** — `transition-duration` / `transition-timing-function` properties and `transition` shorthand; per-element config; interpolation of all numeric/color properties with easing (linear, ease-in, ease-out, ease-in-out); string/display properties snap instantly | Complete |
| **Body default `padding: 8px`** — replaced UA default `margin: 8px` so body background fills edge-to-edge without white gaps; no backward-compat baggage (Morph is not a browser); trivially overridable via CSS | Complete |
| **Window clear color from body background** — `glClearColor` set to body's `background-color` each frame; fallback to white when body is transparent; eliminates color mismatch glitch during resize | Complete |
| **Dev source hash** — CMake rebuild triggered when shared runtime files (core/, render/, widgets/, style/) change | Complete |
| **Wayland/X11 fallback error** — clear message when GLFW window creation fails | Complete |
| **Flexbox `flex-wrap`** — multi-line flex layout with wrap, grow/shrink distribution, `justify-content: space-between/space-around` per line | Complete |
| **Flex shorthand** — `flex: 1` → `grow:1 shrink:1 basis:0%`, `flex: none` → `0 0 auto`, `flex: auto` → `1 1 auto` | Complete |
| **`display: inline` fix + whitespace preservation** — text nodes default to inline; whitespace-only JSX nodes collapse to single space for inter-element spacing; inline measure pass with line-breaking simulation; `\n`/`\t`/`\r` treated as zero-width | Complete |
| **Z-index + paint order** — CSS 2.1 Appendix E stacking: negative/block/inline/auto/positive participant layers with stable sort; `paintOrder()` used for both rendering and hit-testing | Complete |
| **Ancestor-hover rules and transitions** — `.parent:hover .child` style rules applied/transitioned based on ancestor hover state; stencil-buffer fix for border-radius; inherited color on button/text | Complete |
| **Compositor thread** — dedicated render thread owns the GL context; main thread does events/style/layout/paint and flattens into a lock-free `RenderFrame` snapshot; compositor interpolates animations at vsync and pushes completion events back over an SPSC queue | Complete |
| **Flash / Forge dual renderers** — `flash` (lightweight full-clear direct renderer) and `forge` (hybrid retained tile compositor with damage tracking); production resolves renderer at compile time (`constexpr`, dead code eliminated), dev runtime can hot-switch via DevTools toggle | Complete |
| **Forge damage tracking** — `DamageSet` accumulates dirty rects (box-geometry diff vs prev-frame map + pre-layout paint dirt); retained FBO surface with scissored clears, re-raster of only damaged nodes, `glBlitFramebuffer` present; idle frames skip raster entirely | Complete |
| **JS runtime types** — `JsValue` variant (undefined, null, boolean, number, string, array, object, function) with JS `typeof`/truthy/`==` semantics, implicit coercion to `std::string`, `std::formatter` support; `JsNumber` (int/big/double variants), `JsString` (upper/lower/trim/charAt/indexOf/substring/slice/replace/split), `JsArray` (push/pop/index), `JsObject` (map-backed, has/keys/index) | Complete |
| **TypeScript → C++ compiler** — `morph translate` and the build/dev pipeline translate `.ts`/`.mx` JS to C++ via tree-sitter (`TSAstBuilder` + `TSToCppTranslator`): variables, functions, arrow functions, classes/interfaces, if/while/for/do/switch/try/throw, template literals, ternary, sequence, spread, TS types (`int`, `double`, `string`, generics, unions), auto header include detection | Complete |
| **Reactivity system** — `Signal<T>` with thread-local effect tracking, `create_effect()`/`run_pending_effects()`, auto-subscription on signal get/set; `morphState()` → signal getter/setter pair, `morphEffect()` → effect with cleanup; `fmt_double()` renders `8` / `2.5` / `Error` (never `nan`/`inf`) | Complete |
| **Coroutine task scheduler** — `morph::Task` eager coroutines, `co_await next_frame` resumes next tick, `process_tasks()` driver, JS-compatible `setTimeout`/`setInterval`/`clearTimeout` timers | Complete |
| **Async `fetch()`** — `morph::net` namespace: `await fetch(url)` performs HTTP GET on a worker thread and resumes the coroutine; `Response` mirrors JS API (`status`, `headers`, `ok()`, `text()`); `morph::Result<T>` promise-like return type with exception rethrow | Complete |
| **Dev-mode `logic.so`** — JS logic compiled to a content-hash-addressed shared library loaded via `dlopen`; hot reload re-wires signals/effects in place (`morph_logic_rewire`) without re-running effects; registry + signal store keep node/state references across reloads | Complete |
| **DevTools Logs tab** — thread-safe ring buffer of info/ok/warn/error entries with timestamps; clear button | Complete |
| **Docked DevTools panel** — panel occupies right side of the window and app layout is constrained to remaining content (browser-style docking); drag-resizable, never covers app elements | Complete |
| **CSS animations** — `@keyframes` declarations, `animation-name`/`animation-duration`/`animation-timing-function`/`animation-delay`/`animation-iteration-count`/`animation-direction`/`animation-fill-mode` properties and `animation` shorthand; easing functions (linear, ease, ease-in, ease-out, ease-in-out); percentage-based keyframe stops; property interpolation (color, bg-color, margin, padding, border, border-radius, font-size, gap, width/height, opacity, transform); `MORPH_FEATURE_ANIMATION` compile-time gate | Complete |
| **CSS transforms** — `transform` property with `translate`, `rotate`, `scale`, `skew`, `matrix`, `matrix3d` functions; 4x4 matrix composition (`mat4.h`); per-element transform applied in vertex shader; `transform-origin` support; Tailwind `rotate-*`, `scale-*`, `translate-*` utilities; `MORPH_FEATURE_TRANSFORM` gate | Complete |
| **C++/JSX interop** — `import { fn } from './file.cpp'` in JSX; user C++ files `#included` into the generated translation unit; `_morph_state.h` generated exposing signals, setter wrappers, and JSX function declarations so C++ can update state and call JSX code; `native` config block (`include_dirs`, `library_dirs`, `libraries`, `cflags`, `ldflags`) forwarded to g++ | Complete |
| **Window constraints** — `min-width`, `max-width`, `min-height`, `max-height` in `windowConfig` and `<morph-window>` JSX; enforced via `glfwSetWindowSizeLimits` in C++ runtime; serialized/deserialized for dev hot reload | Complete |
| **Build config options** — Wayland backend toggle, FreeType static linking, UPX compression post-build; all configurable via `morph.config.json` `build` block; `--output` directory override for `morph build` | Complete |

### 🚧 In Progress

| Component | Status | Notes |
|---|---|---|
| **CSS style resolver** — full cascade, specificity, selector matching | Partial | `morph/style/selector.py` parses descendant/child/adjacent/sibling + specificity; runtime class-based hover/active rules work; full cascade still being built out |
| **JS interpreter** — JS event handler → C++ lambdas | Partial | Replaced by compile-time TS→C++ translation via `logic.so` (works for the JS subset the translator supports); interactive scripting not yet available |
| **`position: relative/fixed`** — offset positioning, sticky | Partial | `position`/`left`/`top`/`right`/`bottom` parse + feature-gated runtime fields exist; sticky in progress |
| **Forge renderer** — retained-FBO + damage tracking | Beta / buggy | Damage tracking + retained FBO shipped and toggleable in dev, but it's still **in progress** — known bugs around damage-rect edges, scroll-shift, and some compositor animation paths. **Flash is the recommended/default production renderer for now.** Per-tile LRU pool and scroll-shift remap planned in `help/renderer-flash-forge.md` Phases 4/6 |

---

## Features

**CSS Properties** (resolved from inline styles, CSS rules, and Tailwind classes)
- `width`, `height`, `max-width`, `min-width`, `max-height`, `min-height`
- `margin`, `padding` + individual side properties (`margin-top`, `padding-left`, etc.)
- `background-color`, `color` (hex, rgb, named)
- `border-radius`, `border-width`, `border-color`, `border-style`
- `box-sizing` — `content-box`, `border-box`
- `display: flex`, `flex-direction`, `justify-content`, `align-items`, `flex-wrap`, `gap`
- `flex` shorthand — `flex: 1` → `1 1 0%`, `flex: none` → `0 0 auto`, `flex: auto` → `1 1 auto`; `flex-grow`, `flex-shrink`, `flex-basis`
- `z-index` — CSS 2.1 paint-order stacking (negative, block, inline, auto, positive layers)
- `position`, `left`, `right`, `top`, `bottom`
- `overflow`, `cursor`
- `font-size` (px, %, em, bare numbers), `font-weight`, `text-align`
- `color`, `font-size`, `font-weight`, `text-align` cascade from parent to children
- `<body>` default changed from `margin: 8px` → `padding: 8px` — body background fills window edge-to-edge; no white gaps. Override with any CSS rule.
- `transform` — `translate`, `rotate`, `scale`, `skew`, `matrix`, `matrix3d`; 4x4 matrix composition; vertex-shader applied
- `animation` — `@keyframes`, `animation-name`/`duration`/`timing-function`/`delay`/`iteration-count`/`direction`/`fill-mode`; easing functions; property interpolation

**Pseudo-classes**
- `:hover` — class-based rules from CSS files (e.g. `.btn:hover`); style struct snap-copy on mouse enter/leave with correct restoration; all CSS properties above supported (color, background, margin, padding, border, border-radius, font-size, gap, flex alignment, width, height)
- `:hover` with **transitions** — `transition-duration`, `transition-timing-function` CSS properties; `transition` shorthand (`all 0.3s ease-in-out`); interpolates all numeric/color properties (bgColor, color, margin, padding, border, radius, fontSize, gap, width/height, etc.); strings (display, position, flex-direction) snap instantly; easing: linear, ease-in, ease-out, ease-in-out
- `:active` — class-based active-state rules (e.g. `.btn:active`), resolved at runtime
- **Ancestor-hover rules** — `.parent:hover .child` selectors apply/transition styles on descendants when an ancestor is hovered

**HTML Elements**
- `div`, `span`, `h1`–`h6`, `p`, `button`, `input`
- `<img>` — supports PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC via stb_image; intrinsic aspect ratio; `width`/`height` attributes; `border-radius` clipping
- `<morph-window>` — declares a native window
- Conditional rendering — `{cond && <JSX>}`, `{cond ? <A/> : <B/>}` expression containers
- JSX expressions — `{expr}` interpolation, `style={{ ... }}` objects, `className`/`id`/`morph-*` props

**JavaScript / TypeScript Runtime**
- **`morph` TS module** — every `morph init` project ships `node_modules/morph` with real `.d.ts` type definitions (`morphState`, `morphEffect`, `CSS.load`, `WindowConfig`, global `JSX` namespace), so editors give full autocomplete/type-checking on `.mx`/`.tsx` sources; runtime `index.js` is a placeholder — all Morph code is compiled to native at build time
- Compile-time TS→C++ translation (tree-sitter `TSAstBuilder` + `TSToCppTranslator`); `morph translate file.ts` emits `.cpp`
- Primitive types mapped to native types: `int`→`int`, `double`→`double`, `string`→`JsString`, `boolean`→`JsBoolean`, `any`→`JsValue`; contextual types `MouseEvent`→`MorphEvent*`, `Element`/`HTMLElement`→`MorphNode*`, `Promise`→`auto`
- Statements: variable/`const`/`let`, functions, arrow functions, classes + interfaces (inheritance, `super()`, `this`, methods, constructors), `if`/`while`/`for`/`do-while`/`switch`/`try-catch`/`throw`, `return`, `break`, `continue`
- Expressions: binary, unary, ternary, sequence, update (`++`/`--`), assignment, template literals, string/number/boolean/array/object literals, `new`, member access, calls, `await`
- JS value semantics in C++: `typeof`, truthiness, `==`/`!=`, string concatenation coercion (`"" + x`), array `push`/`pop`/index, object `has`/`keys`/index, string `split`/`trim`/`toUpperCase`/`toLowerCase`/`indexOf`/`substring`/`slice`/`replace`/`charAt`
- **Reactive state** — `morphState(initial)` returns `[getter, setter]` backed by `Signal<T>`; components re-render on change
- **Effects** — `morphEffect(fn, deps)` runs on mount and when deps change; auto-subscription; cleanup functions
- **Async / coroutines** — `async` functions compile to C++ coroutines (`morph::Task` + `morph::Result<T>`); `await fetch(url)` runs HTTP on a worker thread; `await next_frame` resumes next tick
- **Timers** — `setTimeout`, `setInterval`, `clearTimeout`
- **Networking** — `fetch(url)` → `Response` with `status`, `headers`, `ok()`, `text()`; errors surface as JS `Error` objects with `.message`
- **C++/JSX interop** — `import { fn } from './file.cpp'` imports user C++ directly; generated `_morph_state.h` exposes signals and JSX functions to C++; `native` config block for include dirs, libraries, and flags

**C++ Runtime**
- OpenGL 3.3 core profile batch renderer (instanced VAO/VBO/IBO)
- Rounded rectangles via SDF fragment shader (radius auto-clamped, `border-radius` > 100px → 100px)
- Border rendering — `border-width`, `border-color`, `border-style` via SDF shader on all elements (`div`, `button`, `img`, etc.); border ring batch (`m_borderBatch`) flushed last, on top of fills, text, and images
- Stencil-based border-radius clipping for images and child overflow (uses `GL_INCR` so nested clips properly intersect)
- `box-sizing: content-box` / `border-box` layout modes
- FreeType text rendering with per-size glyph atlas and word-wrap
- Font weight support (bold / normal with `DejaVuSans-Bold.ttf`)
- Style inheritance cascade (`color`, `font-size`, `font-weight`, `text-align`)
- Transparent backgrounds by default
- `onClick`, `MouseDown`, `MouseUp`, `MouseMove` event dispatch
- Scrollbar with drag, wheel, track-click; nested scroll containers
- Viewport culling for draw + events
- Feature-based dead code elimination
- Image rendering — stb_image-backed texture loading (PNG/JPEG/WebP/GIF/BMP/TGA/PSD/HDR/PNM/PIC); per-texture-ID batched draw calls (`m_imageBatches: unordered_map<GLuint, vector<ImageInstance>>`); `border-radius` stencil clipping on images
- Runtime `margin: auto` — sentinel `-1.0f` in style + `marginAuto[4]` flags, re-resolved dynamically on window resize
- `cursor: pointer` and `cursor: text` via GLFW standard cursors
- `:hover` pseudo-class — mouse enter/leave detection in `dispatchEvent()`; style struct snap-copy to/from `hoverStyle`; `m_baseStyle` snapshot for correct restoration; optional smooth transitions via `HoverTransition` system
- CSS transitions — `m_transitionDuration` / `m_transitionEasing` per-node config; `onHover()` starts interpolation between current style and target; `interpolateStyles()` lerps all numeric/color/position properties, snaps strings instantly; `updateHoverTransition(dt)` runs each frame from `update()`
- Window clear color set from body `background-color` each frame (falls back to white); eliminates color mismatch glitch on resize
- Dirty incremental rendering — `DirtyFlag` enum (LayoutDirty, StyleDirty, PaintDirty, ScrollDirty, SubtreeDirty); `markDirty()` propagates flags up the tree; `layoutIfNeeded()` skips clean nodes; `recordPaintTree()` only re-records display lists for paint-dirty nodes; `executeDisplayList()` replays cached `m_displayList` for all nodes every frame; `MORPH_FEATURE_DIRTY_RENDERING` compile-time gate; auto-enabled when scroll, hover, events, or cursor features are present
- **Compositor thread** — `Compositor` owns the GL context on a dedicated thread; main thread flattens a lock-free `RenderFrame` (flat nodes + display-list `DrawOp`s + `AnimationState`s) and atomically swaps pointers; compositor interpolates compositor-safe animations (X/Y, opacity, bg color, text color, border-radius) at vsync and pushes completion events back via an SPSC queue; main thread drains the feedback queue
- **Dual renderers** — `flash` (default, full-clear direct path) and `forge` (retained FBO + `DamageSet` damage tracking); production picks renderer at compile time (`"renderer": "flash" | "forge"` in config → `MORPH_RENDERER_FORGE` define), zero dead code; dev builds both and hot-switches at runtime
- **Forge retained rendering** — persistent FBO surface; per-frame damage = geometry diff (prev-rect map) ∪ pre-layout paint dirt; fullscreen forced on first frame / compositor geometry anims / node-count change; scissored color clears + stencil reset per damage rect; only nodes touching damage re-rastered; whole surface `glBlitFramebuffer` present; idle frames just blit
- **JS runtime types** — `JsValue` variant with `typeof_()`, `truthy()`, `==`, `toString()`, `operator std::string()`, `std::formatter`; `JsNumber` (int64/double/big-string variants + arithmetic), `JsString` (+ number/string concat operators), `JsArray` (shared-ptr vector), `JsObject` (shared-ptr map), `JsBoolean`, `JsUndefined`/`JsNull`
- **Reactivity** — `Signal<T>` with thread-local `EffectContext` auto-subscription, `notify_all()`, mutex-protected subscriber list; `create_effect()`/`run_pending_effects()`/`destroy_all_effects()`; `fmt_double()` / `str()` helpers for UI formatting
- **Coroutines & timers** — `morph::Task` (eager, `suspend_never` start, `suspend_always` final, scheduler-owned frame), `next_frame` awaiter, `process_tasks()`, `schedule_coroutine()`, `TimerEntry`/`set_timeout`/`set_interval`/`clear_timer`
- **CSS transforms** — `Transform` struct with `mat4` composition; `translate`/`rotate`/`scale`/`skew`/`matrix` functions parsed and composed; vertex-shader application via `MorphStyle::transformMatrix`; `MORPH_FEATURE_TRANSFORM` gate
- **CSS animations** — `Animation` struct with keyframe tracks, easing, fill-mode, iteration count; `KeyframesRegistry` global registry; per-element animation state in `MorphNode`; property interpolation at vsync; `MORPH_FEATURE_ANIMATION` gate

**DevTools (`morph_devrt` only)**
- `F12` — Toggle DevTools panel (morph-branded dark UI); docked on the right side of the window with a drag-resize handle — the app layout is constrained to the remaining content area so the panel never covers app elements
- `F2` or click "Inspect Element" — Toggle inspect mode
- Four tabs: **Elements** (inspect + node info), **Rendering** (pipeline diagnostics + live renderer switch), **Network** (fetch() request log), **Logs**
- Rendering tab: active renderer badge (Flash / Forge) + segmented Flash | Forge toggle to switch renderers live at runtime; frame counter, total nodes, layout count / skipped / percentage, repainted count / cache hit rate, layout/paint savings percentages (color-coded green/red); "Highlight repaints" toggle switch
- Network tab: request summary (total / ok / err / bytes), per-request status dot + code, method, URL, duration and body size; pending requests live-update; clear button; scrollable list; click a request to open a detail view with GENERAL, RESPONSE HEADERS, REQUEST HEADERS, and BODY preview cards (‹ Back to return); raw request/response heads captured from the actual socket
- Logs tab: ring buffer of `info` / `ok` / `warn` / `error` entries with relative timestamps; clear button
- Box-model overlay: margin (orange), border (yellow), padding (green), content (blue)
- Element info panel: tag name, size, position, margin, padding, display, overflow, box-sizing, color (hex swatch), background (hex swatch), font size, font weight, text align
- Hot reload preserves DevTools state

---

## Project Structure

```
my-app/
├── src/
│   ├── App.mx            ← entry point (JSX + CSS + JS)
│   └── components/       ← per-component CSS (shared .mx components planned)
├── cpp/                  ← optional custom C++ nodes
│   └── my_widget.h
├── assets/               ← fonts, textures, etc.
├── morph.config.json     ← project config + dependencies
└── dist/
    └── app               ← compiled binary (gitignored)
```

`morph.config.json`:
```json
{
  "name": "my-app",
  "entry": "src/App.mx",
  "window": {
    "width": 1024,
    "height": 768,
    "title": "My App"
  },
  "renderer": "flash",
  "dependencies": {},
  "cpp_sources": []
}
```
`renderer` selects the production paint backend: `"flash"` (default, lightweight full-clear) or `"forge"` (retained surface + damage tracking). In dev mode both are compiled and you can switch live from the DevTools Rendering tab.

---

## System Requirements

| | Linux | macOS | Windows |
|---|---|---|---|
| Python | 3.10+ | 3.10+ | 3.10+ |
| Compiler | g++ 11+ (C++23) | clang++ 13+ | MSVC / MinGW |
| OpenGL | 3.3+ | 3.3+ | 3.3+ |
| GLFW | `apt install libglfw3-dev` | `brew install glfw` | bundled |
| FreeType / HarfBuzz | `libfreetype-dev` `libharfbuzz-dev` | `brew install freetype harfbuzz` | bundled |

Run `morph doctor` after installing to verify your environment — it checks the toolchain (g++/C++23, cmake, make, pkg-config), graphics libs (GLFW, OpenGL, X11), text libs (FreeType, HarfBuzz), and can auto-install missing packages via your system package manager.

---

## Roadmap

### v0.1.0 (Next Up)
- [ ] **CSS style resolver** — Full cascade + selector matching (selector engine exists; runtime cascade still being built)
- [ ] **JS runtime** — Broaden the TS→C++ translator surface (more built-ins, arrays/objects in UI logic)
- [ ] **`position: relative` / `fixed`** — Offset and viewport-relative positioning (parse + runtime fields exist; sticky in progress)
- [ ] **Forge tile pool** — Content-keyed tile caching, LRU budget, scroll-shift remap (damage tracking shipped)
- [ ] **`margin` collapse** — Collapsing vertical margins between siblings

### Future
- [ ] Multi-window & navigation system
- [ ] `<morph-viewport>` embedded OpenGL canvas
- [ ] morph-icons (first-party package)
- [ ] morph-animate (animation library on top of CSS animations)
- [ ] Windows support
- [ ] VSCode extension (syntax highlighting for `.mx` files)

---

## Examples

Morph ships ready-to-run example apps under `examples/`:

| Example | What it shows |
|---|---|
| **calculator** | Full working calculator UI — `morphState` reactive state, conditional JSX rendering (`{op !== 0 && <span>…</span>}`), typed functions (`:double`, `:int`), flexbox keypad |
| **ipchecker** | Async networking — `await fetch("http://api.ipify.org")`, `Response.ok()`/`status`/`text()`, try/catch error handling, loading/error states |
| **login** | Forms & validation — controlled `<input>` with `onInput`, `type="password"` masking, inline error rendering, screen swap by unmounting the login card |
| **dynamic** | Dynamic classes & styles — template-literal `className` (`className={\`header ${theme == "light" ? "" : "bg-gray-900"}\`}`) with conditional Tailwind effects, and direct state values in inline `style` (`style={{ width: bodyWidth }}`) |
| **dynamic-styles** | Reactive class & inline styles — dynamic `className` and `style` bindings with `morphState`, demonstrating runtime style updates |

```bash
cd examples/calculator
morph dev        # or: morph run
```

---

## Contributing

Morph is in early development. Contributions, ideas, and feedback are very welcome.

```bash
git clone https://github.com/levizr/morph
cd morph
pip install -e ".[dev]"
morph doctor
```

The most impactful areas right now are the **CSS style resolver**, **TS→C++ translator coverage**, and **Forge tile pool**. See the [Current State](#current-state-v006--early-development) section for a full breakdown.

Open an issue before starting on large features so we can align on design.

---

## License

APACHE — see [LICENSE](LICENSE).

---

<div align="center">
<br/>
Built with C++ and Python &nbsp;·&nbsp; Rendered with OpenGL &nbsp;·&nbsp; No browser required
<br/><br/>
</div>
