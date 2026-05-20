<div align="center">

<br/>

```
███╗   ███╗ ██████╗ ██████╗ ██████╗ ██╗  ██╗
████╗ ████║██╔═══██╗██╔══██╗██╔══██╗██║  ██║
██╔████╔██║██║   ██║██████╔╝██████╔╝███████║
██║╚██╔╝██║██║   ██║██╔══██╗██╔═══╝ ██╔══██║
██║ ╚═╝ ██║╚██████╔╝██║  ██║██║     ██║  ██║
╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝
```

**Build native OpenGL UIs with HTML, CSS, and JavaScript.**

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

Morph is a UI framework that compiles HTML, CSS, and JavaScript directly into OpenGL draw calls. You write familiar web syntax — Morph produces a lean, native binary with zero browser overhead.

```html
<!-- src/index.html -->
<morph-window title="My App" width="800" height="600">
  <div id="toolbar" style="height: 48px; background: #1a1a2e;">
    <button morph-open="settings">⚙ Settings</button>
  </div>

  <morph-viewport driver="cpp/scene.h" class="SceneRenderer" style="flex: 1;" />
</morph-window>
```

```bash
morph dev      # live window, hot reload in ~10ms
morph build    # optimized native binary → dist/app
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

A native window opens. Edit `src/index.html`, `src/style.css`, or `src/app.js` — the window updates instantly without restarting.

**4. Ship**
```bash
morph build
# → dist/app  (native binary, no runtime required)
```

---

## How It Works

Morph is a **compiler**, not an interpreter. Your source files never ship — only the compiled binary does.

```
src/index.html ─┐
src/style.css  ─┼─► Python Parser ─► MorphIR ─► C++ Codegen ─► g++ ─► dist/app
src/app.js     ─┘
```

**Python** handles the entire toolchain — parsing, IR building, layout math, and Jinja2-based C++ code generation. **C++** handles the runtime — OpenGL rendering, window management, and event handling. The final binary contains zero Python and zero Node.

In **dev mode**, a pre-compiled generic renderer (`morph_devrt`) stays alive and receives updated IR over a Unix socket on every file save. The window never closes — only the node tree swaps.

---

## Features

**CSS Properties (current)**
- `width`, `height`, `min-width`, `min-height`
- `margin`, `padding` (all sides)
- `background-color`, `color`
- `border-radius`
- `display: flex`, `flex-direction`, `flex`, `gap`
- `font-size`, `font-weight`, `text-align`

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
// app.js — parsed as AST, compiled to C++ lambdas
import { Icon } from 'morph-icons'

const icon = new Icon('settings', { size: 24, color: '#fff' })
icon.mount('#toolbar')
```

---

## Custom C++ Nodes

Drop into C++ whenever you need full control.

```cpp
// cpp/my_button.h
#include <morph/morph_node.h>

class MyButton : public MorphNode {
    std::string m_label;

public:
    MyButton(const std::string& label) : m_label(label) {}

    void draw(Renderer& r) override {
        r.drawRoundedRect(x, y, w, h, 8.0f, style.bgColor);
        r.drawText(m_label, x + w/2, y + h/2, style.color, TextAlign::Center);
    }

    // @morph-expose — callable from JS
    void setLabel(const std::string& label) { m_label = label; }
};
```

Register in `morph.config.json`:
```json
{
  "cpp_sources": ["cpp/my_button.h"]
}
```

Use in HTML:
```html
<my-button style="width: 120px; height: 40px;">Click me</my-button>
```

---

## Viewports — Embedded OpenGL Canvas

Build game editors, video editors, CAD tools, or any application that needs a raw OpenGL canvas inside a Morph UI layout.

```html
<morph-viewport
  id="scene"
  driver="cpp/scene_renderer.h"
  class="SceneRenderer"
  style="flex: 1;"
/>
```

```cpp
// cpp/scene_renderer.h
#include <morph/viewport_driver.h>

class SceneRenderer : public MorphViewportDriver {
    Camera m_camera;

    void onInit(ViewportContext& ctx) override {
        // full OpenGL setup — load shaders, meshes, etc.
    }

    void onDraw(ViewportContext& ctx) override {
        glBindFramebuffer(GL_FRAMEBUFFER, ctx.fbo);
        // your render loop here
    }

    void onMouseMove(float x, float y, ViewportContext& ctx) override {
        m_camera.orbit(x, y);
    }

    // @morph-expose
    void loadMesh(const std::string& path) { /* ... */ }
};
```

Call from JS:
```js
document.querySelector('#scene').call('loadMesh', 'assets/robot.obj')
```

---

## Package System

```bash
morph pkg add morph-icons
morph pkg add morph-charts
morph pkg list
morph pkg install        # restore from morph.config.json (like npm install)
```

Packages are hosted on GitHub and indexed at [morph-ui.dev](https://morph-ui.dev). No central file server — the registry is just a metadata index.

**Writing a package** — two files required:

```
my-package/
├── morph.pkg.json
├── js/index.js          ← JS API with @morph-component annotations
└── runtime/renderer.h   ← C++ header
```

```json
// morph.pkg.json
{
  "name": "my-package",
  "version": "1.0.0",
  "type": "morph-native",
  "js_entry": "js/index.js",
  "runtime_headers": ["runtime/renderer.h"],
  "github": "you/my-package"
}
```

---

## Project Structure

```
my-app/
├── src/
│   ├── index.html       ← entry point
│   ├── style.css
│   └── app.js
├── cpp/                 ← optional custom C++ nodes
│   └── my_widget.h
├── assets/              ← fonts, textures, etc.
├── morph.config.json    ← project config + dependencies
└── dist/
    └── app              ← compiled binary (gitignored)
```

`morph.config.json`:
```json
{
  "name": "my-app",
  "entry": "src/index.html",
  "window": {
    "width": 1024,
    "height": 768,
    "title": "My App"
  },
  "dependencies": {
    "morph-icons": "1.0.0"
  },
  "cpp_sources": []
}
```

---

## System Requirements

| | Linux | macOS | Windows |
|---|---|---|---|
| Python | 3.10+ | 3.10+ | 3.10+ |
| Compiler | g++ 11+ | clang++ 13+ | MSVC / MinGW |
| OpenGL | 4.1+ | 4.1+ | 4.1+ |
| GLFW | `apt install libglfw3-dev` | `brew install glfw` | bundled |

Run `morph doctor` after installing to verify your environment.

---

## Roadmap

- [x] Core pipeline — HTML/CSS parse → IR → C++ emit → compile
- [x] Dev mode — persistent window, Unix socket hot reload
- [x] Multi-window & navigation system
- [x] `<morph-viewport>` embedded OpenGL canvas
- [x] Custom C++ node integration
- [x] Package registry design
- [ ] Box model layout engine (margin, padding, flex)
- [ ] Text rendering (FreeType + SDF)
- [ ] `border-radius` shader (SDF-based)
- [ ] `morph pkg` CLI — install, remove, search
- [ ] morph-icons (first-party package)
- [ ] morph-animate
- [ ] Windows support
- [ ] VSCode extension (syntax highlighting for `.morph` components)

---

## Contributing

Morph is in early development. Contributions, ideas, and feedback are very welcome.

```bash
git clone https://github.com/yourusername/morph
cd morph
pip install -e ".[dev]"
morph doctor
```

Areas that need work: layout engine, text rendering, Windows support, and the package registry website. Open an issue before starting on large features so we can align on design.

---

## License

MIT — see [LICENSE](LICENSE).

---

<div align="center">
<br/>
Built with C++ and Python &nbsp;·&nbsp; Rendered with OpenGL &nbsp;·&nbsp; No browser required
<br/><br/>
</div>