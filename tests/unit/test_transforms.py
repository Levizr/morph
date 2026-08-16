import math

import pytest

from morph.style.transforms import (
    compose_transform,
    identity,
    is_identity,
    multiply,
    needs_layout,
    parse_transform,
)


# ═══════════════════════════════════════════════════════════════════
#  parse_transform — function battery
# ═══════════════════════════════════════════════════════════════════

class TestParseBattery:
    def test_none_and_keywords(self):
        assert parse_transform("none") == []
        for kw in ("inherit", "initial", "revert", "revert-layer", "unset"):
            assert parse_transform(kw) == [], kw

    def test_translate(self):
        assert parse_transform("translate(10px, 20px)") == [
            ("translate", ((10.0, "px"), (20.0, "px")))]
        # single arg → y defaults to 0
        assert parse_transform("translate(10px)") == [
            ("translate", ((10.0, "px"), (0.0, "px")))]

    def test_translate_axes(self):
        # X/Y axes normalize to their 2D forms
        assert parse_transform("translateX(5px)") == [
            ("translate", ((5.0, "px"), (0.0, "px")))]
        assert parse_transform("translateY(50%)") == [
            ("translate", ((0.0, "px"), (50.0, "%")))]
        assert parse_transform("translateZ(3px)") == [
            ("translate3d", ((0.0, "px"), (0.0, "px"), (3.0, "px")))]

    def test_translate3d(self):
        assert parse_transform("translate3d(1px, 2px, 3px)") == [
            ("translate3d", ((1.0, "px"), (2.0, "px"), (3.0, "px")))]

    def test_rotate(self):
        assert parse_transform("rotate(45deg)") == [("rotate", 45.0)]
        assert parse_transform("rotate(1.5rad)") == [("rotate", math.degrees(1.5))]
        assert parse_transform("rotate(0.25turn)") == [("rotate", 90.0)]
        assert parse_transform("rotate(100grad)") == [("rotate", 90.0)]

    def test_rotate_3d_axes(self):
        assert parse_transform("rotateX(30deg)") == [("rotatex", 30.0)]
        assert parse_transform("rotateY(30deg)") == [("rotatey", 30.0)]
        assert parse_transform("rotateZ(30deg)") == [("rotatez", 30.0)]

    def test_rotate3d(self):
        assert parse_transform("rotate3d(1, 1, 0, 45deg)") == [
            ("rotate3d", (1.0, 1.0, 0.0, 45.0))]

    def test_scale(self):
        assert parse_transform("scale(2)") == [("scale", (2.0, 2.0))]
        assert parse_transform("scale(2, 0.5)") == [("scale", (2.0, 0.5))]

    def test_scale_axes(self):
        # X/Y axes normalize to their 2D forms
        assert parse_transform("scaleX(2)") == [("scale", (2.0, 1.0))]
        assert parse_transform("scaleY(0.5)") == [("scale", (1.0, 0.5))]
        assert parse_transform("scaleZ(3)") == [("scale3d", (1.0, 1.0, 3.0))]

    def test_skew(self):
        assert parse_transform("skew(10deg, 20deg)") == [
            ("skew", (10.0, 20.0))]
        # single arg → skewY defaults to 0
        assert parse_transform("skew(15deg)") == [("skew", (15.0, 0.0))]
        # X/Y axes normalize to the 2D form
        assert parse_transform("skewX(15deg)") == [("skew", (15.0, 0.0))]
        assert parse_transform("skewY(15deg)") == [("skew", (0.0, 15.0))]

    def test_matrix(self):
        assert parse_transform("matrix(1, 0.2, 0, 1, 30, 40)") == [
            ("matrix", (1.0, 0.2, 0.0, 1.0, 30.0, 40.0))]

    def test_matrix3d(self):
        assert parse_transform(
            "matrix3d(1,0,0,0, 0,1,0,0, 0,0,1,0, 40,20,0,1)") == [
            ("matrix3d", (1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0,
                          0.0, 0.0, 1.0, 0.0, 40.0, 20.0, 0.0, 1.0))]

    def test_perspective(self):
        assert parse_transform("perspective(600px)") == [("perspective", 600.0)]

    def test_chained_list(self):
        ops = parse_transform("rotate(45deg) translate(10px, 20px) scale(0.5)")
        assert [o[0] for o in ops] == ["rotate", "translate", "scale"]

    def test_case_insensitive(self):
        assert parse_transform("Rotate(45DEG)") == [("rotate", 45.0)]

    def test_unitless_lenient(self):
        assert parse_transform("translate(10, 20)") == [
            ("translate", ((10.0, "px"), (20.0, "px")))]

    def test_invalid_ignored(self):
        assert parse_transform("rotate(45deg) bogus(3)") is None
        assert parse_transform("translate(10px,") is None
        assert parse_transform("nope") is None
        assert parse_transform("") is None
        assert parse_transform("matrix(1,2,3)") is None
        assert parse_transform("rotate3d(1, 1, 0)") is None
        assert parse_transform("translate(10px, 20px, 30px)") is None


class TestNeedsLayout:
    def test_percent_ops_need_layout(self):
        assert needs_layout(parse_transform("translateX(50%)"))
        assert needs_layout(parse_transform("translate3d(0%, 0, 0px)"))

    def test_px_only_ops_do_not(self):
        assert not needs_layout(parse_transform("rotate(45deg)"))
        assert not needs_layout(parse_transform("translate(10px, 20px)"))
        assert not needs_layout(parse_transform("translate3d(0, 0, 0px)"))


# ═══════════════════════════════════════════════════════════════════
#  compose_transform — matrix composition
# ═══════════════════════════════════════════════════════════════════

class TestCompose:
    def test_identity_when_empty(self):
        assert compose_transform([]) == identity()
        assert is_identity(compose_transform([]))

    def test_translate(self):
        m = compose_transform(parse_transform("translate(10px, 20px)"))
        assert m[12] == 10.0 and m[13] == 20.0 and m[14] == 0.0
        assert is_identity(multiply(identity(), identity()))

    def test_translate_percent_against_own_box(self):
        m = compose_transform(parse_transform("translate(50%, 25%)"),
                              own_w=200.0, own_h=100.0)
        assert m[12] == 100.0 and m[13] == 25.0

    def test_rotate(self):
        m = compose_transform(parse_transform("rotate(90deg)"))
        # column-major 2D rotation: cos=0, sin=1
        assert abs(m[0]) < 1e-9 and abs(m[1] - 1.0) < 1e-9
        assert abs(m[4] + 1.0) < 1e-9 and abs(m[5]) < 1e-9

    def test_scale(self):
        m = compose_transform(parse_transform("scale(2, 3)"))
        assert m[0] == 2.0 and m[5] == 3.0 and m[10] == 1.0

    def test_matrix6(self):
        m = compose_transform(parse_transform("matrix(1, 0.2, 0, 1, 30, 40)"))
        assert (m[0], m[1], m[4], m[5]) == (1.0, 0.2, 0.0, 1.0)
        assert (m[12], m[13]) == (30.0, 40.0)

    def test_perspective(self):
        m = compose_transform(parse_transform("perspective(600px)"))
        assert m[11] == pytest.approx(-1.0 / 600.0)
        assert m[15] == 1.0

    def test_matrix3d_passthrough(self):
        m = compose_transform(parse_transform(
            "matrix3d(1,0,0,0, 0,1,0,0, 0,0,1,0, 40,20,0,1)"))
        assert (m[12], m[13], m[14]) == (40.0, 20.0, 0.0)

    def test_chain_order(self):
        # rotate(45deg) translate(10px) == translate applied first (rightmost)
        m = compose_transform(parse_transform("rotate(90deg) translate(10px, 0px)"))
        # rotate by 90°: the (10,0) translation ends up at (0,10) in output
        assert m[12] == pytest.approx(0.0, abs=1e-9)
        assert m[13] == pytest.approx(10.0, abs=1e-9)

    def test_rotate3d_axis(self):
        m = compose_transform(parse_transform("rotate3d(0, 0, 1, 90deg)"))
        assert abs(m[0]) < 1e-9 and abs(m[1] - 1.0) < 1e-9
        assert abs(m[4] + 1.0) < 1e-9

    def test_zero_angle_rotate3d_is_identity(self):
        m = compose_transform(parse_transform("rotate3d(1, 1, 0, 0deg)"))
        assert is_identity(m)

    def test_multiply_order(self):
        a = compose_transform(parse_transform("translate(10px, 0px)"))
        b = compose_transform(parse_transform("scale(2)"))
        # (a*b) puts b's space inside a's: translate then scale
        ab = multiply(a, b)
        assert ab[12] == 10.0
        # (b*a) scales the translation: scale then translate
        ba = multiply(b, a)
        assert ba[12] == 20.0


# ═══════════════════════════════════════════════════════════════════
#  Feature detection
# ═══════════════════════════════════════════════════════════════════

class TestFeatureDetection:
    def _win(self, **kw):
        from morph.codegen.feature_set import FeatureSet
        from morph.ir.node import IRWindow, IRNode
        from morph.ir.style import IRStyle
        fs = FeatureSet()
        fs.scan([IRWindow(window_id="w1", title="T", width=400, height=300,
                          nodes=[IRNode(node_id="n1", node_type="div",
                                        style=IRStyle(**kw))])])
        return fs

    def test_static_transform_enables_feature(self):
        fs = self._win(transform_ops=[("rotate", 45.0)])
        assert "transform" in fs.features
        assert "MORPH_FEATURE_TRANSFORM" in fs.required_defines()

    def test_reactive_transform_enables_feature(self):
        from morph.codegen.feature_set import FeatureSet
        from morph.ir.node import IRWindow, IRNode
        from morph.ir.style import IRStyle
        fs = FeatureSet()
        fs.scan([IRWindow(window_id="w1", title="T", width=400, height=300,
                          nodes=[IRNode(node_id="n1", node_type="div",
                                        style=IRStyle(),
                                        reactive_style={"transform": "expr"})])])
        assert "transform" in fs.features
        assert "MORPH_FEATURE_TRANSFORM" in fs.required_defines()

    def test_no_transform_no_define(self):
        fs = self._win(width=100)
        assert "transform" not in fs.features
        assert "MORPH_FEATURE_TRANSFORM" not in fs.required_defines()


# ═══════════════════════════════════════════════════════════════════
#  End-to-end: IR builder + layout resolve % against own border-box
# ═══════════════════════════════════════════════════════════════════

class TestLayoutPipeline:
    def _make(self, src: str, css: str = ""):
        from morph.parser.morph_parser import MorphParser
        from morph.parser.jsx_walker import JSXWalker
        from morph.ir.builder import IRBuilder
        from morph.style.css_parser import CSSParser
        from morph.style.tailwind import TailwindResolver
        from morph.layout.engine import LayoutEngine
        ast = MorphParser().parse(src)
        walked = JSXWalker().walk(ast)
        css_rules = CSSParser().parse_string(css)
        ir = IRBuilder().build(walked, css_rules, TailwindResolver(project_root="."))
        LayoutEngine().compute(ir)
        return ir[0]

    def test_css_class_transform_matrix_resolved(self):
        # .card: width 200px, transform: translate(50%, 25%) — % resolves
        # against the element's own border-box (200×100) → (100, 25)
        win = self._make("""
import { CSS } from "morph"
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div className="card"></div>
    </morph-window>
  )
}
""", css="""
.card {
  width: 200px;
  height: 100px;
  transform: translate(50%, 25%);
}
""")
        node = win.nodes[0]
        assert node.w == 200.0
        m = node.style.transform_matrix
        assert m is not None
        assert m[12] == pytest.approx(100.0)
        assert m[13] == pytest.approx(25.0)

    def test_inline_style_transform_matrix_resolved(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ width: 100, height: 50, transform: 'translateX(50%)' }}></div>
    </morph-window>
  )
}
""")
        node = win.nodes[0]
        assert node.w == 100.0
        assert node.style.transform_matrix[12] == pytest.approx(50.0)

    def test_inline_block_transform_keeps_own_box(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div>
        <span style={{ display: 'inline-block', width: 40, height: 20,
                       transform: 'translateX(100%)' }}>ab</span>
      </div>
    </morph-window>
  )
}
""")
        span = win.nodes[0].children[0]
        assert span.w == 40.0
        assert span.style.transform_matrix[12] == pytest.approx(40.0)

    def test_no_transform_leaves_matrix_none(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ width: 100 }}></div>
    </morph-window>
  )
}
""")
        assert win.nodes[0].style.transform_matrix is None

    def test_invalid_transform_ignored(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ transform: 'rotate(45deg) bogus(3)' }}></div>
    </morph-window>
  )
}
""")
        assert win.nodes[0].style.transform_ops is None
        assert win.nodes[0].style.transform_matrix is None