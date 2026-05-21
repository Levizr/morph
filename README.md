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

**Build native OpenGL Applicationswith HTML, CSS, and JavaScript.**

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

## Current State (Early Development)

Morph's pipeline is under active development. Here's what's working and what's still being built:

### ✅ Working

| Component | Status |
|---|---|
| **`.mx` file parsing** — tree-sitter-based JSX, imports, props | Complete |
| **CSS parsing** — local files, remote URLs, MD5-cached | Complete |
| **Tailwind CSS** — 500 common utility classes + arbitrary values | Complete |
| **IRBuilder** — walked AST → IR with inline CSS, Tailwind, color/unit conversion | Complete |
| **CLI** — `init` (interactive wizard), `dev`, `build`, `pkg`, `doctor` (advanced), `cache` | Complete |
| **Config** — `morph.config.json` load/save | Complete |
| **IR data models** — `IRNode`, `IRWindow`, `IRPage`, `IRViewport`, `IRStyle`, `IREvent` | Complete |
| **IR serializer** — JSON-safe dict for dev socket | Complete |
| **Layout engine** — box model (margin, padding), vertical stacking, gap | Complete |
| **Dev file watcher** — watchdog-based with debounce | Complete |
| **Unix socket IPC** — sends IR to dev runtime | Complete |
| **Codegen templates** — Jinja2 templates for C++ output | Complete |
| **Package registry client** — fetch, install, manifest parsing | Complete |
| **Color utilities** — hex/rgb parsing and conversion | Complete |
| **System doctor** — version checks, dependency diagnostics | Complete |
| **Inline style resolution** — camelCase→kebab, color parsing, unit conversion | Complete |
| **Event extraction** — `morph-open`, `morph-close`, `morph-navigate` → IREvent | Complete |

### 🚧 In Progress

| Component | Status | Notes |
|---|---|---|
| **Layout engine** — flexbox | Partial | Vertical stacking works, `apply_flex()` is stub |
| **Style resolver** — CSS cascade, specificity, selector matching | Stub | Only inline + Tailwind work via IRBuilder |
| **JS interpreter** — JS event handler → C++ lambdas | Stub | Only `import` works |
| **C++ node emitter** — IR → C++ instantiation code | Stub | Templates exist, emitter returns comments |
| **`morph_devrt` binary** — dev mode renderer | Missing | Binary not yet built |
| **Build compiler** — g++ invocation + binary output | Missing | Module not created |

---

## Features

**CSS Properties** — all resolved from inline styles, CSS rules, and Tailwind classes
- `width`, `height`
- `margin`, `padding` (shorthand: 1-4 values → all sides)
- `background-color`, `color` (hex, rgb, named)
- `border-radius`
- `display: flex`, `flex-direction`, `flex`, `gap`
- `font-size` (px, %, em, bare numbers), `font-weight`, `text-align`
- Unit conversion via `to_px()` with px/%/em support

**HTML Elements**
- `div`, `span`, `h1`–`h6`, `p`, `button`, `input`
- `<morph-window>` — declares a native window
- `<morph-page>` — navigable page within a window
- `<morph-viewport>` — raw OpenGL canvas embedded in layout

**Navigation & Windows**
```html
<button morph-open="settings">Open Settings</button>
<button morph-close="settings">Close</button>
<button morph-navigate="about">Go to About</button>
```

**JavaScript**
```js
// parsed as AST, compiled to C++ lambdas
import { Icon } from 'morph-icons'

const icon = new Icon('settings', { size: 24, color: '#fff' })
icon.mount('#toolbar')
```

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

### Next Up (Priority Order)
- [x] **IRBuilder** — Convert walked AST + CSS + Tailwind into IR nodes ✅
- [ ] **Flexbox layout** — `display: flex` support in layout engine
- [ ] **CSS style resolver** — Selector matching, cascade, specificity
- [ ] **C++ node emitter** — Generate actual C++ instantiation from IR
- [ ] **`morph build`** — compiler module to invoke g++ and produce binary
- [ ] **`morph_devrt` binary** — Pre-compiled renderer for dev mode
- [ ] **JS interpreter** — JS expression handling (NewExpression, CallExpression)

### Future
- [ ] Multi-window & navigation system
- [ ] `<morph-viewport>` embedded OpenGL canvas
- [ ] Custom C++ node integration
- [ ] Text rendering (FreeType + SDF)
- [ ] `border-radius` shader (SDF-based)
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

The most impactful areas right now are the **flexbox layout engine**, **C++ node emitter**, and the **`morph build` compiler module**. See the [Current State](#current-state-early-development) section for a full breakdown.

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
