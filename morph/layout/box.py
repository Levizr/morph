"""W3C box-model helpers used by the layout engine.

Every public function here operates on raw numbers so it can be used in
both the **measure** pass (bottom-up) and the **layout** pass (top-down).
"""

from morph.ir.node import IRNode


# ── Helpers ────────────────────────────────────────────────────


def _box_sizing_border_bonus(node: IRNode) -> tuple[float, float]:
    """Return the horizontal / vertical sizing overhead from the box model.

    For ``box-sizing: content-box`` this is ``padding + border``.
    For ``border-box`` this is zero — the explicit size already includes all three.
    """
    if node.style.box_sizing == "border-box":
        return (0.0, 0.0)
    bw = node.style.border_width
    pt, pr, pb, pl = node.style.padding
    return (pl + pr + bw * 2, pt + pb + bw * 2)


def _min_max_with_bonus(value: float | None, bonus: float) -> float | None:
    """Convert a CSS min/max size to border-box if declared as content-box."""
    if value is None:
        return None
    return value + bonus


# ── Public API ─────────────────────────────────────────────────


def resolve_border_box(node: IRNode, px: float, py: float,
                        parent_w: float, parent_h: float,
                        content_h: float | None = None,
                        ) -> tuple[float, float, float, float]:
    """Compute the raw (unclamped) **border-box** for *node*.

    Returns ``(x, y, w, h)`` without applying ``min-width / max-width /
    min-height / max-height`` — call :func:`apply_min_max` after any
    auto-height expansion to clamp the final size.

    Parameters
    ----------
    node, px, py, parent_w, parent_h
        Standard layout context (see :meth:`LayoutEngine._layout`).
    content_h
        Intrinsic content height from the measure pass, or ``None`` if
        the engine should derive height from the children during layout.
    """
    if node.style.display == "none":
        return (0.0, 0.0, 0.0, 0.0)

    mt, mr, mb, ml = node.style.margin
    h_bonus, v_bonus = _box_sizing_border_bonus(node)

    # ── Width ─────────────────────────────────────────────────
    if node.style.width is not None:
        raw_w = node.style.width
    else:
        raw_w = parent_w - ml - mr

    total_w = raw_w + h_bonus

    # ── Height ────────────────────────────────────────────────
    if node.style.height is not None:
        raw_h = node.style.height
    elif content_h is not None:
        raw_h = content_h
    else:
        raw_h = 0.0

    total_h = raw_h + v_bonus

    # Clamp negative
    total_w = max(total_w, 0.0)
    total_h = max(total_h, 0.0)

    return px + ml, py + mt, total_w, total_h


def apply_min_max(node: IRNode, dim: str = "both") -> None:
    """Apply ``min-width / max-width / min-height / max-height`` to ``node.w / h``.

    Call **after** auto-height expansion so that the min/max constraints
    are the final word on size.

    Parameters
    ----------
    node
        The node whose size should be clamped.
    dim
        ``"width"``, ``"height"``, or ``"both"`` (default).
    """
    bw = node.style.border_width
    pt, pr, pb, pl = node.style.padding
    h_bonus = pl + pr + bw * 2 if node.style.box_sizing != "border-box" else 0.0
    v_bonus = pt + pb + bw * 2 if node.style.box_sizing != "border-box" else 0.0

    if dim in ("width", "both"):
        if node.style.min_width is not None:
            min_w = node.style.min_width + h_bonus
            if node.w < min_w:
                node.w = min_w
        if node.style.max_width is not None:
            max_w = node.style.max_width + h_bonus
            if node.w > max_w:
                node.w = max_w

    if dim in ("height", "both"):
        if node.style.min_height is not None:
            min_h = node.style.min_height + v_bonus
            if node.h < min_h:
                node.h = min_h
        if node.style.max_height is not None:
            max_h = node.style.max_height + v_bonus
            if node.h > max_h:
                node.h = max_h


def resolve_content_box(node: IRNode) -> tuple[float, float, float, float]:
    """Return the **content area** inside the node's border-box.

    Returns ``(cx, cy, cw, ch)`` — the rectangle **inside** border and
    padding where children are placed.
    """
    bw = node.style.border_width
    pt, pr, pb, pl = node.style.padding
    cx = node.x + bw + pl
    cy = node.y + bw + pt
    cw = node.w - pl - pr - bw * 2
    ch = node.h - pt - pb - bw * 2
    return cx, cy, max(cw, 0.0), max(ch, 0.0)
