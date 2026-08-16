"""Production-grade layout engine implementing the CSS visual formatting model.

Architecture
------------
Two-pass layout:

  **Pass 1 — Measure** (bottom-up via ``_measure``)
      Compute intrinsic content heights and text widths.  Because children
      are measured before parents we know how tall a ``height: auto`` node
      will be before its position is resolved.

  **Pass 2 — Layout** (top-down via ``_layout``)
      Resolve the full box model (margin, border, padding, ``box-sizing``),
      assign final border-box positions, lay out children by ``display``
      type, apply min/max constraints, and handle overflow / positioning.


Supported display types
-----------------------
* ``block``   — vertical stacking with margin collapsing, fills available width.
* ``inline``  — line-box based horizontal flow with text wrapping.
* ``none``    — completely removed from layout.
* ``flex``    — delegated to ``morph.layout.flex`` (stub).

Box model features
-------------------
* W3C visual formatting model (border-box = content + padding + border).
* ``box-sizing: content-box | border-box``
* Margin collapsing between adjacent block siblings (CSS 2.2 §8.3.1).
* ``min-width`` / ``max-width`` / ``min-height`` / ``max-height`` clamping.
* ``overflow: visible | hidden | scroll | auto``.
* ``position: static | relative`` (absolute / fixed TBD).

Every function writes final positions back onto ``IRNode.x / y / w / h``
so downstream consumers (C++ codegen, dev-socket serialisation) see
border-box coordinates.
"""

import math
from morph.ir.node import IRWindow, IRNode
from morph.layout.box import resolve_border_box, resolve_content_box, apply_min_max
from morph.layout.inline import (
    layout_inline_lines,
    apply_inline_positions,
    estimate_text_width,
)
from morph.layout.flex import apply_flex
from morph.style.units import resolve, DEFERRED
from morph.style.transforms import compose_transform


def _resolve_transform_origin(raw: tuple, w: float, h: float) -> tuple[float, float]:
    """Resolve a raw transform-origin into fractions of the box (0..1).

    ``raw`` is ((x, is_pct_x), (y, is_pct_y)) from the builder.
    """
    (xv, xp), (yv, yp) = raw
    fx = (xv / 100.0) if xp else (xv / w if w > 1e-6 else 0.0)
    fy = (yv / 100.0) if yp else (yv / h if h > 1e-6 else 0.0)
    return (fx, fy)


class LayoutEngine:

    def compute(self, windows: list[IRWindow]) -> None:
        for win in windows:
            self._viewport_w = float(win.width)
            self._viewport_h = float(win.height)

            self._heights: dict[str, float] = {}  # node_id → content height
            for node in win.nodes:
                self._measure(node, self._viewport_w)
            for node in win.nodes:
                self._layout(node, 0.0, 0.0, self._viewport_w, self._viewport_h)

    @staticmethod
    def _resolve_raw_styles(node: IRNode, parent_w: float, parent_h: float,
                             viewport_w: float, viewport_h: float) -> None:
        """Resolve deferred width/height/gap CSS values (%, vh, vw) at layout time.
        
        Margin/padding percentages are resolved directly in resolve_border_box
        because they reference the parent's content width.
        """
        if not node.raw_styles:
            return

        s = node.style
        for css_key, css_val in node.raw_styles.items():
            if css_key in ("margin", "padding", "margin-top", "margin-right",
                           "margin-bottom", "margin-left", "padding-top",
                           "padding-right", "padding-bottom", "padding-left"):
                continue  # resolved in resolve_border_box
            v = css_val.strip()
            if v.endswith("vh"):
                val = (float(v[:-2]) / 100.0) * viewport_h
            elif v.endswith("vw"):
                val = (float(v[:-2]) / 100.0) * viewport_w
            elif v.endswith("%"):
                if css_key in ("height", "min-height", "max-height"):
                    val = (float(v[:-1]) / 100.0) * parent_h
                else:
                    val = (float(v[:-1]) / 100.0) * parent_w
            else:
                val = resolve(css_val, parent_w, viewport_h)

            if val is not None:
                if css_key == "width":
                    s.width = val
                elif css_key == "height":
                    s.height = val
                elif css_key == "min-width":
                    s.min_width = val
                elif css_key == "max-width":
                    s.max_width = val
                elif css_key == "min-height":
                    s.min_height = val
                elif css_key == "max-height":
                    s.max_height = val
                elif css_key == "gap":
                    s.gap = val
                elif css_key == "left":
                    s.left = val
                elif css_key == "right":
                    s.right = val
                elif css_key == "top":
                    s.top = val
                elif css_key == "bottom":
                    s.bottom = val

    @staticmethod
    def _resolve_transforms(node: IRNode) -> None:
        if not node.style.transform_ops and not node.style.transform_origin and not (
            node.hover_style and node.hover_style.transform_ops
        ) and not (node.active_style and node.active_style.transform_ops) \
                and not any(s.transform_ops for _, s in node.ancestor_hover_rules) \
                and not any(s.transform_ops for _, s in node.ancestor_active_rules):
            return

        def resolve_style(s) -> None:
            if s is not None:
                if s.transform_ops:
                    s.transform_matrix = compose_transform(s.transform_ops,
                                                           node.w, node.h)
                if s.transform_origin:
                    s.transform_origin_resolved = _resolve_transform_origin(
                        s.transform_origin, node.w, node.h)

        resolve_style(node.style)
        resolve_style(node.hover_style)
        resolve_style(node.active_style)
        for _, s in node.ancestor_hover_rules:
            resolve_style(s)
        for _, s in node.ancestor_active_rules:
            resolve_style(s)

    # ═══════════════════════════════════════════════════════════
    #  Pass 1 — Measure (bottom-up)
    # ═══════════════════════════════════════════════════════════

    def _measure(self, node: IRNode, avail_w: float) -> float:
        """Compute intrinsic content height for *node* and its descendants.

        Returns the **content height** (before padding / border) that the
        node would have if its width were *avail_w*.  The result is also
        stored in ``self._heights[node.node_id]``.

        For text leaves this also estimates a pixel width on ``node.w``.
        """
        if node.style.display == "none":
            self._heights[node.node_id] = 0.0
            return 0.0

        # Estimate the content-box width so children know how wide they are.
        bw = node.style.border_width
        pt, pr, pb, pl = node.style.padding
        mt, mr, mb, ml = node.style.margin

        # Deferred margins (auto, %) — treat as 0 during measure pass
        ml = 0.0 if ml == DEFERRED else ml
        mr = 0.0 if mr == DEFERRED else mr

        if node.style.width is not None and node.style.width != DEFERRED:
            raw = node.style.width
            if node.style.box_sizing == "border-box":
                cw = raw - pl - pr - bw * 2
            else:
                cw = raw
        else:
            cw = avail_w - ml - mr - pl - pr - bw * 2
        cw = max(cw, 0.0)

        # ── Text leaf ────────────────────────────────────────
        if node.node_type == "__text__":
            tw = estimate_text_width(node)
            node.w = tw
            node.h = node.style.font_size * 1.4
            self._heights[node.node_id] = node.h
            return node.h

        # ── Recurse into children ────────────────────────────
        content_h = 0.0
        prev_mb = 0.0
        i = 0

        # Flex container: measure children individually, estimate total height
        # from flex line wrapping (like inline, but along main axis).
        if node.style.display == "flex":
            is_row = node.style.flex_dir == "row"
            wrap = node.style.flex_wrap == "wrap"
            flex_gap = node.style.gap
            main_avail = cw if is_row else cw  # use cw for both (approximate)
            total_cross = 0.0
            line_main = 0.0
            line_cross = 0.0
            first_in_line = True

            for child in node.children:
                if child.style.display == "none":
                    self._heights[child.node_id] = 0.0
                    continue
                self._measure(child, cw)
                mt, mr, mb, ml = child.style.margin
                # Clamp DEFERRED (auto) margins to 0 during measure —
                # they will be resolved in the layout pass.
                ml = 0.0 if ml == DEFERRED else ml
                mr = 0.0 if mr == DEFERRED else mr
                mt = 0.0 if mt == DEFERRED else mt
                mb = 0.0 if mb == DEFERRED else mb
                cmain = (child.w + ml + mr) if is_row else (child.h + mt + mb)
                ccross = (child.h + mt + mb) if is_row else (child.w + ml + mr)

                if not first_in_line and wrap and line_main + cmain > main_avail:
                    total_cross += max(line_cross, 0.0) + flex_gap
                    line_main = 0.0
                    line_cross = 0.0
                    first_in_line = True
                line_main += cmain + (flex_gap if not first_in_line else 0)
                if ccross > line_cross:
                    line_cross = ccross
                first_in_line = False

            if line_cross > 0:
                total_cross += line_cross
            content_h = total_cross
        else:
            while i < len(node.children):
                child = node.children[i]
                dsp = "inline" if child.node_type == "__text__" else child.style.display

                if dsp == "none":
                    self._heights[child.node_id] = 0.0
                    i += 1
                    continue

                if dsp in ("inline", "inline-block"):
                    group: list[IRNode] = []
                    while i < len(node.children):
                        ic = node.children[i]
                        icd = "inline" if ic.node_type == "__text__" else ic.style.display
                        if icd not in ("inline", "inline-block"):
                            break
                        self._measure(ic, cw)
                        group.append(ic)
                        i += 1
                    # Line-breaking simulation matching inline.py layout_inline_lines
                    if group:
                        line_h = max(c.h for c in group)
                        n_lines = 1
                        line_x = 0.0
                        for c in group:
                            if line_x > 0.0 and line_x + c.w > cw:
                                n_lines += 1
                                line_x = 0.0
                            line_x += c.w
                        content_h += line_h * n_lines + (n_lines - 1) * node.style.gap
                    prev_mb = 0.0  # inline resets collapsing context

                else:  # block
                    child_h = self._measure(child, cw)
                    # Convert content height to border-box height
                    child_bh = child_h
                    if child.style.height is not None:
                        if child.style.box_sizing == "border-box":
                            child_bh = child.style.height
                        else:
                            child_bh = child.style.height + child.style.border_width * 2 \
                                        + child.style.padding[0] + child.style.padding[2]
                    else:
                        child_bh = child_h + child.style.border_width * 2 \
                                    + child.style.padding[0] + child.style.padding[2]

                    child_bh = max(child_bh, 0.0)
                    cmt = child.style.margin[0]
                    cmb = child.style.margin[2]
                    # Clamp DEFERRED (auto) vertical margins to 0 during measure
                    cmt = 0.0 if cmt == DEFERRED else cmt
                    cmb = 0.0 if cmb == DEFERRED else cmb
                    gap = max(prev_mb, cmt) if prev_mb >= 0 else cmt
                    content_h += gap + child_bh
                    prev_mb = cmb
                    i += 1

        self._heights[node.node_id] = content_h
        return content_h

    # ═══════════════════════════════════════════════════════════
    #  Pass 2 — Layout (top-down)
    # ═══════════════════════════════════════════════════════════

    def _layout(self, node: IRNode, px: float, py: float,
                parent_w: float, parent_h: float) -> None:
        """Assign final border-box position and size.

        Parameters
        ----------
        node
            Node to lay out.
        px, py
            Origin of the parent's **content area**.
        parent_w, parent_h
            Size of the parent's content area.
        """
        if node.style.display == "none":
            node.x = node.y = node.w = node.h = 0.0
            return

        # ── 1. Text leaves (skip border-box, keep measured size) ─
        if node.node_type == "__text__":
            node.x = px
            node.y = py
            # Use parent content width so the build-time hardcoded w
            # matches the wrapping width the runtime will compute
            if node.w == 0.0 or node.w > parent_w:
                node.w = parent_w
            if node.h == 0.0:
                node.h = node.style.font_size * 1.4
            return

        # ── 2. Resolve deferred raw styles (%, vh, vw) ───────
        self._resolve_raw_styles(node, parent_w, parent_h,
                                 self._viewport_w, self._viewport_h)

        # ── 3. Resolve border-box ─────────────────────────────
        ch = self._heights.get(node.node_id)
        node.x, node.y, node.w, node.h = resolve_border_box(
            node, px, py, parent_w, parent_h, content_h=ch,
        )

        # ── 4. Content area for children ──────────────────────
        cx, cy, cw, _ch = resolve_content_box(node)

        # ── 5. Lay out children ───────────────────────────────
        if node.style.display == "flex":
            apply_flex(node)
            # Recurse into children for their own descendants
            for child in node.children:
                if child.style.display != "none":
                    self._layout(child, child.x, child.y, child.w, child.h)
            # Skip normal child loop
            # Auto-height expansion
            if node.style.height is None and node.children:
                bw = node.style.border_width
                pb = node.style.padding[2]
                max_bottom = max(
                    (c.y + c.h for c in node.children if c.style.display != "none"),
                    default=node.y + node.h,
                )
                desired = (max_bottom - node.y) + pb + bw
                if desired > node.h:
                    node.h = desired
            apply_min_max(node)
            self._resolve_transforms(node)
            return

        i = 0
        prev_block: IRNode | None = None  # last block sibling (for margin collapsing)

        while i < len(node.children):
            child = node.children[i]
            dsp = "inline" if child.node_type == "__text__" else child.style.display

            if dsp == "none":
                self._layout(child, 0, 0, 0, 0)
                i += 1
                continue

            if dsp in ("inline", "inline-block"):
                # Collect consecutive inline children into line boxes
                group: list[IRNode] = []
                while i < len(node.children):
                    ic = node.children[i]
                    icd = "inline" if ic.node_type == "__text__" else ic.style.display
                    if icd not in ("inline", "inline-block"):
                        break
                    group.append(ic)
                    i += 1
                if group:
                    cy = self._layout_inline_group(group, cx, cy, cw, node.style.gap,
                                                    node.style.text_align)
                # Inline children do not set prev_block (no margin collapsing)
                continue

            # ── Block child with margin collapsing ───────────
            mt = child.style.margin[0]
            cmb = child.style.margin[2]

            if prev_block is not None:
                prev_mb = prev_block.style.margin[2]
                collapsed = max(prev_mb, mt)
                # Place child so its border-box top = prev_block's bottom + collapsed
                # resolve_border_box does: child.y = child_py + mt
                # We want: child.y = prev_block.y + prev_block.h + collapsed
                # So: child_py = prev_block.y + prev_block.h + collapsed - mt
                child_py = prev_block.y + prev_block.h + collapsed - mt
            else:
                child_py = cy  # first block / after inline

            self._layout(child, cx, child_py, cw, _ch)
            prev_block = child
            i += 1

        # ── 6. Auto-height expansion ─────────────────────────
        if node.style.height is None and node.children:
            bw = node.style.border_width
            pb = node.style.padding[2]
            max_bottom = max(
                (c.y + c.h for c in node.children if c.style.display != "none"),
                default=node.y + node.h,
            )
            desired = (max_bottom - node.y) + pb + bw
            if desired > node.h:
                node.h = desired

        # ── 7. Min / max clamping (final) ───────────────────
        apply_min_max(node)

        # ── 8. Resolve transform matrices (needs final box) ─
        self._resolve_transforms(node)

    # ── Inline group helper ─────────────────────────────────

    def _layout_inline_group(self, children: list[IRNode], cx: float, cy: float,
                              available_width: float, gap: float,
                              text_align: str) -> float:
        """Place inline children into line boxes and return the Y cursor
        (just past the last line, including trailing gap).

        Each child's border-box size is computed from its style before
        line-breaking.  After positioning, non-text inline children get
        their own descendants recursively laid out.
        """
        # Pre-compute sizes for non-text inline nodes
        for child in children:
            if child.node_type == "__text__":
                if child.w == 0.0:
                    child.w = estimate_text_width(child)
                if child.h == 0.0:
                    child.h = child.style.font_size * 1.4
            else:
                bw2 = child.style.border_width
                pt2, pr2, pb2, pl2 = child.style.padding
                if child.style.width is not None:
                    if child.style.box_sizing == "border-box":
                        child.w = child.style.width
                    else:
                        child.w = child.style.width + pl2 + pr2 + bw2 * 2
                elif child.children:
                    # Shrink-to-fit: sum children's width + padding + border
                    content_w = 0.0
                    for sub in child.children:
                        if sub.node_type == "__text__":
                            content_w += estimate_text_width(sub)
                        elif sub.style.width is not None:
                            content_w += sub.style.width
                        else:
                            content_w += 80.0  # fallback default
                    child.w = content_w + pl2 + pr2 + bw2 * 2
                else:
                    child.w = pl2 + pr2 + bw2 * 2  # just padding + border

                if child.style.height is not None:
                    if child.style.box_sizing == "border-box":
                        child.h = child.style.height
                    else:
                        child.h = child.style.height + pt2 + pb2 + bw2 * 2
                else:
                    child.h = child.style.font_size * 1.4 + pt2 + pb2 + bw2 * 2

        lines = layout_inline_lines(children, cx, cy, available_width,
                                     gap, text_align)
        apply_inline_positions(lines)

        # Recursively lay out each non-text inline child's descendants
        for child in children:
            if child.node_type != "__text__" and child.children:
                from morph.layout.box import resolve_content_box
                # Compute content area from the now-final position
                ccx, ccy, ccw, cch = resolve_content_box(child)
                for sub in child.children:
                    self._layout(sub, ccx, ccy, ccw, cch)
                # Re-apply min/max now that children are laid out
                apply_min_max(child)

        # Resolve transform matrices for inline-block children — their
        # border-box is final only after line placement above.
        for child in children:
            if child.node_type != "__text__":
                self._resolve_transforms(child)

        if not lines:
            return cy
        last_line = lines[-1]
        last_item = last_line.items[-1] if last_line.items else None
        if last_item:
            return last_item.y + last_item.h + gap
        return cy
