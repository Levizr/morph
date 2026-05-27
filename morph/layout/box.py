"""W3C box-model helpers used by the layout engine.

Every public function here operates on raw numbers so it can be used in
both the **measure** pass (bottom-up) and the **layout** pass (top-down).
"""

import math
from morph.ir.node import IRNode
from morph.style.units import DEFERRED, resolve


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


def _resolve_auto_margins(mt: float, mr: float, mb: float, ml: float,
                           child_w: float, parent_w: float,
                           ) -> tuple[float, float, float, float]:
    """Resolve ``auto`` margins (stored as ``DEFERRED`` sentinel).
    
    In normal flow, auto left/right margins split the remaining space
    equally (centering) when both are auto, or absorb all remaining
    space when only one is auto.
    """
    n_auto_h = sum(1 for m in (ml, mr) if m == DEFERRED)

    if n_auto_h == 2:
        avail = parent_w - child_w
        each = max(avail / 2.0, 0.0)
        ml = each
        mr = each
    elif n_auto_h == 1:
        fixed = (mr if ml == DEFERRED else ml)
        avail = parent_w - child_w - fixed
        if ml == DEFERRED:
            ml = max(avail, 0.0)
        else:
            mr = max(avail, 0.0)

    # Auto top/bottom → 0 in normal flow (only meaningful in flex)
    mt = 0.0 if mt == DEFERRED else mt
    mb = 0.0 if mb == DEFERRED else mb

    return mt, mr, mb, ml


# ── Public API ─────────────────────────────────────────────────


def _resolve_pct_margin(m: float, parent_w: float, raw_styles: dict,
                          side: str) -> float:
    """Resolve a percentage margin that was DEFERRED; leave ``auto`` as-is."""
    if m != DEFERRED:
        return m
    raw = raw_styles.get(f"margin-{side}") or raw_styles.get("margin", "")
    if not raw:
        return m  # still DEFERRED — let _resolve_auto_margins handle it
    raw_s = raw.strip() if isinstance(raw, str) else ""
    if raw_s.endswith("%"):
        return (float(raw_s[:-1]) / 100.0) * parent_w
    return m  # not a percentage, keep DEFERRED for auto


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
    raw_s = node.raw_styles

    # Resolve percentage margins against parent width (per CSS 2.2 §8.3).
    # Leave DEFERRED (auto) values untouched for _resolve_auto_margins.
    ml = _resolve_pct_margin(ml, parent_w, raw_s, "left")
    mr = _resolve_pct_margin(mr, parent_w, raw_s, "right")
    mt = _resolve_pct_margin(mt, parent_w, raw_s, "top")
    mb = _resolve_pct_margin(mb, parent_w, raw_s, "bottom")

    h_bonus, v_bonus = _box_sizing_border_bonus(node)

    # ── Width ─────────────────────────────────────────────────
    if node.style.width is not None and node.style.width != DEFERRED:
        raw_w = node.style.width
    else:
        raw_w = parent_w - (0.0 if ml == DEFERRED else ml) \
                         - (0.0 if mr == DEFERRED else mr)

    total_w = raw_w + h_bonus

    # Resolve auto horizontal margins using the computed width
    mt, mr, mb, ml = _resolve_auto_margins(mt, mr, mb, ml, total_w, parent_w)

    # Write back resolved margins.  Auto margins keep the DEFERRED sentinel
    # so the C++ runtime can re-resolve them dynamically on resize.
    new_margin = [mt, mr, mb, ml]
    for i in range(4):
        if node.style.margin_auto[i]:
            new_margin[i] = DEFERRED
    node.style.margin = tuple(new_margin)

    # ── Height ────────────────────────────────────────────────
    if node.style.height is not None and node.style.height != DEFERRED:
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
