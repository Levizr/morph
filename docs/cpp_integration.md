# C++ Integration

**Status: Target design — C++ pipeline (node emitter + build compiler) not yet implemented.**

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

## Viewports

```html
<morph-viewport driver="cpp/scene.h" class="SceneRenderer" style="flex:1;" />
```

```cpp
#include <morph/viewport_driver.h>

class SceneRenderer : public MorphViewportDriver {
    void onInit(ViewportContext& ctx) override { /* setup */ }
    void onDraw(ViewportContext& ctx) override {
        glBindFramebuffer(GL_FRAMEBUFFER, ctx.fbo);
        /* your render loop */
    }
};
```

## Current Implementation

| Component | File | Status |
|---|---|---|
| C++ headers (MorphNode, Renderer, Event, WindowManager) | `runtime/core/*.h` | ✅ Declared |
| Viewport driver interface | `runtime/viewport/viewport_driver.h` | ✅ Declared |
| Jinja2 codegen templates | `morph/codegen/templates/` | ✅ 6 templates |
| Node → C++ emitter | `morph/codegen/node_emitter.py` | ❌ Stub (returns comment) |
| Event emitter | `morph/codegen/event_emitter.py` | ⚠️ Partial |
| Build compiler (g++ invocation) | (missing) | ❌ Not created |
| `morph_devrt` binary | (missing) | ❌ Not built |
| C++ implementations (.cpp) | `runtime/` | ❌ Only `.gitkeep` files |
