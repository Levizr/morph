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
```

## Deep-Dive: IRBuilder

This is the heart of the compiler — now fully implemented. Here's what it does:

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
                    {"tag": "button", "props": {"morph-open": "settings"}, "children": []},
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
| `font-weight`, `text-align` | `font_weight`, `text_align` | string |

#### Event Extraction

The builder scans props for `morph-open`, `morph-close`, `morph-navigate` and creates `IREvent(trigger="click", action=..., target=...)` for each.

#### Special Elements

- **`<morph-window>`** → `IRWindow` with window config (title, width, height)
- **`<morph-viewport>`**, **`<morph-page>`** → detected but handled generically (IRViewport/IRPage creation not yet fully wired)

### Example: What the Pipeline Produces

### Reference: Utility Functions Available

| Function | Location | Purpose |
|---|---|---|
| `parse_color()` | `morph/utils/color.py` | `"#ff0000"` → `IRStyle.color` field |
| `to_px()` / resolve units | `morph/style/units.py` | `"16px"` → float; supports px, %, em |
| `TailwindResolver.resolve()` | `morph/style/tailwind.py` | `"bg-red-500"` → `{"background-color": "#ef4444"}` |
| `CSSParser.parse_string()` | `morph/style/css_parser.py` | raw CSS text → `{selector: {prop: val}}` |
| `IRSerializer.to_dict()` | `morph/ir/serializer.py` | IR → JSON for dev socket |

## Deep-Dive: C++ Codegen

### Node Emitter (`morph/codegen/node_emitter.py`)

The `NodeEmitter` recursively walks the IR tree and produces C++ instantiation code:

- **`IRNode`** → `RectNode` or `TextNode` or `ButtonNode` with position/size
- **`IRStyle`** → `MorphStyle` fields (bgColor, color, borderRadius, fontSize, fontWeight, padding)
- **Style inheritance** — `color`, `font-size`, `font-weight` cascade from parent to children
- **Events** → `ButtonNode::onClick` lambda registration

### Feature Set (`morph/codegen/feature_set.py`)

Scans the IR tree and determines which C++ headers must be included:

- `morph_rect.h` for div/span/h1–h6/p
- `morph_text.h` for text nodes
- `morph_button.h` for buttons with events
- `morph_radius.h` for elements with border-radius

### Build Compiler (`morph/build/compiler.py`)

Invokes `g++` with the correct flags:

```
g++ -std=c++17 -O2 app.cpp /path/to/glad.c -o app \
  -I runtime -I runtime/vendor \
  -I/usr/include/freetype2 -I/usr/include/libpng16 \
  -lglfw -lGL -lX11 -lpthread -ldl -lfreetype
```

## C++ Runtime Architecture

### Renderer (`runtime/render/gl_renderer.h` / `.cpp`)

- OpenGL 3.3 core profile
- Instanced rendering via VAO/VBO/IBO with `glDrawElementsInstanced`
- Shared unit quad (top-left origin, 0..1 range)
- Three shader programs: quad (rounded rect SDF), text (glyph atlas texture), border (SDF ring)
- Stencil buffer for border-radius clipping: `GL_INCR` for proper nesting
- Flush order: fills → text → images (per-texture-ID batches) → borders (on top of everything)

### Text Rendering

- FreeType per-size glyph atlas stored in an `R8` texture
- Two-pass glyph loading: `FT_LOAD_DEFAULT` for advance metrics, `FT_LOAD_RENDER` for bitmap
- Space characters get advance without bitmap
- Per-font-size text batches flushed separately

### Image Rendering

- `stb_image`-backed: PNG, JPEG, WebP, GIF, BMP, TGA, PSD, HDR, PNM, PIC
- Texture cache keyed by `src` path
- Batch-per-texture-ID: `m_imageBatches: unordered_map<GLuint, vector<ImageInstance>>`
- `border-radius` applied via stencil clipping; border ring drawn inside stencil scope

### Style Inheritance

- `color`, `font-size`, `font-weight` cascade from parent to child elements
- Intermediate containers that don't set a color still pass the inherited color to text children
- Default bgColor is transparent `(0,0,0,0)` — no white background unless explicitly set

### Margin Auto Runtime

- Build-time sentinel `-1.0f` for auto margins, `marginAuto[4]` flags in JSON
- C++ `MorphNode::layout()` re-resolves centering on every frame when flags are set
- Both build and dev modes share the same mechanism
