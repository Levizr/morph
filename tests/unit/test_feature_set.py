from morph.codegen.feature_set import FeatureSet
from morph.ir.node import IRWindow, IRNode
from morph.ir.style import IRStyle


def _win(nodes):
    return [IRWindow(window_id="w1", title="T", width=400, height=300, nodes=nodes)]


def test_static_zindex_enables_feature():
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(position="absolute", z_index=10))]))
    assert "zindex" in fs.features
    assert "MORPH_FEATURE_ZINDEX" in fs.required_defines()
    assert "position" in fs.features


def test_reactive_zindex_enables_feature():
    """A reactive inline `zIndex: expr` must still turn on the feature define,
    or the generated C++ (n->style.zIndex = ...) wouldn't compile."""
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(),
                        reactive_style={"z-index": "someVar"})]))
    assert "zindex" in fs.features
    assert "MORPH_FEATURE_ZINDEX" in fs.required_defines()


def test_reactive_position_enables_feature():
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(),
                        reactive_style={"position": "expr", "top": "expr2"})]))
    assert "position" in fs.features


def test_reactive_cursor_border_flex_enable_features():
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(),
                        reactive_style={"cursor": "c", "border-width": "b",
                                        "flex-direction": "f"})]))
    assert "cursor" in fs.features
    assert "border" in fs.features
    assert "flex" in fs.features
