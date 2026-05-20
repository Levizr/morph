from morph.ir.node import IRWindow, IRNode


class LayoutEngine:
    """Computes x, y, w, h for every IRNode in each window."""

    def compute(self, windows: list[IRWindow]) -> None:
        for win in windows:
            for node in win.nodes:
                self._compute_node(node, 0.0, 0.0, float(win.width), float(win.height))

    def _compute_node(self, node: IRNode, px: float, py: float,
                      parent_w: float, parent_h: float) -> None:
        from morph.layout.box import resolve_box
        node.x, node.y, node.w, node.h = resolve_box(node, px, py, parent_w, parent_h)
        cx, cy = node.x + node.style.padding[3], node.y + node.style.padding[0]
        for child in node.children:
            self._compute_node(child, cx, cy, node.w, node.h)
            cy += child.h + node.style.gap
