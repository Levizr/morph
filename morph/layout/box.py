from morph.ir.node import IRNode


def resolve_box(node: IRNode, px: float, py: float,
                parent_w: float, parent_h: float) -> tuple[float, float, float, float]:
    """Returns (x, y, w, h) for a node given its parent bounds."""
    mt, mr, mb, ml = node.style.margin

    w = node.style.width  if node.style.width  is not None else parent_w - ml - mr
    h = node.style.height if node.style.height is not None else 0.0

    x = px + ml
    y = py + mt

    return x, y, w, h
