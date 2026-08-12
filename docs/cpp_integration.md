# C++ Integration

Morph's runtime is C++ (OpenGL 3.3 + GLFW + FreeType). The compiler emits C++ that instantiates the node tree; component JS logic is translated to C++ and either compiled into the binary (build) or into a `logic.<hash>.so` (dev, `dlopen`ed).

## Custom Nodes

Place `.h` files in `cpp/` and reference them in `morph.config.json`:

```json
{ "cpp_sources": ["cpp/my_widget.h"] }
```

Inherit from `MorphNode` (header at `runtime/core/node.h`):

```cpp
#include "core/node.h"

class MyWidget : public MorphNode {
public:
    void recordDisplayList(Renderer& r) override {
        DrawOp op;
        op.setRounded(x, y, w, h, 8.0f, style.bgColor);
        m_displayList.push_back(op);
    }
};
```

## Runtime Source Map

| Component | File |
|---|---|
| `MorphNode` + `MorphStyle` | `runtime/core/node.h` + `runtime/core/node/` |
| `MorphWindow` (GLFW + compositor) | `runtime/core/window.h` / `.cpp` |
| Compositor thread | `runtime/core/compositor.h` / `.cpp` |
| Render frame + draw ops | `runtime/core/render_frame.h`, `draw_op.h` |
| Renderer interface | `runtime/core/renderer.h` |
| OpenGL batch renderer | `runtime/render/gl_renderer.h` / `.cpp` |
| Paint backends (Flash / Forge) | `runtime/renderers/` |
| GLSL shaders | `runtime/shaders/shader.h` |
| Widget nodes (Rect, Text, Button, Image, Input) | `runtime/ui/` |
| JS runtime types (JsValue, JsNumber, …) | `runtime/types/` |
| Reactivity (Signal, effects, coroutines) | `runtime/reactivity/` |
| Networking (`fetch()`) | `runtime/net/` |
| Node → C++ emitter | `morph/codegen/node_emitter.py` |
| Logic (TS→C++) emitter | `morph/codegen/logic_emitter.py` |
| Build compiler (g++) | `morph/build/compiler.py` |
| Dev runtime (`morph_devrt`) | `runtime/dev/` |

## Renderer Notes

- The SDF shader clamps `border-radius` to `[0.001, 100.0]`; borders render as SDF rings on a batch flushed on top of everything
- Border-radius clipping is stencil-based (`GL_INCR` so nested clips intersect correctly)
- Newer nodes implement `recordDisplayList()` (draw-op recording) rather than immediate `draw()`; display lists are cached and replayed each frame (see `MORPH_FEATURE_DIRTY_RENDERING`)
- Production picks Flash or Forge at compile time; dev builds both and hot-switches via DevTools (`MORPH_FEATURE_DEV_RENDERER_SWITCH`)
