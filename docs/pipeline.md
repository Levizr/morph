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
                                                      morph_devrt binary             emitter → g++ → binary
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
| `DOMAttributes.parse()` | `morph/dom/attributes.py` | detect morph-open/close/navigate |
| `IRSerializer.to_dict()` | `morph/ir/serializer.py` | IR → JSON for dev socket |

### Example: What the Pipeline Produces

Given `src/App.mx`:

```html
<morph-window title="My App" width="800" height="600">
  <div style={{ backgroundColor: '#0f0f0f', padding: 32, gap: 16 }}>
    <h1 style={{ color: '#e8e8f0', fontSize: 28 }}>Hello, Morph!</h1>
    <p style={{ color: '#8888aa', fontSize: 14 }}>Edit src/App.mx to get started.</p>
  </div>
</morph-window>
```

`morph build` produces this IR dict (LayoutEngine computes positions):

```json
{
  "type": "app",
  "windows": [{
    "id": "node_0001", "title": "My App", "width": 800, "height": 600,
    "nodes": [{
      "id": "node_0002", "type": "div",
      "style": {
        "bg_color": [0.0588, 0.0588, 0.0588, 1.0],
        "padding": [32.0, 32.0, 32.0, 32.0],
        "display": "flex", "flex_dir": "column", "gap": 16.0
      },
      "children": [
        { "id": "node_0003", "type": "h1",
          "text": "",
          "style": { "color": [0.91, 0.91, 0.94, 1.0], "font_size": 28.0 },
          "children": [
            { "id": "node_0004", "type": "__text__",
              "text": "Hello, Morph!" }
          ]
        },
        { "id": "node_0005", "type": "p",
          "style": { "color": [0.53, 0.53, 0.67, 1.0], "font_size": 14.0 },
          "children": [
            { "id": "node_0006", "type": "__text__",
              "text": "Edit src/App.mx to get started." }
          ]
        }
      ]
    }]
  }]
}
```

All style fields (margin, border_radius, font_weight, text_align, flex, etc.) are also serialized with defaults where not specified.

## Deep-Dive: C++ Codegen (Next Step)

The IR pipeline is complete. The next step is the codegen → compile path:

1. **`IRSerializer.to_dict()`** ✅ converts IR to JSON for dev mode
2. **`FeatureSet.scan(nodes)`** ✅ determines which C++ headers are needed
3. **`NodeEmitter.emit_node(node)`** ❌ stub — returns C++ comments, not real code
4. **`EventEmitter.emit(event)`** ⚠️ partial — basic handlers work
5. **`Emitter.render(template_name, context)`** ✅ renders Jinja2 templates
6. **`Compiler.compile(cpp_source)`** ❌ module does not exist

The Jinja2 templates are complete:
- `app_main.cpp.j2` — WindowManager setup, event loop
- `window.cpp.j2` — Window instantiation
- `node_rect.cpp.j2` — RectNode with style
- `node_text.cpp.j2` — TextNode
- `node_custom.cpp.j2` — Custom C++ node
- `node_viewport.cpp.j2` — ViewportNode with driver

The missing piece is `node_emitter.py` which should map each `IRNode` to the right template and fill in the template variables, and `morph/build/compiler.py` which should invoke g++ to produce the final binary.
