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
