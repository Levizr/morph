# `<viewport>` + `<canvas>` — Custom Drawing

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown
> here are proposals — they can be completely different when actually
> implemented. Supersedes the scaffold notes below where they disagree
> (the old table referenced the Python-era codebase and a driver
> signature the node never implemented).

## The split

Two elements, one shared GL context (per window, as today — no
per-viewport contexts; context switches are expensive and resource
sharing would break window isolation):

- **`<viewport>`** — C++ escape hatch. A user driver class gets full
  OpenGL 3.3 over a viewport rect (charts, editors, games, third-party
  renderers). Renders into its own FBO texture on the window's context;
  the compositor blits it (stable under damage tracking and Forge
  tiles — inline drawing would break batching).
- **`<canvas>`** — retained 2D commands from `.mx` (`fillRect`, `arc`,
  `drawImage`, text). Records into the node; the renderer executes.
  Repaint reuses dirty flags — no new loop. No raw-context escape hatch
  (breaks the retained model and context isolation — rejected the way
  suspend/resume was rejected for the page cache: tax the cold path,
  never the hot one).

## C++ syntax (all three directions)

**Define** — a driver class in a co-located `.cpp` (compiled via the
existing `cpp_sources` config), one include:

```cpp
// src/plot_view.cpp
#include "morph_api.h"
#include "morph/viewport_driver.h"

struct PlotDriver : MorphViewportDriver {
    std::vector<float> points;   // ordinary C++ state, lives with the node

    void onInit(ViewportContext& ctx) override {
        glGenBuffers(1, &vbo);
    }

    void onDraw(ViewportContext& ctx) override {
        // Rect + FBO already bound. GL state is saved/restored around
        // this call — neither side can corrupt the other.
        glViewport(0, 0, ctx.w, ctx.h);
        glBindBuffer(GL_ARRAY_BUFFER, vbo);
        glBufferData(GL_ARRAY_BUFFER, points.size() * sizeof(float),
                     points.data(), GL_DYNAMIC_DRAW);
        // ... draw calls ...
    }

    void onResize(int w, int h, ViewportContext& ctx) override {
        rebuildProjection(w, h);
    }

    void onMouseDown(int btn, ViewportContext& ctx) override {
        if (nearPoint(ctx.mouseX, ctx.mouseY)) dragging = true;
    }
};
```

Only `onDraw` is mandatory. `ViewportContext` carries `fbo`, `x/y/w/h`,
`deltaTime`, viewport-local `mouseX/mouseY`, `focused`, plus a
read-only `params` view (see below). Drivers delete their GL objects in
their destructor (runs on unmount/window close — no leaks by
construction).

**Bind** — import the class, not string props:

```tsx
import { PlotDriver } from './plot_view.cpp'

export default function App() {
  return (
    <body>
      <viewport driver={PlotDriver} width={600} height={400} id="plot" />
    </body>
  )
}
```

Typo-proof at build (unknown export = today's import error), no new
machinery. Tag `viewport` (lowercase, route-style).

**Control, C++ → viewport** — push state, request redraw (no immediate
blocking draws — drawing happens on the compositor's frame):

```cpp
app::viewport::invalidate(wid, "plot");         // mark dirty → onDraw next frame
app::viewport::setVisible(wid, "plot", false);  // skip draw, keep state
```

**Control, viewport → app** — results travel on the existing event
channels (no new bus):

```cpp
morph::channel("app::events::pointPicked").emit(JsValue(pickedIndex));
```

```tsx
morphEvent("pointPicked", (i) => setSelected(i))
```

**Control, JSX → driver** — plain props become per-frame params (UI-speed
configuration; per-frame data lives in C++):

```tsx
<viewport driver={PlotDriver} width={600} height={400} color="#ff0000" showGrid={true} />
```

```cpp
std::string color = ctx.params.get("color").as_string();  // read-only, never parsed
```

## Scaffold audit (2026-09-21)

| Layer | State |
|---|---|
| `MorphViewportDriver` + `ViewportContext` | ✅ Exists (`runtime/cpp/viewport/viewport_driver.h`) |
| `ViewportNode` | ❌ Broken — calls `driver->onDraw(r)` with a `Renderer&`; the driver takes `ViewportContext&`. Must build the context (rect from layout, delta from pump, mouse/focus from hit-test) and wrap the call in state save/restore |
| `runtime/cpp/ui/` copy | ❌ Its `#include "viewport_driver.h"` resolves inside `ui/` (no such header); dedupe to one location |
| IR / codegen / linter / feature gate | ❌ Nothing (old rows described the Python codebase) |
| Input routing | ❌ Viewport must intercept hit-test over its rect (hook: hover infra in `window.cpp`) |

## Build steps (when picked up)

1. Fix `ViewportNode` (context construction + save/restore + FBO blit), dedupe headers.
2. Tag support in the Rust linter registry + IR node + codegen emitter (`new ViewportNode(new <Driver>())`).
3. Input routing via hit-test interception.
4. Smoke test: rotating-triangle driver fixture.
5. `<canvas>` retained-2D track (shapes + text first, images second, hit-regions third).
