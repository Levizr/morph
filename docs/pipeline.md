# Pipeline Deep-Dive

## End-to-End Flow

```
.mx source ──► [MorphParser] ──► AST ──► [JSXWalker] ──► walked dict
                                                              │
                                                              ├── imports: [css_url, css_local, component]
                                                              └── components: [{name, params, jsx, exported}]
                                                                                │
                                                                                ▼
                                                              [CSSParser] ──► css_rules: {selector: {prop: val}}
                                                              [TailwindResolver] ──► resolved class styles
                                                                                │
                                                                                ▼
                                                              [IRBuilder] ──► list[IRWindow]
                                                                                │
                                                                                ▼
                                                              [LayoutEngine] ──► computed positions
                                                                                │
                                                                                ▼
                                                              [IRSerializer] ──► JSON-safe dict
                                                                                │
                                                              ┌─────────────────┴──────────────┐
                                                              ▼                               ▼
                                                      [Dev: IPC Socket]              [Build: C++ Codegen]
                                                      morph_devrt binary        node_emitter → g++ → binary
                                                      + logic.<hash>.so         + TS→C++ logic → g++ → logic
                                                      (dlopen, hot re-wired)    (compiled into the binary)
```

Both branches consume the **same IR dict**: dev sends it over the Unix socket to the pre-compiled `morph_devrt`, build feeds it into Jinja2 code generation. This guarantees pixel-identical dev/build rendering.

## Deep-Dive: IRBuilder

This is the heart of the compiler. Here's what it does:

### Input

```python
def build(
    self,
    walked: dict,          # from JSXWalker.walk()
    css_rules: dict,       # from CSSParser.parse_*()
    tw_resolver: TailwindResolver,  # resolves Tailwind classes
) -> list[IRWindow]:
```

**`walked` shape:**
```python
{
    "imports": [
        {"type": "css_url",   "url": "..."},
        {"type": "css_local", "path": "..."},
        {"type": "component", "path": "...", "specifiers": [...]},
    ],
    "components": [
        {
            "name": "App",
            "params": ["label", "color"],
            "jsx": {
                "tag": "div",
                "props": {"style": "..."},
                "children": [
                    {"tag": "h1", "props": {}, "children": [
                        {"tag": "__text__", "text": "Hello"}
                    ]},
                    {"tag": "button", "props": {"onClick": "..."}, "children": []},
                ],
                "self_closing": False,
            },
            "exported": True,
        }
    ],
}
```

**`css_rules` shape:**
```python
{
    ".toolbar": {
        "background-color": "#1a1a2e",
        "height": "48px",
    },
    "h1": {
        "color": "#e0e0e0",
        "font-size": "24px",
    },
}
```

### What build() Does (Implementation)

Located at `morph/ir/builder.py`. Key design:

#### Style Resolution Order

The builder implements a basic CSS cascade: **inline > Tailwind > CSS rules > defaults**

1. **Tag selectors**: Matches element tag against `css_rules` keys (e.g. `h1` → `css_rules["h1"]`)
2. **Class selectors**: Splits `className`/`class` prop, matches against `.classname` keys in `css_rules`
3. **Tailwind classes**: Each class token passed to `TailwindResolver.resolve()` which returns a CSS dict
4. **Inline styles**: Parsed from `props["style"]` dict (from JSX walker), already camelCase→kebab'd

All layers are merged via `dict.update()` with later sources overriding earlier ones.

#### Selector Matching

`morph/style/selector.py` parses selector strings into compounds + combinators:

- Descendant (`A B`), child (`A > B`), adjacent sibling (`A + B`), general sibling (`A ~ B`)
- Tag / class / id / universal compounds, `:pseudo` suffixes (including `:hover`, `:active`)
- **Ancestor-hover syntax** — `.parent:hover .child` rules that apply/transition styles on descendants
- Specificity computation per CSS spec

The runtime deserializer and codegen emit these rules so hover/active styles resolve at runtime.

#### CSS → IRStyle Conversion

The `_CSS_TO_IR` mapping handles field name differences:

| CSS property | IRStyle field | Conversion |
|---|---|---|
| `background-color` | `bg_color` | `parse_color()` → RGBA tuple |
| `color` | `color` | `parse_color()` → RGBA tuple |
| `padding`, `margin` | `padding`, `margin` | `_parse_side_value()` → 4-float tuple |
| `width`, `height` | `width`, `height` | `to_px()` → float or None |
| `border-radius` | `border_radius` | `to_px()` → float |
| `font-size` | `font_size` | `to_px()` → float |
| `gap`, `flex` | `gap`, `flex` | `to_px()` → float |
| `display` | `display` | string (e.g. `"flex"`, `"block"`) |
| `flex-direction` | `flex_dir` | string (e.g. `"row"`, `"column"`) |
| `flex-wrap` | `flex_wrap` | bool |
| `z-index` | `z_index` | int |
| `font-weight`, `text-align` | `font_weight`, `text_align` | string |

#### Events

Component event handlers (`onClick`, etc.) are collected and translated to C++ lambdas by the logic pipeline (`morph/js/` + `codegen/logic_emitter.py`) rather than the old `morph-open`/`morph-close` attribute intents (which are legacy).

### Example: What the Pipeline Produces

For the calculator example's keypad:

```python
IRWindow(  # from windowConfig export
    title="Calculator", width=340, height=500,
    page=IRPage(root=IRNode(
        type="div", id="app",
        children=[
            IRNode(type="button", className="key fn",
                   events=[IREvent(trigger="click", handler="pressClear")]),
            ...
        ])))
```

## Deep-Dive: TS/JS → C++ (Logic)

Component JS lives in `.mx` components and `.ts` modules. The `morph/js/` package translates it to C++:

1. `TSAstBuilder` (`morph/js/ast_builder.py`) — builds a typed AST from the tree-sitter TypeScript tree
2. `TSToCppTranslator` (`morph/js/codegen.py`) — emits C++ with automatic `#include` detection

Supported surface (see README for the full list): variables/`const`/`let`, functions + arrow functions, classes & interfaces (inheritance, `super`, `this`, constructors, methods), `if`/`while`/`for`/`do`/`switch`/`try-catch`/`throw`, template literals, ternary/sequence/spread/update/assignment expressions, array/object literals, `new`, member access, calls, `await`. TS type annotations map to native types (`int`, `double`, `string`→`JsString`, `boolean`→`JsBoolean`, `any`→`JsValue`, contextual `MouseEvent`→`MorphEvent*`, `Element`→`MorphNode*`, `Promise`→`auto`).

- `morphState(x)` → `Signal<T>` getter/setter pair
- `morphEffect(fn, deps)` → `morph::create_effect` with auto-subscription
- `async` functions → C++ coroutines (`morph::Task`); `await fetch(url)` → `morph::net::fetch` on a worker thread
- `setTimeout`/`setInterval` → scheduler timers

### Dev Mode: `logic.<hash>.so`

In dev, the translated logic is compiled to a content-hash-addressed shared library (`logic.<hash>.so`) loaded via `dlopen`. Hot reload re-wires signals/effects in place (`morph_logic_init`/`morph_logic_rewire`/`morph_logic_cleanup`) without re-running effects; `NodeRegistry` + `SignalStore` keep node/state references across reloads; stale `.so` files are pruned (last 3 kept).

### Build Mode

The same translated logic is emitted as C++ and compiled directly into the production binary (`morph build`).

## Deep-Dive: C++ Codegen

### Node Emitter (`morph/codegen/node_emitter.py`)

The `NodeEmitter` recursively walks the IR tree and produces C++ instantiation code:

- **`IRNode`** → `RectNode`, `TextNode`, `ButtonNode`, `ImageNode`, or `InputNode` with position/size
- **`IRStyle`** → `MorphStyle` fields (bgColor, color, borderRadius, fontSize, fontWeight, padding, etc.)
- **Style inheritance** — `color`, `font-size`, `font-weight` cascade from parent to children
- **Events** → button/`onClick` lambda registration wired to translated logic

### Logic Emitter (`morph/codegen/logic_emitter.py`)

Converts the translated JS logic + reactive styles into the C++ that backs the components: signal setup, effect registration, event handler lambdas, and default style values.

### Feature Set (`morph/codegen/feature_set.py`)

Scans the IR tree and determines which C++ headers/features are needed. Emits `#define MORPH_FEATURE_*` guards so the linker GC strips unused code:

- `text`, `button`, `scroll`, `radius`, `bold`, `position`, `flex`, `zindex`, `hover`, `active`, `input`, `dirty_rendering`, `dev_renderer_switch`, etc.

### Build Compiler (`morph/build/compiler.py`)

Invokes `g++` with the correct flags:

```
g++ -std=c++23 -O2 app.cpp /path/to/glad.c -o app \
  -I runtime -I runtime/vendor \
  -I/usr/include/freetype2 -I/usr/include/libpng16 \
  -lglfw -lGL -lX11 -lpthread -ldl -lfreetype
```

`morph build --static` additionally statically links GLFW/FreeType/HarfBuzz (requires the `.a` dev archives) into a single self-contained binary.

## C++ Runtime Architecture

### Threading + Renderers (since v0.0.6)

- **Main thread** — GLFW event pump, style, layout, paint. Flattens the tree into a lock-free `RenderFrame` snapshot.
- **Compositor thread** (`runtime/core/compositor.h`) — owns the GL context, waits on `g_framePending`, interpolates compositor-safe animations at vsync, draws through the active renderer, pushes completion events back over an SPSC queue.
- **Flash** (`runtime/renderers/flash/`) — full-clear direct path.
- **Forge** (`runtime/renderers/forge/`) — retained FBO + `DamageSet`; only damaged regions re-raster; whole surface `glBlitFramebuffer` present.
- Production selects the renderer at compile time (`constexpr`); dev compiles both and hot-switches from the DevTools Rendering tab.

### Batch Renderer (`runtime/render/gl_renderer.h` / `.cpp`)

- OpenGL 3.3 core profile
- Instanced rendering via VAO/VBO/IBO with `glDrawElementsInstanced`
- Shared unit quad (top-left origin, 0..1 range)
- Shader programs: quad (rounded rect SDF), text (glyph atlas texture), border (SDF ring)
- Stencil buffer for border-radius clipping: `GL_INCR` for proper nesting
- Flush order: fills → text → images (per-texture-ID batches) → borders (on top of everything)

### Text Rendering

- FreeType per-size glyph atlas stored in an `R8` texture
- Two-pass glyph loading: `FT_LOAD_DEFAULT` for advance metrics, `FT_LOAD_RENDER` for bitmap
- Space characters get advance without bitmap
- `_effFontSize()` / `_effFontWeight()` resolve effective font properties for alignment, wrapping, and measurement
- Per-font-size text batches flushed separately

### Image Rendering

- `stb_image`-backed: PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC
- Texture cache keyed by `src` path
- Batch-per-texture-ID: `m_imageBatches: unordered_map<GLuint, vector<ImageInstance>>`
- `border-radius` applied via stencil clipping; border ring drawn inside stencil scope

### Style Inheritance

- `color`, `font-size`, `font-weight` cascade from parent to child elements
- Intermediate containers that don't set a color still pass the inherited color to text children (hover transitions walk the parent chain)
- Default bgColor is transparent `(0,0,0,0)` — no white background unless explicitly set

### Margin Auto Runtime

- Build-time sentinel `-1.0f` for auto margins, `marginAuto[4]` flags in JSON
- C++ `MorphNode::layout()` re-resolves centering on every frame when flags are set
- Both build and dev modes share the same mechanism
