from __future__ import annotations

import math
from morph.ir.node import IRNode, IRWindow
from morph.ir.style import IRStyle
from morph.ir.event import IREvent
from morph.style.tailwind import TailwindResolver
from morph.style.selector import matches_selector, calculate_specificity, parse_selector, selector_to_string
from morph.utils.color import parse_color
from morph.style.units import to_px, needs_layout, DEFERRED

# User-agent default styles for HTML tags (lowest priority — overridden by everything)
# Specificity is effectively (0,0,0) — any user CSS rule overrides these.
_UA_DEFAULTS: dict[str, dict[str, str]] = {
    # ── Document ────────────────────────────────────────────
    "html": {"display": "block"},
    "body": {"display": "block", "padding": "8px"},

    # ── Headings ────────────────────────────────────────────
    "h1": {"display": "block", "font-size": "32px",    "font-weight": "bold", "margin": "21.44px 0"},
    "h2": {"display": "block", "font-size": "24px",    "font-weight": "bold", "margin": "19.92px 0"},
    "h3": {"display": "block", "font-size": "18.72px", "font-weight": "bold", "margin": "18.72px 0"},
    "h4": {"display": "block", "font-size": "16px",    "font-weight": "bold", "margin": "21.28px 0"},
    "h5": {"display": "block", "font-size": "13.28px", "font-weight": "bold", "margin": "22.18px 0"},
    "h6": {"display": "block", "font-size": "10.72px", "font-weight": "bold", "margin": "24.97px 0"},

    # ── Grouping ────────────────────────────────────────────
    "div":       {"display": "block"},
    "p":         {"display": "block", "margin": "16px 0"},
    "pre":       {"display": "block", "margin": "16px 0"},
    "blockquote":{"display": "block", "margin": "16px 40px"},
    "hr":        {"display": "block"},
    "figure":    {"display": "block", "margin": "16px 40px"},
    "figcaption":{"display": "block"},
    "main":      {"display": "block"},
    "header":    {"display": "block"},
    "footer":    {"display": "block"},
    "nav":       {"display": "block"},
    "section":   {"display": "block"},
    "article":   {"display": "block"},
    "aside":     {"display": "block"},

    # ── Lists ───────────────────────────────────────────────
    "ul":  {"display": "block", "margin": "16px 0"},
    "ol":  {"display": "block", "margin": "16px 0"},
    "li":  {"display": "block"},
    "dl":  {"display": "block", "margin": "16px 0"},
    "dt":  {"display": "block"},
    "dd":  {"display": "block", "margin-left": "40px"},

    # ── Text-level ──────────────────────────────────────────
    "span":    {"display": "inline"},
    "a":       {"display": "inline", "color": "#0000ee", "cursor": "pointer"},
    "strong":  {"font-weight": "bold"},
    "b":       {"font-weight": "bold"},
    "small":   {"font-size": "13.28px"},
    "mark":    {"background-color": "#ffff00", "color": "#000000"},
    "sub":     {"font-size": "13.28px"},
    "sup":     {"font-size": "13.28px"},
    "code":    {"display": "inline"},
    "kbd":     {"display": "inline"},
    "samp":    {"display": "inline"},
    "em":      {"display": "inline"},
    "i":       {"display": "inline"},
    "ins":     {"display": "inline"},
    "u":       {"display": "inline"},
    "del":     {"display": "inline"},
    "s":       {"display": "inline"},
    "q":       {"display": "inline"},

    # ── Embedded ────────────────────────────────────────────
    "img": {"display": "inline-block"},

    # ── Forms ───────────────────────────────────────────────
    "button":   {"display": "inline-block"},
    "input":    {"display": "inline-block"},
    "select":   {"display": "inline-block"},
    "textarea": {"display": "inline-block"},
    "label":    {"display": "inline"},
    "fieldset": {"display": "block", "border-width": "2px", "border-style": "groove", "margin": "0 2px", "padding": "5px 12px 10px"},
    "legend":   {"display": "block", "padding": "0 2px"},
    "form":     {"display": "block"},

    # ── Tables ──────────────────────────────────────────────
    "table":    {"display": "block"},
    "caption":  {"display": "block"},
    "thead":    {"display": "block"},
    "tbody":    {"display": "block"},
    "tfoot":    {"display": "block"},
    "tr":       {"display": "block"},
    "td":       {"display": "block"},
    "th":       {"display": "block", "font-weight": "bold", "text-align": "center"},

    # ── Interactive ─────────────────────────────────────────
    "details":  {"display": "block"},
    "summary":  {"display": "block"},
    "dialog":   {"display": "block"},
}

# CSS property name → IRStyle field name
_CSS_TO_IR: dict[str, str] = {
    "background-color":         "bg_color",
    "color":                    "color",
    "width":                    "width",
    "min-width":                "min_width",
    "max-width":                "max_width",
    "height":                   "height",
    "min-height":               "min_height",
    "max-height":               "max_height",
    "margin":                   "margin",
    "margin-top":               "margin_top_side",
    "margin-bottom":            "margin_bottom_side",
    "margin-left":              "margin_left_side",
    "margin-right":             "margin_right_side",
    "padding":                  "padding",
    "padding-top":              "padding_top_side",
    "padding-bottom":           "padding_bottom_side",
    "padding-left":             "padding_left_side",
    "padding-right":            "padding_right_side",
    "border-radius":            "border_radius",
    "font-size":                "font_size",
    "font-weight":              "font_weight",
    "text-align":               "text_align",
    "max-width":                "max_width",
    "display":                  "display",
    "flex-direction":           "flex_dir",
    "flex-grow":                "flex_grow",
    "flex-shrink":              "flex_shrink",
    "flex-basis":               "flex_basis",
    "gap":                      "gap",
    "overflow":                 "overflow",
    "position":                 "position",
    "left":                     "left",
    "right":                    "right",
    "top":                      "top",
    "bottom":                   "bottom",
    "justify-content":          "justify_content",
    "align-items":              "align_items",
    "flex-wrap":                "flex_wrap",
    "cursor":                   "cursor",
    "scrollbar-width":           "scrollbar_width",
    "scrollbar-track-color":     "scrollbar_track_color",
    "scrollbar-thumb-color":     "scrollbar_thumb_color",
    "scrollbar-border-radius":   "scrollbar_border_radius",
    "border-width":              "border_width",
    "border-color":              "border_color",
    "border-style":              "border_style",
    "box-sizing":                "box_sizing",
}


def _parse_side_value(key: str, val: str) -> tuple[float, float, float, float]:
    """Parse a CSS shorthand like '10px 20px' or '0 auto' into 4 sides.
    
    ``auto`` values are stored as ``math.inf`` (DEFERRED) so the layout
    engine can compute the actual value with available space.
    """
    parts = val.split()
    nums = [to_px(p) for p in parts]
    if len(nums) == 1:
        return (nums[0], nums[0], nums[0], nums[0])
    if len(nums) == 2:
        return (nums[0], nums[1], nums[0], nums[1])
    if len(nums) == 3:
        return (nums[0], nums[1], nums[2], nums[1])
    if len(nums) == 4:
        return (nums[0], nums[1], nums[2], nums[3])
    return (0.0, 0.0, 0.0, 0.0)


def _parse_margin_auto(val: str) -> tuple[bool, bool, bool, bool]:
    """Return a 4-tuple indicating which sides are ``auto``."""
    parts = val.split()
    n = len(parts)
    auto_flags = [p == "auto" for p in parts]
    if n == 1:
        return (auto_flags[0], auto_flags[0], auto_flags[0], auto_flags[0])
    if n == 2:
        return (auto_flags[0], auto_flags[1], auto_flags[0], auto_flags[1])
    if n == 3:
        return (auto_flags[0], auto_flags[1], auto_flags[2], auto_flags[1])
    if n == 4:
        return (auto_flags[0], auto_flags[1], auto_flags[2], auto_flags[3])
    return (False, False, False, False)


class IRBuilder:
    """Converts a walked AST + CSS rules + Tailwind into an IR tree."""

    def __init__(self, config=None):
        self.config = config
        self._counter = 0

    def build(
        self,
        walked: dict,
        css_rules: dict,
        tw_resolver: TailwindResolver,
    ) -> list[IRWindow]:
        ir_windows = []

        win_cfg = walked.get("windowConfig")
        for comp in walked.get("components", []):
            if not comp.get("exported", False):
                continue

            jsx = comp.get("jsx", {})
            tag = jsx.get("tag")

            if win_cfg:
                window_id = self._next_id()
                window_nodes = []
                children = jsx.get("children", []) if tag == "__fragment__" else [jsx]
                for child in children:
                    node = self._build_node(child, css_rules, tw_resolver)
                    if node:
                        window_nodes.append(node)

                ir_windows.append(IRWindow(
                    window_id=window_id,
                    nodes=window_nodes,
                    startup_logs=comp.get("body_logs", []),
                    title=win_cfg.get("title", str(getattr(self.config, "name", "Untitled"))),
                    width=win_cfg.get("width", 800),
                    height=win_cfg.get("height", 600),
                ))
            elif tag == "morph-window":
                props = jsx.get("props", {})
                tw_styles = _resolve_tw(props, tw_resolver)

                window_nodes = []
                for child in jsx.get("children", []):
                    node = self._build_node(child, css_rules, tw_resolver)
                    if node:
                        window_nodes.append(node)

                ir_windows.append(IRWindow(
                    window_id=self._next_id(),
                    nodes=window_nodes,
                    startup_logs=comp.get("body_logs", []),
                    title=props.get("title", str(getattr(self.config, "name", "Untitled"))),
                    width=_int_prop(props, "width", tw_styles, 800),
                    height=_int_prop(props, "height", tw_styles, 600),
                ))

        return ir_windows

    def _build_node(
        self,
        jsx_node: dict,
        css_rules: dict,
        tw_resolver: TailwindResolver,
        ancestry: list[tuple[str, list[str]]] | None = None,
    ) -> IRNode | None:
        tag = jsx_node.get("tag")
        if not tag:
            return None

        node_id = self._next_id()
        props = jsx_node.get("props", {})

        # ── Text node ────────────────────────────────────────
        if tag == "__text__":
            return IRNode(
                node_id=node_id,
                node_type="__text__",
                text_content=jsx_node.get("text", ""),
                style=IRStyle(display="inline"),
                children=[],
                events=[],
            )

        # ── Resolve CSS cascade ──────────────────────────────
        inline_raw = props.get("style", {})
        if isinstance(inline_raw, str):
            inline_raw = {}
        class_names = _get_classes(props)
        tw_styles = _resolve_tw(props, tw_resolver)
        node_id_attr = props.get("id", "")

        # Cascade order (lowest to highest priority):
        #   1. UA defaults
        #   2. External CSS rules (sorted by specificity)
        #   3. Tailwind classes
        #   4. HTML attributes (width, height)
        #   5. Inline style
        merged = {}
        merged.update(_UA_DEFAULTS.get(tag, {}))

        # Collect matching CSS rules with specificity, then apply in order
        matched = []               # non-:hover rules
        hover_matched = []         # :hover pseudo-class on THIS element
        ancestor_hover_matched = []  # :hover on an ANCESTOR compound
        for rule_key, rule_props in css_rules.items():
            has_hover = ":hover" in rule_key
            if not has_hover:
                if not matches_selector(rule_key, tag, class_names, node_id_attr, ancestry):
                    continue
                spec = calculate_specificity(rule_key)
                matched.append((spec, rule_props))
            else:
                # Strip :hover and check if the node matches the structural part
                match_key = rule_key.replace(":hover", "").strip()
                if not matches_selector(match_key, tag, class_names, node_id_attr, ancestry):
                    continue
                spec = calculate_specificity(rule_key)
                # Parse selector to determine if :hover is on this element or an ancestor
                selectors = parse_selector(rule_key)
                is_self_hover = False
                is_ancestor_hover = False
                ancestor_tag = None
                for sel in selectors:
                    for i, comp in enumerate(sel.compounds):
                        if comp.pseudo == "hover":
                            if i == len(sel.compounds) - 1:
                                is_self_hover = True
                            else:
                                ancestor_tag = comp.tag
                                is_ancestor_hover = True
                            break
                if is_ancestor_hover and ancestor_tag:
                    ancestor_hover_matched.append((spec, rule_props, ancestor_tag))
                elif is_self_hover:
                    hover_matched.append((spec, rule_props))
        matched.sort(key=lambda x: x[0])   # lowest specificity first
        hover_matched.sort(key=lambda x: x[0])
        ancestor_hover_matched.sort(key=lambda x: x[0])
        for _, rule_props in matched:
            merged.update(rule_props)

        merged.update(tw_styles)
        # HTML attributes like width="400" height="300" → CSS properties
        for attr in ('width', 'height'):
            val = props.get(attr)
            if val is not None:
                try:
                    merged[attr] = str(int(val)) + 'px'
                except (ValueError, TypeError):
                    pass
        merged.update(inline_raw)  # inline style overrides everything

        # ── Convert merged CSS → IRStyle fields ──────────────
        ir_kw, raw_styles = self._css_to_ir_kw(merged)

        # Build hover style from matching :hover CSS rules
        hover_style = None
        if hover_matched:
            hover_merged = {}
            for _, rule_props in hover_matched:
                hover_merged.update(rule_props)
            hover_ir_kw, _ = self._css_to_ir_kw(hover_merged, collect_raw=False)
            try:
                hover_style = IRStyle(**hover_ir_kw)
            except TypeError:
                pass

        # Build ancestor hover rules (pairs of ancestor_tag → resolved IRStyle)
        ancestor_hover_rules = []
        if ancestor_hover_matched:
            by_tag: dict[str, dict] = {}
            for _, rule_props, ancestor_tag in ancestor_hover_matched:
                existing = by_tag.get(ancestor_tag)
                if existing is None:
                    by_tag[ancestor_tag] = dict(rule_props)
                else:
                    existing.update(rule_props)
            for ancestor_tag, props in by_tag.items():
                rule_ir_kw, _ = self._css_to_ir_kw(props, collect_raw=False)
                try:
                    rule_style = IRStyle(**rule_ir_kw)
                    ancestor_hover_rules.append((ancestor_tag, rule_style))
                except TypeError:
                    pass

        try:
            node_style = IRStyle(**ir_kw)
        except TypeError:
            node_style = IRStyle()

        # ── Children ─────────────────────────────────────────
        child_ancestry = (ancestry or []) + [(tag, class_names)]
        children_nodes = []
        for child in jsx_node.get("children", []):
            child_node = self._build_node(child, css_rules, tw_resolver, child_ancestry)
            if child_node:
                children_nodes.append(child_node)

        # ── Tag attributes (src, alt, etc.) ──────────────────
        attrs = {}
        for attr_key in ("src", "alt", "href", "target"):
            val = props.get(attr_key)
            if val is not None and isinstance(val, str):
                attrs[attr_key] = val

        # ── Events ───────────────────────────────────────────
        events = []
        for attr_key in ("morph-open", "morph-close", "morph-navigate"):
            target = props.get(attr_key)
            if target:
                action = attr_key.split("-")[1]
                events.append(IREvent(trigger="click", action=action, target=target))

        # onClick with console.log
        onclick = props.get("onClick")
        if isinstance(onclick, dict) and "__fn__" in onclick:
            body = onclick["__fn__"]
            import re
            m = re.search(r'console\.log\(([^)]+)\)', body)
            if m:
                msg = m.group(1).strip().strip('"\'')
                events.append(IREvent(trigger="click", action="log", target=msg))

        # Extract transition config from merged CSS
        trans_dur = 0.0
        trans_easing = "ease-in-out"
        # Support both `transition` shorthand and individual properties
        if "transition" in merged:
            trans_parts = merged["transition"].split()
            # Parse: transition: <property> <duration> <timing-function> <delay>
            for tp in trans_parts:
                if tp.endswith("ms") or tp.endswith("s"):
                    trans_dur = _parse_transition_duration(tp)
                elif tp in ("ease", "ease-in", "ease-out", "ease-in-out", "linear"):
                    trans_easing = tp
            # If shorthand has no duration, default to 0.3s as sanity
            if trans_dur == 0.0:
                trans_dur = 0.3
        if "transition-duration" in merged:
            trans_dur = _parse_transition_duration(merged["transition-duration"])
        if "transition-timing-function" in merged:
            trans_easing = merged["transition-timing-function"]
        # Normalize easing
        if trans_easing == "ease":
            trans_easing = "ease-in-out"

        return IRNode(
            node_id=node_id,
            node_type=tag,
            style=node_style,
            hover_style=hover_style,
            children=children_nodes,
            events=events,
            attrs=attrs,
            raw_styles=raw_styles,
            transition_duration=trans_dur,
            transition_easing=trans_easing,
            ancestor_hover_rules=ancestor_hover_rules,
        )

    @staticmethod
    def _parse_flex_shorthand(ir_kw: dict, css_val: str) -> None:
        """Parse the CSS `flex` shorthand into flex_grow/shrink/basis."""
        parts = css_val.strip().split()
        kw = css_val.strip()
        if kw == "none":
            ir_kw["flex_grow"] = 0.0
            ir_kw["flex_shrink"] = 0.0
            ir_kw["flex_basis"] = "auto"
        elif kw == "auto":
            ir_kw["flex_grow"] = 1.0
            ir_kw["flex_shrink"] = 1.0
            ir_kw["flex_basis"] = "auto"
        elif kw == "initial":
            ir_kw["flex_grow"] = 0.0
            ir_kw["flex_shrink"] = 1.0
            ir_kw["flex_basis"] = "auto"
        elif len(parts) == 1:
            try:
                v = float(parts[0])
                ir_kw["flex_grow"] = v
                ir_kw["flex_shrink"] = 1.0
                ir_kw["flex_basis"] = "0%"
            except ValueError:
                pass
        elif len(parts) == 2:
            try:
                g = float(parts[0])
                s = float(parts[1])
                ir_kw["flex_grow"] = g
                ir_kw["flex_shrink"] = s
                ir_kw["flex_basis"] = "0%"
            except ValueError:
                pass
        elif len(parts) == 3:
            try:
                g = float(parts[0])
                s = float(parts[1])
                ir_kw["flex_grow"] = g
                ir_kw["flex_shrink"] = s
                ir_kw["flex_basis"] = parts[2]
            except ValueError:
                pass

    def _next_id(self) -> str:
        self._counter += 1
        return f"node_{self._counter:04d}"

    def _css_to_ir_kw(self, css_dict: dict, collect_raw: bool = True) -> tuple[dict, dict]:
        """Convert CSS property dict → (IRStyle kwargs, raw_styles)."""
        ir_kw: dict[str, any] = {}
        raw_styles: dict[str, str] = {}
        for css_key, css_val in css_dict.items():
            if css_key == "border":
                parts = css_val.split()
                for p in parts:
                    if p in ("solid", "dashed", "dotted", "none"):
                        ir_kw["border_style"] = p
                    elif p.startswith("#") or p.startswith("rgb") or p in ("transparent",):
                        ir_kw["border_color"] = parse_color(p)
                    else:
                        try:
                            ir_kw["border_width"] = to_px(p)
                        except (ValueError, TypeError):
                            pass
                continue

            if css_key == "flex":
                self._parse_flex_shorthand(ir_kw, css_val)
                continue
            ir_field = _CSS_TO_IR.get(css_key)
            if ir_field is None:
                continue

            css_val_stripped = css_val.strip() if isinstance(css_val, str) else ""

            # `auto` for width/height → None (fills available / content-based)
            if css_val_stripped == "auto" and ir_field in ("width", "height"):
                continue

            # Values needing layout-time resolution (%, vh, vw) — store raw
            if collect_raw and needs_layout(css_val) and css_val_stripped != "auto":
                raw_styles[css_key] = css_val
                ir_kw[ir_field] = DEFERRED
                continue

            val = _convert_value(ir_field, css_val)
            if val is not None:
                ir_kw[ir_field] = val
                if css_key == "margin":
                    ir_kw["margin_auto"] = _parse_margin_auto(css_val)

        # Merge individual side properties into margin/padding tuples
        for base in ("margin", "padding"):
            for side_field, idx in (("_top_side", 0), ("_right_side", 1), ("_bottom_side", 2), ("_left_side", 3)):
                val = ir_kw.pop(base + side_field, None)
                if val is not None:
                    cur = ir_kw.get(base)
                    if cur is None:
                        tup = [0.0, 0.0, 0.0, 0.0]
                    else:
                        tup = list(cur)
                    tup[idx] = val
                    ir_kw[base] = tuple(tup)

        return ir_kw, raw_styles


# ── Helpers ────────────────────────────────────────────────────


def _get_classes(props: dict) -> list[str]:
    raw = props.get("className") or props.get("class") or ""
    if isinstance(raw, str):
        return raw.split()
    if isinstance(raw, dict) and "__ref__" in raw:
        return []
    return []


def _resolve_tw(props: dict, tw: TailwindResolver) -> dict:
    """Resolve Tailwind classes from className prop into a CSS dict."""
    classes = _get_classes(props)
    merged = {}
    for cls in classes:
        result = tw.resolve(cls)
        if result:
            merged.update(result)
    return merged


def _int_prop(props: dict, key: str, tw_styles: dict, fallback: int) -> int:
    raw = props.get(key)
    if raw is not None:
        try:
            return int(raw)
        except (ValueError, TypeError):
            pass
    tw = tw_styles.get(key)
    if tw is not None:
        try:
            return int(tw)
        except (ValueError, TypeError):
            pass
    return fallback


def _parse_transition_duration(raw: str) -> float:
    """Parse CSS time like '0.3s' or '300ms' to float seconds."""
    raw = raw.strip().lower()
    if raw.endswith("ms"):
        try:
            return float(raw[:-2]) / 1000.0
        except ValueError:
            return 0.0
    if raw.endswith("s"):
        try:
            return float(raw[:-1])
        except ValueError:
            return 0.0
    try:
        return float(raw)
    except ValueError:
        return 0.0


def _convert_value(field: str, raw: str | float | int) -> float | str | tuple | None:
    """Convert a raw CSS string/float to the type expected by IRStyle."""
    if isinstance(raw, (int, float)):
        raw = str(raw)

    if not isinstance(raw, str):
        raw = str(raw)

    if field in ("bg_color", "color"):
        return parse_color(raw)

    if field in ("width", "height", "min_width", "max_width", "min_height", "max_height",
                 "border_radius", "font_size", "flex_grow", "flex_shrink", "gap"):
        try:
            return to_px(raw)
        except (ValueError, TypeError):
            return None

    if field in ("margin", "padding"):
        return _parse_side_value(field, raw)

    if field in ("margin_top_side", "margin_bottom_side", "margin_left_side", "margin_right_side",
                 "padding_top_side", "padding_bottom_side", "padding_left_side", "padding_right_side"):
        try:
            return to_px(raw)
        except (ValueError, TypeError):
            return None

    if field in ("font_weight", "text_align", "display", "flex_dir",
                 "overflow", "position", "justify_content", "align_items",
                 "flex_wrap", "flex_basis", "cursor", "box_sizing"):
        return raw

    if field in ("left", "right", "top", "bottom"):
        try:
            return to_px(raw)
        except (ValueError, TypeError):
            return None

    if field in ("scrollbar_width", "scrollbar_border_radius", "border_width"):
        try:
            return to_px(raw)
        except (ValueError, TypeError):
            return None

    if field in ("scrollbar_track_color", "scrollbar_thumb_color", "border_color"):
        return parse_color(raw)

    if field == "border_style":
        return raw

    return None
