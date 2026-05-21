# C++ Integration

## Custom Nodes

Place `.h` files in `cpp/` and reference them in `morph.config.json`:

```json
{ "cpp_sources": ["cpp/my_widget.h"] }
```

Inherit from `MorphNode` (header at `runtime/core/morph_node.h`):

```cpp
#include <morph/morph_node.h>

class MyWidget : public MorphNode {
public:
    void draw(Renderer& r) override {
        r.drawRoundedRect(x, y, w, h, 8.0f, style.bgColor);
    }
    // @morph-expose
    void setLabel(const std::string& s) { m_label = s; }
};
```

## Current Implementation

| Component | File | Status |
|---|---|---|
| C++ headers (MorphNode, Renderer, Event, WindowManager) | `runtime/core/*.h` | ✅ |
| OpenGL 3.3 batch renderer (instanced VAO/VBO/IBO) | `runtime/core/gl_renderer.h` | ✅ |
| Rounded rect SDF shader | embedded in `gl_renderer.h` | ✅ |
| FreeType text rendering (glyph atlas, batching) | `runtime/core/gl_renderer.h` | ✅ |
| Font weight support (bold/normal) | `runtime/core/gl_renderer.h` | ✅ |
| Widget nodes (Rect, Text, Button) | `runtime/widgets/` | ✅ |
| Node → C++ emitter | `morph/codegen/node_emitter.py` | ✅ |
| Style inheritance (color, fontSize, fontWeight) | `morph/codegen/node_emitter.py` | ✅ |
| Event emitter | `morph/codegen/event_emitter.py` | ✅ |
| Build compiler (g++ invocation) | `morph/build/compiler.py` | ✅ |
| `morph build` CLI command | `morph/cli/cmd_build.py` | ✅ |
| `morph_devrt` binary | (missing) | ❌ Not built |
| Viewport driver interface | `runtime/viewport/viewport_driver.h` | ✅ Declared |
