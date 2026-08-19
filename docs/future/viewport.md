# `<morph-viewport>` — Native OpenGL Canvas

**Status:** future · **Priority:** high

> **Note:** This is a future plan, not a commitment. The syntax and API shown here are proposals — they can be completely different when actually implemented.

An embedded OpenGL canvas element. Users drop `<morph-viewport>` anywhere in their JSX and get a native GL context inside the window — the same one the renderer uses — so they can draw whatever they want (charts, editors, games, custom widgets) with full access to OpenGL 3.3.

## Why it matters

Morph renders everything through its own batched renderer. Today there is **no way to draw arbitrary custom content** — you're limited to the built-in widgets (rect, text, button, image, input, list). A viewport unlocks:

- Custom data visualizations (graphs, plots, timelines) at native speed
- Embedded editors / canvases (drawing tools, CAD-style previews)
- Mini-games or particle systems inside an app
- Third-party C++ renderers that own their own GL state

## How it will work

The runtime scaffold already exists — the parser/builder wiring is what's missing.

### Runtime: `ViewportNode` + `MorphViewportDriver`

`morph/runtime/viewport/viewport_node.h` declares a `ViewportNode : public MorphNode` that owns a `MorphViewportDriver*`. The driver is an interface (`viewport_driver.h`) with these callbacks:

```cpp
struct ViewportContext {
    unsigned int fbo;      // viewport's framebuffer
    int x, y, w, h;        // viewport rect (screen coords)
    float deltaTime;       // seconds since last frame
    float mouseX, mouseY;  // cursor position (viewport-local)
    bool focused;          // viewport has keyboard focus
};

class MorphViewportDriver {
public:
    virtual void onInit(ViewportContext&) {}
    virtual void onResize(ViewportContext&) {}
    virtual void onDraw(ViewportContext&) = 0;          // required
    virtual void onMouseMove(ViewportContext&) {}
    virtual void onMouseDown(ViewportContext&) {}
    virtual void onScroll(ViewportContext&) {}
    virtual void onKeyDown(ViewportContext&) {}
};
```

Only `onDraw` is mandatory; everything else has empty defaults. The context hands you the viewport's FBO, position/size, frame delta, and input state.

### Planned usage

A `morph-viewport` element is configured with a driver class in a user C++ header:

```tsx
// src/App.mx
import { draw } from './my_viewport.cpp'   // exports MyViewportDriver

export default function App() {
  return (
    <morph-viewport driver="./my_viewport.cpp" driver-class="MyViewportDriver"
                    width="100%" height="400" />
  )
}
```

```cpp
// my_viewport.cpp
#include "morph/viewport_driver.h"

struct MyViewportDriver : MorphViewportDriver {
    void onDraw(ViewportContext& ctx) override {
        // glViewport(ctx.x, ctx.y, ctx.w, ctx.h);
        // draw anything — the FBO is already bound
    }
};
```

The compiler emits `ViewportNode* vp = new ViewportNode(new MyViewportDriver());` and adds it as a child of the container node.

## Current state (scaffold audit)

| Layer | State |
|---|---|
| `ViewportNode` class | ✅ Declared (`runtime/viewport/viewport_node.h` + `runtime/ui/` copy) |
| `MorphViewportDriver` interface + `ViewportContext` | ✅ Declared (`viewport_driver.h`) |
| `IRViewport` IR dataclass | ✅ Declared (`morph/ir/node.py:96`) |
| Codegen `_emit_viewport()` | ✅ Present (`node_emitter.py:1058`) — emits `new ViewportNode(new <driver_class>())` |
| Feature gate `"viewport"` + header include | ✅ In `feature_set.py` |
| JSX parsing (`jsx_walker.py`) | ❌ Nothing — tags parse generically, no viewport special-casing |
| IR building (`ir/builder.py`) | ❌ Only `morph-window` is special-cased |
| `morph check` tag registry | ❌ `SUPPORTED_TAGS` has no viewport → would flag `mx-tag` |
| Dev-mode IR serializer / deserializer | ❌ No viewport support |

## Open questions

- **Config surface** — `driver`/`driver-class` props on the element, or a `viewport` block in config?
- **Retained surface** — should the viewport render into its own FBO texture that the compositor blits (stable), or draw inline into the frame (simpler)?
- **Input routing** — hit-testing for `onMouseMove`/`onMouseDown`/`onScroll`/`onKeyDown` needs the viewport to intercept events over its rect.
- **Interaction with Forge** — viewport content can't be tile-cached; damage tracking must treat it as always-dirty or as its own retained layer.

## Build steps (when picked up)

1. Add viewport to `SUPPORTED_TAGS` in `checker/registry.py` (props: `driver`, `driver-class`, `width`, `height`)
2. Parse in `jsx_walker.py` → produce `IRViewport` in `ir/builder.py`
3. Wire the IR through the serializer + dev deserializer (`runtime/dev/ir_deserializer.h`)
4. Test with a driver that draws a rotating triangle (the canonical smoke test)