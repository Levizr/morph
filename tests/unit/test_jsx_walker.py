from morph.parser.morph_parser import MorphParser
from morph.parser.jsx_walker import JSXWalker


SOURCE = """
import Button from './Button.mx'
import './global.css'

export default function App() {
  return (
    <morph-window title="Test" width={800} height={600}>
      <div style={{ backgroundColor: '#111', padding: 16 }}>
        <Button label="Click" color="#7c6af5" />
      </div>
    </morph-window>
  )
}
"""


def test_extracts_imports():
    root   = MorphParser().parse(SOURCE)
    walked = JSXWalker().walk(root)
    types  = [i["type"] for i in walked["imports"]]
    assert "component" in types
    assert "css_local" in types


def test_extracts_component():
    root   = MorphParser().parse(SOURCE)
    walked = JSXWalker().walk(root)
    assert len(walked["components"]) >= 1
    assert walked["components"][0]["name"] == "App"


def test_jsx_root_tag():
    root   = MorphParser().parse(SOURCE)
    walked = JSXWalker().walk(root)
    jsx    = walked["components"][0]["jsx"]
    assert jsx["tag"] == "morph-window"


def test_style_camel_to_kebab():
    root   = MorphParser().parse(SOURCE)
    walked = JSXWalker().walk(root)
    jsx    = walked["components"][0]["jsx"]
    child  = jsx["children"][0]
    style  = child["props"].get("style", {})
    assert "background-color" in style


def test_negative_numeric_style_is_static():
    """Inline `zIndex: -1` parses as a static value, not a reactive expr."""
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div style={{ position: 'absolute', zIndex: -1, top: 10 }} />
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    div    = walked["components"][0]["jsx"]["children"][0]
    style  = div["props"]["style"]
    assert style["z-index"] == "-1", f"Expected static '-1', got {style['z-index']!r}"
    assert style["position"] == "absolute"
    assert style["top"] == "10"


# ── Keyed lists ({arr.map(item => <JSX/>)}) ─────────────────


def _first_map(jsx):
    """First __list__ child in the given jsx subtree."""
    for child in jsx.get("children", []):
        if child.get("tag") == "__list__":
            return child
        found = _first_map(child)
        if found:
            return found
    return None


def test_map_keyed_single_param():
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div>
            {items.map(item => <div key={item.id}>{item.name}</div>)}
          </div>
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    lst = _first_map(walked["components"][0]["jsx"])
    assert lst is not None
    assert lst["array_expr"] == "items"
    assert lst["item_param"] == "item"
    assert lst["index_param"] == ""
    assert lst["key_expr"] == "item.id"
    assert lst["item_template"]["tag"] == "div"
    assert lst["item_template"]["props"].get("key") is None, "key prop must be removed"
    assert lst["item_template"]["children"][0]["tag"] == "__expr__"


def test_map_two_params():
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div>
            {tags.map((t, i) => <span key={i}>{t}</span>)}
          </div>
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    lst = _first_map(walked["components"][0]["jsx"])
    assert lst["item_param"] == "t"
    assert lst["index_param"] == "i"
    assert lst["key_expr"] == "i"
    assert lst["item_template"]["tag"] == "span"


def test_map_block_body():
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div>
            {rows.map(row => { return <li key={row.k}>{row.v}</li> })}
          </div>
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    lst = _first_map(walked["components"][0]["jsx"])
    assert lst is not None
    assert lst["key_expr"] == "row.k"
    assert lst["item_template"]["tag"] == "li"


def test_map_fragment_body():
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div>
            {flags.map(f => <>[{f}]</>)}
          </div>
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    lst = _first_map(walked["components"][0]["jsx"])
    assert lst is not None
    assert lst["key_expr"] == ""
    assert lst["item_template"]["tag"] == "__fragment__"


def test_map_without_key_uses_index_fallback():
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div>
            {names.map(n => <p>{n}</p>)}
          </div>
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    lst = _first_map(walked["components"][0]["jsx"])
    assert lst["key_expr"] == ""


def test_non_map_expression_stays_expr():
    src = """
    export default function App() {
      return (
        <morph-window title="T" width={400} height={300}>
          <div>
            {items.length}
            {items.map(renderItem)}
          </div>
        </morph-window>
      )
    }
    """
    root   = MorphParser().parse(src)
    walked = JSXWalker().walk(root)
    children = walked["components"][0]["jsx"]["children"][0]["children"]
    assert all(c["tag"] == "__expr__" for c in children)
