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


def test_static_opacity_enables_feature():
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(opacity=0.5))]))
    assert "opacity" in fs.features
    assert "MORPH_FEATURE_OPACITY" in fs.required_defines()


def test_default_opacity_no_feature():
    """opacity: 1 (the default) must not enable the feature or bloat the binary."""
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div", style=IRStyle())]))
    assert "opacity" not in fs.features
    assert "MORPH_FEATURE_OPACITY" not in fs.required_defines()


def test_hover_opacity_enables_feature():
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(),
                        hover_style=IRStyle(opacity=0.5))]))
    assert "opacity" in fs.features
    assert "MORPH_FEATURE_OPACITY" in fs.required_defines()


def test_reactive_opacity_enables_feature():
    """A reactive inline `opacity: expr` must still turn on the feature define,
    or the generated C++ (n->style.opacity = ...) wouldn't compile."""
    fs = FeatureSet()
    fs.scan(_win([IRNode(node_id="n1", node_type="div",
                        style=IRStyle(),
                        reactive_style={"opacity": "someVar"})]))
    assert "opacity" in fs.features
    assert "MORPH_FEATURE_OPACITY" in fs.required_defines()


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
