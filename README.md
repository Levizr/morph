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
[![C++](https://img.shields.io/badge/C++-17-4da6ff?style=flat-square&logo=cplusplus&logoColor=white)](https://isocpp.org)
[![OpenGL](https://img.shields.io/badge/OpenGL-3.3-f06449?style=flat-square)](https://opengl.org)
[![Status](https://img.shields.io/badge/status-early%20dev-f5a623?style=flat-square)]()

<br/>

</div>

---

## What is Morph?

Morph is a UI framework that compiles `.mx` files (JSX-like syntax with CSS and JavaScript) directly into native OpenGL binaries. You write familiar web syntax — Morph produces a lean, native binary with zero browser overhead.

```html
<!-- src/App.mx -->
<morph-window title="My App" width="800" height="600">
  <div style="height: 48px; background: #1a1a2e;">
    <button morph-open="settings">Settings</button>
  </div>

  <h1 style="color: #e0e0e0;">Hello from Morph</h1>
</morph-window>
```

```bash
morph dev      # live window, hot reload via Unix socket
morph build    # optimized native binary
```

---

## Why Morph?

| | Electron | Qt | Morph |
|---|---|---|---|
| Write UI in | HTML/CSS/JS | C++ / QML | HTML/CSS/JS |
| Runtime | Chromium (~150MB) | Qt libs | **Zero** |
| Binary size | ~80MB+ | ~20MB+ | **<1MB** |
| Native OpenGL access | ✗ | ✓ | ✓ |
| Hot reload | ✓ | ✗ | ✓ |
| Custom C++ nodes | ✗ | ✓ | ✓ |

---

## Quick Start

**1. Install**
```bash
pip install morph-ui
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
morph build
# builds native binary
```

---

## How It Works

Morph is a **compiler**, not an interpreter. Your source files never ship — only the compiled binary does.

```
src/App.mx ──► MorphParser ──► JSXWalker ──► IRBuilder ──► LayoutEngine ──► IR dict ──► C++ Codegen ──► g++ ──► native binary
                                                                                        │
                                                                                 [Dev: Unix Socket]
```

**Python** handles the entire toolchain — `.mx` parsing via tree-sitter, IR building, layout math, and Jinja2-based C++ code generation. **C++** handles the runtime — OpenGL rendering, window management, and event handling. The final binary contains zero Python and zero Node.

In **dev mode**, the pipeline produces an IR dict that is sent over a Unix socket to a pre-compiled renderer (`morph_devrt`) on every file save. The window never closes — only the node tree swaps.

---

## Current State (v0.0.4 — Early Development)

### ✅ Working

| Component | Status |
|---|---|
| **`.mx` file parsing** — tree-sitter-based JSX, imports, props | Complete |
| **CSS parsing** — local files, remote URLs, MD5-cached | Complete |
| **Tailwind CSS** — 500 common utility classes + arbitrary values | Complete |
| **IRBuilder** — walked AST → IR with inline CSS, Tailwind, color/unit conversion | Complete |
| **CLI** — `init` (interactive wizard), `dev`, `build`, `pkg`, `doctor`, `cache` | Complete |
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
| **Window config hot reload** — title update on save, node tree swap without restart | Complete |
| **DevTools panel** — F12 toggle, element inspect (F2/click), box-model overlay (margin/border/padding/content), element info panel | Complete |

### 🚧 In Progress

| Component | Status | Notes |
|---|---|---|
| **Style resolver** — CSS cascade, specificity, selector matching | Stub | Only inline + Tailwind work via IRBuilder |
| **JS interpreter** — JS event handler → C++ lambdas | Stub | Only `console.log` works |
| **Flexbox `flex-wrap`** — row overflow wrapping | Partial | Content-based sizing works, wrap still in progress |
| **`position: relative/fixed`** — offset positioning, sticky | Stub | Not yet implemented |

---

## Features

**CSS Properties** (resolved from inline styles, CSS rules, and Tailwind classes)
- `width`, `height`, `max-width`
- `margin`, `padding` + individual side properties (`margin-top`, `padding-left`, etc.)
- `background-color`, `color` (hex, rgb, named)
- `border-radius`, `border-width`, `border-color`, `border-style`
- `box-sizing` — `content-box`, `border-box`
- `display: flex`, `flex-direction`, `justify-content`, `align-items`, `flex-wrap`, `gap`
- `position`, `left`, `right`, `top`, `bottom`
- `overflow`, `cursor`
- `font-size` (px, %, em, bare numbers), `font-weight`, `text-align`
- `color`, `font-size`, `font-weight`, `text-align` cascade from parent to children

**HTML Elements**
- `div`, `span`, `h1`–`h6`, `p`, `button`
- `<img>` — supports PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC via stb_image; intrinsic aspect ratio; `width`/`height` attributes; `border-radius` clipping
- `<morph-window>` — declares a native window

**C++ Runtime**
- OpenGL 3.3 core profile batch renderer (instanced VAO/VBO/IBO)
- Rounded rectangles via SDF fragment shader (radius auto-clamped)
- Border rendering — `border-width`, `border-color`, `border-style` via SDF shader on all elements (`div`, `button`, `img`, etc.); border ring batch flushed on top of everything
- `box-sizing: content-box` / `border-box` layout modes
- FreeType text rendering with per-size glyph atlas and word-wrap
- Font weight support (bold / normal with `DejaVuSans-Bold.ttf`)
- Style inheritance cascade (`color`, `font-size`, `font-weight`, `text-align`)
- Transparent backgrounds by default
- `onClick`, `MouseDown`, `MouseUp`, `MouseMove` event dispatch
- Scrollbar with drag, wheel, track-click; nested scroll containers
- Viewport culling for draw + events
- Feature-based dead code elimination
- Image rendering — stb_image-backed texture loading (PNG/JPEG/WebP/GIF/BMP/TGA/PSD/HDR/PNM/PIC); per-texture-ID batched draw calls; `border-radius` stencil clipping on images
- `cursor: pointer` and `cursor: text` via GLFW standard cursors

**DevTools (`morph_devrt` only)**
- `F12` — Toggle DevTools panel
- `F2` or click "Inspect Element" — Toggle inspect mode
- Box-model overlay: margin (orange), border (yellow), padding (green), content (blue)
- Element info panel: tag name, size, position, margin, padding, display, overflow, box-sizing, color (hex swatch), background (hex swatch), font size, font weight, text align
- Hot reload preserves DevTools state

---

## Project Structure

```
my-app/
├── src/
│   ├── App.mx            ← entry point (JSX + CSS + JS)
│   └── components/       ← reusable .mx components
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
  "dependencies": {},
  "cpp_sources": []
}
```

---

## System Requirements

| | Linux | macOS | Windows |
|---|---|---|---|
| Python | 3.10+ | 3.10+ | 3.10+ |
| Compiler | g++ 11+ | clang++ 13+ | MSVC / MinGW |
| OpenGL | 3.3+ | 3.3+ | 3.3+ |
| GLFW | `apt install libglfw3-dev` | `brew install glfw` | bundled |

Run `morph doctor` after installing to verify your environment.

---

## Roadmap

### v0.1.0 (Next Up)
- [ ] **CSS style resolver** — Selector matching, cascade, specificity
- [ ] **JS interpreter** — Full JS expression handling (CallExpression, BinaryExpression)
- [ ] **`flex-wrap`** — Row overflow wrapping
- [ ] **`position: relative` / `fixed`** — Offset and viewport-relative positioning
- [ ] **`margin` collapse** — Collapsing vertical margins between siblings

### Future
- [ ] Multi-window & navigation system
- [ ] `<morph-viewport>` embedded OpenGL canvas
- [ ] Custom C++ node integration
- [ ] morph-icons (first-party package)
- [ ] morph-animate
- [ ] Windows support
- [ ] VSCode extension (syntax highlighting for `.mx` files)

---

## Contributing

Morph is in early development. Contributions, ideas, and feedback are very welcome.

```bash
git clone https://github.com/levizr/morph
cd morph
pip install -e ".[dev]"
morph doctor
```

The most impactful areas right now are the **CSS style resolver**, **`morph_devrt` binary**, and **JS interpreter**. See the [Current State](#current-state-v002--early-development) section for a full breakdown.

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
