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

## Deep-Dive: IRBuilder (The Critical Stub)

This is the most important unimplemented component. Here's what it needs to do:

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

### What build() Must Do

#### Step 1: Walk Components

Iterate over `walked["components"]`. For each component:

1. Create an `IRWindow` (top-level component becomes a window)
2. Walk the JSX tree recursively

#### Step 2: Resolve Styles for Each Element

For each JSX element:

1. **Tag-based CSS**: Match element tag against `css_rules` keys (e.g. `h1` → `css_rules["h1"]`)
2. **Class-based CSS**: If element has `props["class"]` or `props["className"]`, split into classes and match each against `css_rules` (e.g. `.toolbar`)
3. **Tailwind classes**: Split class string, pass each token to `tw_resolver.resolve()` which returns a CSS dict
4. **Inline styles**: Parse `props["style"]` string or dict into CSS properties
5. **Merge**: Inline > Tailwind > class CSS > tag CSS (basic cascade)
6. **Convert to IRStyle**: Map CSS string values to typed `IRStyle` fields:
   - `"#fff"` / `"white"` → `parse_color()` → `IRStyle.bg_color`
   - `"16px"` → `to_px()` → `IRStyle.padding`
   - `"true"` / `"1"` / `"flex"` → boolean flags

#### Step 3: Build IRNode Tree

For each JSX element:

1. Create `IRNode` with:
   - `node_id` from `_next_id()`
   - `node_type` from tag (`"div"`, `"h1"`, `"button"`, `"morph-window"`, etc.)
   - `style` from resolved `IRStyle`
   - `children` recursively built
2. For `morph-window` elements, wrap in `IRWindow`
3. For event attributes (`morph-open`, `morph-close`, `morph-navigate`), create `IREvent` objects

#### Step 4: Handle Special Elements

- **`<morph-window>`** → Create `IRWindow`, move attributes to window config
- **`<morph-viewport>`** → Create `IRViewport`, attach driver class
- **`<morph-page>`** → Create `IRPage`

### Reference: Utility Functions Available

| Function | Location | Purpose |
|---|---|---|
| `parse_color()` | `morph/utils/color.py` | `"#ff0000"` → `IRStyle.color` field |
| `to_px()` / resolve units | `morph/style/units.py` | `"16px"` → float; supports px, %, em |
| `TailwindResolver.resolve()` | `morph/style/tailwind.py` | `"bg-red-500"` → `{"background-color": "#ef4444"}` |
| `CSSParser.parse_string()` | `morph/style/css_parser.py` | raw CSS text → `{selector: {prop: val}}` |
| `DOMAttributes.parse()` | `morph/dom/attributes.py` | detect morph-open/close/navigate |
| `IRSerializer.to_dict()` | `morph/ir/serializer.py` | IR → JSON for dev socket |

### Example: What a Working Pipeline Should Produce

Given this `.mx` file:

```html
<morph-window title="Demo" width="800" height="600">
  <h1 class="title" style="color: #333;">Hello</h1>
  <button morph-open="modal">Open</button>
</morph-window>
```

`IRBuilder.build()` should return:

```python
[
    IRWindow(
        config={"title": "Demo", "width": 800, "height": 600},
        nodes=[
            IRNode(
                node_id="node_0001",
                node_type="h1",
                style=IRStyle(color=(0.2, 0.2, 0.2, 1.0), ...),
                children=[
                    IRNode(node_id="node_0002", node_type="__text__",
                           text="Hello")
                ],
            ),
            IRNode(
                node_id="node_0003",
                node_type="button",
                style=IRStyle(bg_color=(0.5, 0.5, 0.5, 1.0), ...),
                events=[
                    IREvent(trigger="click", action="open", target="modal")
                ],
            ),
        ],
    ),
]
```

## Deep-Dive: C++ Codegen (Post-IRBuilder)

Once `IRBuilder.build()` returns real data, the codegen path is:

1. **`IRSerializer.to_dict()`** converts IR to JSON for dev mode
2. **`FeatureSet.scan(nodes)`** determines which C++ headers are needed
3. **`NodeEmitter.emit_node(node)`** generates C++ instantiation for each node
4. **`EventEmitter.emit(event)`** generates event handler lambdas
5. **`Emitter.render(template_name, context)`** renders Jinja2 templates
6. **`Compiler.compile(cpp_source)`** (not yet built) invokes g++

The Jinja2 templates are already complete:
- `app_main.cpp.j2` — WindowManager setup, event loop
- `window.cpp.j2` — Window instantiation
- `node_rect.cpp.j2` — RectNode with style
- `node_text.cpp.j2` — TextNode
- `node_custom.cpp.j2` — Custom C++ node
- `node_viewport.cpp.j2` — ViewportNode with driver

The missing piece is `node_emitter.py` which should map each `IRNode` to the right template and fill in the template variables.
