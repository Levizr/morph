from morph.ir.node import IRNode
from morph.style.units import DEFERRED


def apply_flex(parent: IRNode) -> None:
    s = parent.style
    is_row = s.flex_dir == "row"
    wrap = s.flex_wrap == "wrap"

    bw = s.border_width
    pt, pr, pb, pl = s.padding
    cx = parent.x + pl + bw
    cy = parent.y + pt + bw
    cw = parent.w - pl - pr - bw * 2
    ch = parent.h - pt - pb - bw * 2

    children = [c for c in parent.children if c.style.display != "none"]
    if not children:
        return

    gap = s.gap
    main_avail = cw if is_row else ch
    cross_avail = ch if is_row else cw

    items = []
    for child in children:
        cs = child.style
        mt, mr, mb, ml = cs.margin
        # Track auto margins on main axis before clamping
        ml_auto = ml == DEFERRED
        mr_auto = mr == DEFERRED
        mt_auto = mt == DEFERRED
        mb_auto = mb == DEFERRED
        # Clamp DEFERRED (auto) margins
        ml = 0.0 if ml == DEFERRED else ml
        mr = 0.0 if mr == DEFERRED else mr
        mt = 0.0 if mt == DEFERRED else mt
        mb = 0.0 if mb == DEFERRED else mb
        if is_row:
            main = child.w + ml + mr
            cross = child.h + mt + mb
            auto_main = ml_auto or mr_auto
        else:
            main = child.h + mt + mb
            cross = child.w + ml + mr
            auto_main = mt_auto or mb_auto
        items.append({
            "node": child,
            "main": max(main, 0.0),
            "cross": max(cross, 0.0),
            "mt": mt, "mr": mr, "mb": mb, "ml": ml,
            "mt_auto": mt_auto, "mb_auto": mb_auto,
            "ml_auto": ml_auto, "mr_auto": mr_auto,
            "auto_main": auto_main,
        })

    # Wrap into flex lines
    if wrap:
        lines = []
        cur = []
        cur_main = 0.0
        for item in items:
            need = item["main"] + (gap if cur else 0)
            if cur and cur_main + need > main_avail:
                lines.append(cur)
                cur = []
                cur_main = 0.0
            cur.append(item)
            cur_main += item["main"] + (gap if len(cur) > 1 else 0)
        if cur:
            lines.append(cur)
    else:
        lines = [items]

    # Flex-grow / flex-shrink per line
    for line in lines:
        total_main = sum(item["main"] for item in line)
        extra_gap = gap * (len(line) - 1) if len(line) > 1 else 0

        # grow
        remaining = main_avail - total_main - extra_gap
        if remaining > 0:
            grow_total = sum(item["node"].style.flex_grow for item in line)
            if grow_total > 0:
                per_unit = remaining / grow_total
                for item in line:
                    grow = item["node"].style.flex_grow
                    if grow > 0:
                        add = per_unit * grow
                        if is_row:
                            item["node"].w += add
                        else:
                            item["node"].h += add
                        item["main"] += add

        # shrink
        overflow = total_main + extra_gap - main_avail
        if overflow > 0:
            scaled_total = sum(item["main"] * item["node"].style.flex_shrink for item in line)
            if scaled_total > 0:
                for item in line:
                    sh = item["node"].style.flex_shrink
                    if sh > 0:
                        scaled = item["main"] * sh
                        reduction = overflow * scaled / scaled_total
                        reduced = max(0.0, item["main"] - reduction)
                        if is_row:
                            item["node"].w -= item["main"] - reduced
                        else:
                            item["node"].h -= item["main"] - reduced
                        item["main"] = reduced

    # Position
    main_start = cx if is_row else cy
    cross_start = cy if is_row else cx
    cursor_cross = cross_start

    for line in lines:
        line_cross = max(item["cross"] for item in line) if line else 0.0
        total_main = sum(item["main"] for item in line)
        extra_gap = gap * (len(line) - 1) if len(line) > 1 else 0
        used = total_main + extra_gap
        free = main_avail - used

        # Distribute free space to auto margins on main axis
        auto_main_items = [item for item in line if item["auto_main"]]
        if auto_main_items and free > 0:
            per_auto = free / len(auto_main_items)
            for item in auto_main_items:
                if is_row:
                    if item["ml_auto"]:
                        item["ml"] += per_auto
                    elif item["mr_auto"]:
                        item["mr"] += per_auto
                else:
                    if item["mt_auto"]:
                        item["mt"] += per_auto
                    elif item["mb_auto"]:
                        item["mb"] += per_auto
                item["main"] += per_auto
            # Recompute used/free after auto margin absorption
            total_main = sum(item["main"] for item in line)
            used = total_main + extra_gap
            free = main_avail - used

        # Compute main-axis offset
        justify = s.justify_content
        if justify == "center":
            offset = free * 0.5
            item_gap = gap
        elif justify == "flex-end":
            offset = free
            item_gap = gap
        elif justify == "space-between":
            offset = 0.0
            item_gap = gap + free / (len(line) - 1) if len(line) > 1 else 0.0
        elif justify == "space-around":
            offset = free / (len(line) * 2) if len(line) > 0 else 0.0
            item_gap = gap + free / len(line) if len(line) > 0 else 0.0
        else:
            offset = 0.0
            item_gap = gap

        cursor_main = main_start + offset

        for i, item in enumerate(line):
            child = item["node"]
            ml, mr, mt, mb = item["ml"], item["mr"], item["mt"], item["mb"]

            if is_row:
                child.x = cursor_main + ml
                cs = child.style
                ali = s.align_items
                if ali == "center":
                    child.y = cursor_cross + mt + (line_cross - (child.h + mt + mb)) * 0.5
                elif ali == "flex-end":
                    child.y = cursor_cross + line_cross - child.h - mb
                elif ali == "stretch":
                    target = line_cross - mt - mb
                    if target > child.h:
                        child.h = target
                    child.y = cursor_cross + mt
                else:
                    child.y = cursor_cross + mt
                cursor_main += child.w + ml + mr + item_gap
            else:
                child.y = cursor_main + mt
                cs = child.style
                ali = s.align_items
                if ali == "center":
                    child.x = cursor_cross + ml + (line_cross - (child.w + ml + mr)) * 0.5
                elif ali == "flex-end":
                    child.x = cursor_cross + line_cross - child.w - mr
                elif ali == "stretch":
                    target = line_cross - ml - mr
                    if target > child.w:
                        child.w = target
                    child.x = cursor_cross + ml
                else:
                    child.x = cursor_cross + ml
                cursor_main += child.h + mt + mb + item_gap

        cursor_cross += line_cross + gap

    # Auto-height expansion
    if s.height is None and children:
        bw = s.border_width
        pb = s.padding[2]
        max_bottom = max((c.y + c.h for c in children if c.style.display != "none"), default=parent.y + parent.h)
        desired = (max_bottom - parent.y) + pb + bw
        if desired > parent.h:
            parent.h = desired
