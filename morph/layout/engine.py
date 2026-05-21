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

        # Give text leaf nodes an estimated height from font size
        if node.node_type == "__text__" and node.style.height is None and node.h == 0.0:
            node.h = node.style.font_size * 1.4

        cw = node.w - node.style.padding[3] - node.style.padding[1]
        ch = node.h - node.style.padding[0] - node.style.padding[2]
        if cw < 0: cw = 0
        if ch < 0: ch = 0
        cx, cy = node.x + node.style.padding[3], node.y + node.style.padding[0]
        for child in node.children:
            self._compute_node(child, cx, cy, cw, ch)
            cy += child.h + node.style.gap

        # Auto-height: if height not explicitly set, expand to contain children
        if node.style.height is None and node.children:
            last_bottom = cy - node.style.gap
            content_h = last_bottom - node.y + node.style.padding[2]
            if content_h > node.h:
                node.h = content_h
