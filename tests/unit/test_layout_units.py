import math
from morph.ir.node import IRNode
from morph.ir.style import IRStyle
from morph.layout.box import (
    _resolve_auto_margins,
    _resolve_pct_margin,
    resolve_border_box,
    resolve_content_box,
    apply_min_max,
    _box_sizing_border_bonus,
)
from morph.style.units import DEFERRED, to_px, needs_layout, resolve


# ═══════════════════════════════════════════════════════════════════
#  Unit conversion
# ═══════════════════════════════════════════════════════════════════

class TestToPx:
    def test_px(self):
        assert to_px("23px") == 23.0

    def test_auto(self):
        assert to_px("auto") == DEFERRED

    def test_percent_no_parent(self):
        assert to_px("50%") == DEFERRED

    def test_percent_with_parent(self):
        assert to_px("50%", parent_px=800) == 400.0

    def test_vh(self):
        assert to_px("10vh") == DEFERRED

    def test_vw(self):
        assert to_px("10vw") == DEFERRED

    def test_em(self):
        assert to_px("2em") == 32.0

    def test_rem(self):
        assert to_px("2rem") == 32.0

    def test_pt(self):
        assert abs(to_px("12pt") - 16.0) < 0.001

    def test_pc(self):
        assert to_px("1pc") == 16.0

    def test_cm(self):
        assert abs(to_px("1cm") - 37.795) < 0.001

    def test_mm(self):
        assert abs(to_px("1mm") - 3.7795) < 0.001

    def test_in(self):
        assert to_px("1in") == 96.0

    def test_plain_number(self):
        assert to_px("42") == 42.0

    def test_empty_string(self):
        assert to_px("") == DEFERRED


class TestNeedsLayout:
    def test_percent(self):
        assert needs_layout("50%") is True

    def test_vh(self):
        assert needs_layout("10vh") is True

    def test_vw(self):
        assert needs_layout("10vw") is True

    def test_auto(self):
        assert needs_layout("auto") is True

    def test_empty(self):
        assert needs_layout("") is True

    def test_px(self):
        assert needs_layout("23px") is False

    def test_em(self):
        assert needs_layout("2em") is False

    def test_raw_number(self):
        assert needs_layout("42") is False


class TestResolve:
    def test_percent(self):
        assert resolve("50%", 800) == 400.0

    def test_vh(self):
        assert resolve("10vh", 0, viewport_px=600) == 60.0

    def test_vw(self):
        assert resolve("10vw", 0, viewport_px=800) == 80.0

    def test_auto_resolves_zero(self):
        assert resolve("auto", 800) == 0.0

    def test_px(self):
        assert resolve("23px", 0) == 23.0

    def test_in(self):
        assert resolve("1in", 0) == 96.0


# ═══════════════════════════════════════════════════════════════════
#  Box-model helpers
# ═══════════════════════════════════════════════════════════════════

class TestBoxModelHelpers:

    def test_box_sizing_bonus_content_box(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.padding = (5, 10, 5, 10)
        node.style.border_width = 2
        h, v = _box_sizing_border_bonus(node)
        assert h == 24  # pl(10) + pr(10) + bw*2(4)
        assert v == 14  # pt(5) + pb(5) + bw*2(4)

    def test_box_sizing_bonus_border_box(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.box_sizing = "border-box"
        node.style.padding = (5, 10, 5, 10)
        node.style.border_width = 2
        h, v = _box_sizing_border_bonus(node)
        assert h == 0
        assert v == 0

    def test_apply_min_max_clamps_width(self):
        node = IRNode(node_id="x", node_type="div")
        node.w = 300
        node.style.min_width = 400
        node.style.max_width = 500
        apply_min_max(node, dim="width")
        assert node.w == 400  # clamped up to min

    def test_apply_min_max_clamps_height(self):
        node = IRNode(node_id="x", node_type="div")
        node.h = 300
        node.style.min_height = 100
        node.style.max_height = 200
        apply_min_max(node, dim="height")
        assert node.h == 200  # clamped down to max

    def test_resolve_content_box(self):
        node = IRNode(node_id="x", node_type="div")
        node.x, node.y, node.w, node.h = 10, 20, 200, 100
        node.style.padding = (5, 10, 5, 10)
        node.style.border_width = 2
        cx, cy, cw, ch = resolve_content_box(node)
        assert cx == 22   # x + bw + pl
        assert cy == 27   # y + bw + pt
        assert cw == 176  # w - pl - pr - bw*2
        assert ch == 86   # h - pt - pb - bw*2


# ═══════════════════════════════════════════════════════════════════
#  Auto margin resolution
# ═══════════════════════════════════════════════════════════════════

class TestResolveAutoMargins:

    def test_both_auto_centers(self):
        mt, mr, mb, ml = _resolve_auto_margins(0, DEFERRED, 0, DEFERRED, 100, 500)
        assert ml == 200.0
        assert mr == 200.0

    def test_single_left_auto(self):
        mt, mr, mb, ml = _resolve_auto_margins(0, 50, 0, DEFERRED, 100, 500)
        assert ml == 350.0  # 500 - 100 - 50
        assert mr == 50

    def test_single_right_auto(self):
        mt, mr, mb, ml = _resolve_auto_margins(0, DEFERRED, 0, 50, 100, 500)
        assert mr == 350.0
        assert ml == 50

    def test_top_auto_resolves_zero(self):
        mt, mr, mb, ml = _resolve_auto_margins(DEFERRED, 0, 0, 0, 100, 500)
        assert mt == 0.0

    def test_bottom_auto_resolves_zero(self):
        mt, mr, mb, ml = _resolve_auto_margins(0, 0, DEFERRED, 0, 100, 500)
        assert mb == 0.0

    def test_no_auto_unchanged(self):
        mt, mr, mb, ml = _resolve_auto_margins(10, 20, 10, 20, 100, 500)
        assert mt == 10
        assert mr == 20
        assert mb == 10
        assert ml == 20

    def test_child_wider_than_parent(self):
        mt, mr, mb, ml = _resolve_auto_margins(0, DEFERRED, 0, DEFERRED, 600, 500)
        assert ml == 0.0  # max(avail/2, 0) = max(-50, 0) = 0
        assert mr == 0.0


# ═══════════════════════════════════════════════════════════════════
#  Percentage margin resolution
# ═══════════════════════════════════════════════════════════════════

class TestResolvePctMargin:

    def test_not_deferred(self):
        assert _resolve_pct_margin(10.0, 800, {}, "left") == 10.0

    def test_deferred_no_raw(self):
        result = _resolve_pct_margin(DEFERRED, 800, {}, "left")
        assert result == DEFERRED  # left for auto

    def test_deferred_pct_raw(self):
        result = _resolve_pct_margin(DEFERRED, 800, {"margin-left": "25%"}, "left")
        assert result == 200.0

    def test_deferred_pct_raw_shared_margin(self):
        result = _resolve_pct_margin(DEFERRED, 800, {"margin": "10%"}, "left")
        assert result == 80.0

    def test_deferred_pct_raw_shared_margin_right(self):
        result = _resolve_pct_margin(DEFERRED, 800, {"margin": "10%"}, "right")
        assert result == 80.0

    def test_deferred_non_pct_raw(self):
        result = _resolve_pct_margin(DEFERRED, 800, {"margin": "20px"}, "left")
        assert result == DEFERRED  # not a %, keep for auto


# ═══════════════════════════════════════════════════════════════════
#  resolve_border_box — full integration
# ═══════════════════════════════════════════════════════════════════

class TestResolveBorderBox:

    def test_simple_block(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (0, 0, 0, 0)
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert x == 0
        assert y == 0
        assert w == 100
        assert h >= 0

    def test_with_margin(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (10, 20, 10, 20)
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 50, 50, 800, 600)
        assert x == 70   # px(50) + ml(20)
        assert y == 60   # py(50) + mt(10)
        assert w == 100
        assert h >= 0

    def test_auto_margin_centering(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (0, DEFERRED, 0, DEFERRED)
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert x == 350.0  # (800 - 100) / 2
        assert w == 100

    def test_auto_margin_single_left(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (0, 50, 0, DEFERRED)
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 0, 0, 500, 600)
        assert x == 350.0  # 500 - 100 - 50
        assert w == 100

    def test_display_none(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.display = "none"
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert (x, y, w, h) == (0, 0, 0, 0)

    def test_content_height_passed(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (0, 0, 0, 0)
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600, content_h=50.0)
        assert h == 50.0

    def test_border_box_sizing(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.box_sizing = "border-box"
        node.style.width = 200.0
        node.style.padding = (10, 10, 10, 10)
        node.style.border_width = 2
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert w == 200  # border-box includes padding+border

    def test_content_box_sizing(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.box_sizing = "content-box"
        node.style.width = 200.0
        node.style.padding = (10, 10, 10, 10)
        node.style.border_width = 2
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert w == 224  # 200 + 10*2 + 2*2

    def test_no_width_fills_parent(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (0, 20, 0, 20)
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert w == 760  # 800 - 20 - 20

    def test_content_h_none_no_height(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600, content_h=None)
        assert h == 0.0

    def test_percentage_margin_from_raw(self):
        node = IRNode(node_id="x", node_type="div")
        node.style.margin = (0, DEFERRED, 0, DEFERRED)
        node.raw_styles = {"margin": "10%"}
        node.style.width = 100.0
        x, y, w, h = resolve_border_box(node, 0, 0, 800, 600)
        assert x == 80.0  # 10% of 800


# ═══════════════════════════════════════════════════════════════════
#  End-to-end layout with inline styles
# ═══════════════════════════════════════════════════════════════════

class TestLayoutPipeline:

    def _make(self, src: str):
        from morph.parser.morph_parser import MorphParser
        from morph.parser.jsx_walker import JSXWalker
        from morph.ir.builder import IRBuilder
        from morph.style.tailwind import TailwindResolver
        from morph.layout.engine import LayoutEngine
        ast = MorphParser().parse(src)
        walked = JSXWalker().walk(ast)
        ir = IRBuilder().build(walked, {}, TailwindResolver(project_root="."))
        LayoutEngine().compute(ir)
        return ir[0]

    def test_auto_margin_centers_block(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ margin: '0 auto', width: 100 }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.x == 350.0, f"x={d.x}, expected 350"
        assert d.w == 100.0

    def test_percentage_width(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ width: '50%' }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.w == 400.0, f"w={d.w}, expected 400"

    def test_percentage_height(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ width: 100, height: '30%' }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.h == 180.0, f"h={d.h}, expected 180"

    def test_margin_shorthand_2value(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ margin: '10px 20px', width: 100 }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.x == 20.0, f"x={d.x}, expected 20"
        assert d.y == 10.0, f"y={d.y}, expected 10"
        assert d.w == 100.0

    def test_margin_shorthand_3value(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ margin: '10px 20px 30px', width: 100 }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.x == 20.0, f"x={d.x}, expected 20"
        assert d.y == 10.0, f"y={d.y}, expected 10"
        assert d.w == 100.0

    def test_margin_shorthand_4value(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ margin: '5px 10px 15px 20px', width: 100 }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.x == 20.0, f"x={d.x}, expected 20 (left)"
        assert d.y == 5.0, f"y={d.y}, expected 5 (top)"

    def test_children_stack_vertically(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div>
        <div style={{ width: 100, height: 30, backgroundColor: 'red' }}></div>
        <div style={{ width: 100, height: 50, backgroundColor: 'blue' }}></div>
      </div>
    </morph-window>
  )
}
""")
        parent = win.nodes[0]
        c1, c2 = parent.children
        assert c1.y == parent.y, "first child should be at parent top"
        assert c2.y >= c1.y + c1.h, "second child should be below first"

    def test_percentage_margin_via_inline(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ marginLeft: '10%', width: 100 }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.x == 80.0, f"x={d.x}, expected 80 (10% of 800)"

    def test_box_sizing_content_vs_border(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ width: 200, padding: '10px', border: '2px solid black', boxSizing: 'border-box' }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.w == 200.0  # border-box: padding+border inside

    def test_display_none_removes_element(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ display: 'none', width: 100 }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        assert d.w == 0.0
        assert d.h == 0.0

    def test_margin_auto_no_width(self):
        win = self._make("""
export default function App() {
  return (
    <morph-window title="T" width={800} height={600}>
      <div style={{ margin: '0 auto' }}></div>
    </morph-window>
  )
}
""")
        d = win.nodes[0]
        # No explicit width → fills parent width (800), auto margins → 0
        assert d.w == 800.0
        assert d.x == 0.0
