# Custom C++ Nodes

Extend Morph by creating custom C++ node types that inherit from `MorphNode`.

## Creating a Custom Node

Place `.h` files in your project and reference them in `morph.config.json`:

```json
{
  "cpp_sources": ["cpp/my_widget.h"]
}
```

Then implement your node:

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

## MorphNode API

Key methods to override:

| Method | Description |
|---|---|
| `recordDisplayList(Renderer& r)` | Record draw operations for rendering |
| `layout(float parentW, float parentH)` | Compute position and size |
| `contentWidth()` | Return intrinsic width for flex sizing |

## Key Files

| File | Description |
|---|---|
| `runtime/core/node.h` | `MorphNode` base class and `MorphStyle` |
| `runtime/core/render_frame.h` | Lock-free frame snapshot |
| `runtime/core/draw_op.h` | Display-list draw operations |
| `runtime/render/gl_renderer.h` | OpenGL batch renderer |

See the [C++ Integration help doc](../../help/cpp_integration.md) for the full runtime source map.
