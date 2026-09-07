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

Morph is a UI framework that compiles `.mx` files (JSX-like syntax with TypeScript and CSS) directly into native OpenGL binaries. You write familiar web-style code — Morph produces a tiny, standalone native binary with zero browser overhead.

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

---

## Who is Morph for?

- **Web developers** who want to build native desktop apps without learning C++ or Qt
- **Desktop app developers** who want the speed of native rendering with the productivity of HTML/CSS
- **Hobbyists and tinkerers** who want to build small, fast tools and widgets
- **Anyone** tired of shipping Electron apps that bundle an entire browser

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

## When should I use Morph?

Morph is a great fit when you need:

- A lightweight desktop app with a native feel
- A UI tool, dashboard, or widget that starts fast and uses minimal resources
- A project where you want HTML/CSS for layout but don't want the overhead of a browser engine
- A learning project to explore how compilers, renderers, and UI frameworks work under the hood

Morph may **not** be the right choice yet if you need a battle-tested production framework — it's still in early development (v0.0.6).

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

**4. Build for production**
```bash
morph run         # builds and runs the production binary
morph run --static   # link everything into a single self-contained file
```

For full details, see the [Getting Started guide](docs/getting-started/quick-start.md).

---

## Where is Morph being used?

Check out the included examples under `examples/`:

| Example | What it shows |
|---|---|
| **calculator** | Reactive state, conditional rendering, flexbox keypad |
| **ipchecker** | Async networking with `await fetch()`, error handling |
| **login** | Forms, input handling, screen transitions |
| **dynamic** | Dynamic classes, template literals, Tailwind integration |
| **dynamic-styles** | Reactive inline styles and class bindings |

```bash
cd examples/calculator
morph dev
```

---

## How it works

Morph is a **compiler**, not an interpreter. Your source files never ship — only the compiled binary does.

You write `.mx` files using JSX-like syntax and CSS. Morph's Python-based compiler parses your code, builds an intermediate representation, runs layout calculations, and generates C++ code. That C++ is compiled into a native binary using OpenGL for rendering — no browser, no runtime dependencies, no garbage collector.

In **dev mode**, the compiler sends updates over a Unix socket to a pre-built renderer, so changes appear instantly. In **build mode**, it produces a standalone binary.

To dive deeper, see:
- [Architecture overview](docs/concepts/architecture.md)
- [How it works (detailed)](docs/concepts/how-it-works.md)
- [Dev mode vs Build mode](docs/concepts/dev-mode.md)

---

## Current Status (v0.0.6)

Morph is in **early development** and actively being built. Here's where things stand:

**Working:** CSS parsing, Tailwind, flexbox, layout engine, image rendering, event system, scrollbars, hover/active states, CSS transitions and animations, transforms, DevTools panel, reactive state (`morphState`), effects, async/await, coroutine task scheduler, `fetch()` API, TypeScript-to-C++ compiler, dual renderers (Flash and Forge), compositor thread, and more.

**Needs help — great places to contribute:**

| Area | Difficulty | What's needed |
|---|---|---|
| **CSS style resolver** | Medium | Full cascade + selector matching at runtime |
| **TS→C++ translator** | Medium | Broaden supported JS surface area |
| **Forge renderer tile pool** | Hard | Content-keyed tile caching, LRU, scroll-shift |
| **`position: relative/fixed`** | Medium | Offset and viewport-relative positioning |
| **Tests** | Easy | Increase coverage for layout, translator, runtime |

For a full breakdown, see the [development guide](help/development.md) or browse the [docs](docs/).

---

## Roadmap (v0.1.0)

- [ ] CSS style resolver — full cascade + selector matching
- [ ] Broader TS→C++ translator coverage
- [ ] `position: relative` / `fixed` / `sticky`
- [ ] Forge tile pool — LRU caching + scroll-shift remap
- [ ] `margin` collapse

See the full [roadmap](docs/roadmap/under-construction.md) and [future proposals](docs/future/index.md) for what's beyond v0.1.0.

---

## Project Structure

```
my-app/
├── src/
│   ├── App.mx            ← entry point (JSX + CSS + JS)
│   └── components/       ← per-component CSS
├── cpp/                  ← optional custom C++ nodes
├── assets/               ← fonts, textures, etc.
├── morph.config.json     ← project config
└── dist/
    └── app               ← compiled binary
```

See [Project Structure](docs/getting-started/project-structure.md) and [Configuration](docs/getting-started/configuration.md) for details.

---

## System Requirements

| | Linux | macOS | Windows |
|---|---|---|---|
| Python | 3.10+ | 3.10+ | 3.10+ |
| Compiler | g++ 11+ (C++23) | clang++ 13+ | MSVC / MinGW |
| OpenGL | 3.3+ | 3.3+ | 3.3+ |
| GLFW | `apt install libglfw3-dev` | `brew install glfw` | bundled |
| FreeType / HarfBuzz | `libfreetype-dev` `libharfbuzz-dev` | `brew install freetype harfbuzz` | bundled |

Run `morph doctor` after installing to verify your environment.

---

## Contributing

Contributions are very welcome — whether it's code, docs, bug reports, or ideas.

**Easiest ways to get started:**

1. Pick up one of the [needs-help areas](#current-status-v006) listed above
2. Check for [open issues](https://github.com/levizr/morph/issues) labeled `good first issue`
3. Improve documentation — the [docs](docs/) are always growing
4. Report bugs or suggest features by opening an issue

**Setup for contributors:**

```bash
git clone https://github.com/levizr/morph
cd morph
pip install -e ".[dev]"
morph doctor
```

Please read the [Contributing Guide](CONTRIBUTING.md) before submitting a pull request. We ask that you:

- Open an issue first for large features so we can align on design
- Keep PRs focused — one feature or fix per PR
- Add or update tests when possible
- Run `python -m pytest tests/ -v` before submitting

---

## Community

Morph is built by a small, enthusiastic team and we'd love to hear from you.

- **Bug reports & feature ideas** — [Open an issue](https://github.com/levizr/morph/issues)
- **Questions** — Open a discussion or issue — we're happy to help
- **Show what you built** — Share your Morph projects, we'd love to see them

Every contribution matters, no matter how small. A typo fix or a bug report is just as valuable as a new feature.

---

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). By participating, you agree to uphold its standards of fostering an open and inclusive environment. Please read it before contributing.

See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

---

## License

Apache — see [LICENSE](LICENSE).

---

<div align="center">
<br/>
Built with C++ and Python &nbsp;·&nbsp; Rendered with OpenGL &nbsp;·&nbsp; No browser required
<br/><br/>
</div>
