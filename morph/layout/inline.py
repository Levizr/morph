"""Inline layout engine.

Handles line-box construction, text measurement, and inline element
positioning following the CSS 2.2 visual formatting model (§9.4.2).
"""

from __future__ import annotations
from dataclasses import dataclass, field
from morph.ir.node import IRNode


# Rough per-character width as a fraction of font-size.
# Measured against a mix of ASCII lowercase / uppercase / space on
# a typical system font; good enough for Python-side layout estimation.
_CHAR_WIDTH_RATIO = 0.58
_SPACE_WIDTH_RATIO = 0.30


def estimate_text_width(node: IRNode) -> float:
    """Estimate the pixel width of a text node's content."""
    text = node.text_content
    fs = node.style.font_size
    w = 0.0
    for ch in text:
        if ch == " ":
            w += fs * _SPACE_WIDTH_RATIO
        elif ch in "\t\r\n":
            pass
        else:
            w += fs * _CHAR_WIDTH_RATIO
    return w


# ── Line box model ────────────────────────────────────────────


@dataclass
class PlacedInline:
    """A single inline-level child placed within a line box."""
    node: IRNode
    x: float
    y: float
    w: float
    h: float


@dataclass
class LineBox:
    """A single line box containing one or more inline-level items."""
    items: list[PlacedInline] = field(default_factory=list)
    width: float = 0.0
    height: float = 0.0
    baseline: float = 0.0


def layout_inline_lines(
    children: list[IRNode],
    cx: float,
    cy: float,
    available_width: float,
    gap: float,
    text_align: str = "left",
) -> list[LineBox]:
    """Break a sequence of inline-level children into one or more line boxes.

    Parameters
    ----------
    children
        Consecutive inline-level siblings from the parent's child list.
    cx, cy
        Starting origin (top-left) of the content area.
    available_width
        Maximum line width before wrapping.
    gap
        Vertical gap between lines (applied as line spacing).
    text_align
        Horizontal alignment within each line (``left``, ``center``, ``right``).

    Returns
    -------
    list[LineBox]
        One line box per wrapped line.
    """
    lines: list[LineBox] = []
    current_line = LineBox()
    line_x = cx

    for child in children:
        display = "inline" if child.node_type == "__text__" else child.style.display

        if display == "none":
            continue

        text_w = estimate_text_width(child) if child.node_type == "__text__" else child.w
        child_h = child.h if child.h > 0 else (child.style.font_size * 1.4 if child.node_type == "__text__" else 0.0)

        # Does this item fit on the current line?
        if current_line.items and (line_x - cx + text_w) > available_width:
            # Finalise current line
            _finalise_line(current_line, cx, text_align, available_width)
            lines.append(current_line)
            current_line = LineBox()
            line_x = cx

        item = PlacedInline(
            node=child,
            x=line_x,
            y=cy,
            w=text_w,
            h=child_h,
        )
        current_line.items.append(item)
        current_line.width = (line_x - cx) + text_w
        if child_h > current_line.height:
            current_line.height = child_h
        line_x += text_w

    if current_line.items:
        _finalise_line(current_line, cx, text_align, available_width)
        lines.append(current_line)

    # Adjust Y positions within each line (vertical-align: top)
    y_cursor = cy
    for line in lines:
        for item in line.items:
            item.y = y_cursor
        y_cursor += line.height + gap

    return lines


def _finalise_line(line: LineBox, cx: float, text_align: str,
                   available_width: float) -> None:
    """Apply horizontal alignment to a completed line."""
    if text_align == "left":
        return  # already left-aligned
    offset = 0.0
    if text_align == "center":
        offset = (available_width - line.width) / 2
    elif text_align == "right":
        offset = available_width - line.width
    if offset > 0:
        for item in line.items:
            item.x += offset


def apply_inline_positions(lines: list[LineBox]) -> float:
    """Write the computed positions from *lines* back onto the IR nodes.

    Returns the total vertical advance consumed by the lines
    (last line bottom minus starting ``cy``).
    """
    if not lines:
        return 0.0
    first_cy = lines[0].items[0].y if lines[0].items else 0.0
    last_line = lines[-1]
    last_item = last_line.items[-1] if last_line.items else None
    if not last_item:
        return 0.0
    for line in lines:
        for item in line.items:
            item.node.x = item.x
            item.node.y = item.y
            item.node.w = item.w
            item.node.h = item.h
    return (last_item.y + last_item.h) - first_cy
