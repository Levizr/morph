# C++ Integration

## Custom Nodes

Place `.h` files in `cpp/` and reference them in `morph.config.json`:

```json
{ "cpp_sources": ["cpp/my_widget.h"] }
```

Inherit from `MorphNode`:

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
