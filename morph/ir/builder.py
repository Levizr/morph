from __future__ import annotations

from morph.ir.node import IRNode, IRWindow
from morph.ir.style import IRStyle
from morph.ir.event import IREvent
from morph.style.tailwind import TailwindResolver
from morph.utils.color import parse_color
from morph.style.units import to_px

# User-agent default styles for HTML tags (lowest priority — overridden by everything)
_UA_DEFAULTS: dict[str, dict[str, str]] = {
    "h1": {"font-size": "32px",    "font-weight": "bold", "margin": "21.44px 0"},
    "h2": {"font-size": "24px",    "font-weight": "bold", "margin": "19.92px 0"},
    "h3": {"font-size": "18.72px", "font-weight": "bold", "margin": "18.72px 0"},
    "h4": {"font-size": "16px",    "font-weight": "bold", "margin": "21.28px 0"},
    "h5": {"font-size": "13.28px", "font-weight": "bold", "margin": "22.18px 0"},
    "h6": {"font-size": "10.72px", "font-weight": "bold", "margin": "24.97px 0"},
    "p":   {"margin": "16px 0"},
    "strong": {"font-weight": "bold"},
    "span": {"display": "inline"},
    "a":    {"display": "inline", "color": "#0000ee", "cursor": "pointer"},
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
    "flex":                     "flex",
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
    """Parse a CSS shorthand like '10px 20px' or a single value into 4 sides."""
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
        for comp in walked.get("components", []):
            if not comp.get("exported", False):
                continue

            jsx = comp.get("jsx", {})
            tag = jsx.get("tag")

            if tag == "morph-window":
                window_id = self._next_id()
                props = jsx.get("props", {})

                # ── Resolve Tailwind classes on window itself ──
                tw_styles = _resolve_tw(props, tw_resolver)

                config = {
                    "title":  props.get("title",  str(getattr(self.config, "name", "Untitled"))),
                    "width":  _int_prop(props, "width",  tw_styles, 800),
                    "height": _int_prop(props, "height", tw_styles, 600),
                }

                window_nodes = []
                for child in jsx.get("children", []):
                    node = self._build_node(child, css_rules, tw_resolver)
                    if node:
                        window_nodes.append(node)

                ir_windows.append(IRWindow(
                    window_id=window_id,
                    nodes=window_nodes,
                    startup_logs=comp.get("body_logs", []),
                    **config,
                ))

        return ir_windows

    def _build_node(
        self,
        jsx_node: dict,
        css_rules: dict,
        tw_resolver: TailwindResolver,
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
                style=IRStyle(),
                children=[],
                events=[],
            )

        # ── Resolve CSS cascade ──────────────────────────────
        inline_raw = props.get("style", {})
        if isinstance(inline_raw, str):
            inline_raw = {}
        class_names = _get_classes(props)
        tw_styles = _resolve_tw(props, tw_resolver)

        # Merge: inline > Tailwind > CSS rules > UA defaults > system defaults
        merged = {}
        merged.update(_UA_DEFAULTS.get(tag, {}))
        for rule_key in css_rules:
            if _selector_matches(tag, rule_key, class_names):
                merged.update(css_rules[rule_key])
        merged.update(tw_styles)
        merged.update(inline_raw)

        # ── Convert merged CSS → IRStyle fields ──────────────
        ir_kw = {}
        for css_key, css_val in merged.items():
            if css_key == "border":
                # Expand border shorthand: <width> <style> <color>
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
            ir_field = _CSS_TO_IR.get(css_key)
            if ir_field is None:
                continue
            val = _convert_value(ir_field, css_val)
            if val is not None:
                ir_kw[ir_field] = val

        # Merge individual side properties into margin/padding tuples
        for base in ("margin", "padding"):
            # tuple index: 0=top, 1=right, 2=bottom, 3=left
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

        try:
            node_style = IRStyle(**ir_kw)
        except TypeError:
            node_style = IRStyle()

        # ── Children ─────────────────────────────────────────
        children_nodes = []
        for child in jsx_node.get("children", []):
            child_node = self._build_node(child, css_rules, tw_resolver)
            if child_node:
                children_nodes.append(child_node)

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

        return IRNode(
            node_id=node_id,
            node_type=tag,
            style=node_style,
            children=children_nodes,
            events=events,
        )

    def _next_id(self) -> str:
        self._counter += 1
        return f"node_{self._counter:04d}"


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


def _selector_matches(tag: str, rule_key: str, classes: list[str]) -> bool:
    """Simple tag / class selector matching."""
    key = rule_key.strip()
    if key.startswith("."):
        return key[1:] in classes
    if key.startswith("#"):
        return False  # id matching not needed yet
    return key == tag


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


def _convert_value(field: str, raw: str | float | int) -> float | str | tuple | None:
    """Convert a raw CSS string/float to the type expected by IRStyle."""
    if isinstance(raw, (int, float)):
        raw = str(raw)

    if not isinstance(raw, str):
        raw = str(raw)

    if field in ("bg_color", "color"):
        return parse_color(raw)

    if field in ("width", "height", "min_width", "max_width", "min_height", "max_height",
                 "border_radius", "font_size", "flex", "gap"):
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
                 "flex_wrap", "cursor", "box_sizing"):
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
