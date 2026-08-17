from __future__ import annotations

import math
from morph.ir.node import IRNode, IRWindow
from morph.ir.style import IRStyle
from morph.ir.event import IREvent
from morph.ir.animation import IRKeyframe, IRAnimation, parse_animations
from morph.style.tailwind import TailwindResolver
from morph.style.selector import matches_selector, calculate_specificity, parse_selector, selector_to_string
from morph.utils.color import parse_color
from morph.style.units import to_px, needs_layout, DEFERRED

import tree_sitter_typescript as tsts
from tree_sitter import Language, Parser

_JS_LANG = Language(tsts.language_tsx())
_JS_PARSER = Parser(_JS_LANG)

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
    # Button defaults mirror browser UA styles (buttonface / buttonborder),
    # so plain buttons are visible without any user CSS. `color` uses a
    # near-black sentinel (#010101) instead of pure black so the runtime
    # treats it as an explicit value and does NOT inherit the parent's
    # color — matching browser behavior where button text stays dark.
    "button": {
        "display": "inline-block",
        "background-color": "#efefef",
        "color": "#010101",
        "border-width": "1px",
        "border-style": "solid",
        "border-color": "#767676",
        "border-radius": "4px",
        "padding": "1px 6px",
        "font-size": "13.33px",
        "text-align": "center",
    },
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

# User-agent default :hover styles (lowest priority — merged FIRST, any
# matching user `:hover` rule overrides per-property, like browsers).
_UA_HOVER_DEFAULTS: dict[str, dict[str, str]] = {
    "button": {"background-color": "#e6e6e6"},
}

# User-agent default :active styles (pressed state — darker face + border,
# mirroring the browser's buttonface → buttonhighlight/buttonshadow shift).
_UA_ACTIVE_DEFAULTS: dict[str, dict[str, str]] = {
    "button": {"background-color": "#d4d4d4", "border-color": "#5a5a5a"},
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
    "z-index":                  "z_index",
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
    "transform":                 "transform_ops",
    "transform-origin":          "transform_origin",
    "opacity":                   "opacity",
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


def _parse_transform_origin(raw: str) -> tuple | None:
    """Parse a CSS `transform-origin` value into ((x, is_pct_x), (y, is_pct_y)).

    One value sets both axes (second defaults to center).  Keywords
    left/top/center/right/bottom map to 0 / 0.5 / 1.  Returns None when the
    value is invalid (property ignored).
    """
    parts = raw.split()
    if len(parts) == 0 or len(parts) > 2:
        return None

    def axis(tok: str) -> tuple[float, bool] | None:
        k = tok.strip().lower()
        if k in ("left", "top"):
            return (0.0, False)
        if k == "center":
            return (0.5, False)
        if k in ("right", "bottom"):
            return (1.0, False)
        if k.endswith("%"):
            try:
                return (float(k[:-1]), True)
            except ValueError:
                return None
        if k.endswith("px"):
            k = k[:-2]
        try:
            return (float(k), False)
        except ValueError:
            return None

    x = axis(parts[0])
    if x is None:
        return None
    if len(parts) == 2:
        y = axis(parts[1])
        if y is None:
            return None
    else:
        y = (0.5, False)
    return (x, y)


class IRBuilder:

    def __init__(self, config=None):
        self.config = config
        self._counter = 0
        self._extra_headers: set[str] = set()

    def build(
        self,
        walked: dict,
        css_rules: dict,
        tw_resolver: TailwindResolver,
        keyframes: dict | None = None,
    ) -> list[IRWindow]:
        ir_windows = []
        # Raw @keyframes: name → [(offset, declarations), ...] → partial
        # IRStyle keyframes (validated against what the runtime can animate).
        ir_keyframes = self._keyframes_to_ir(keyframes or {})

        # ── Transpile top-level function declarations ─────
        premain_functions = []
        for fd in walked.get("function_declarations", []):
            try:
                source = fd["source"]
                tree = _JS_PARSER.parse(source.encode("utf-8"))
                from morph.js.ast_builder import TSAstBuilder
                from morph.js.codegen import TSToCppTranslator
                builder = TSAstBuilder()
                func_node = builder.build_statement(tree.root_node.children[0])
                translator = TSToCppTranslator(indent_level=0)
                cpp = translator.translate(func_node)
                if cpp:
                    premain_functions.append(cpp)
                    self._extra_headers.update(translator._needed)
            except Exception:
                pass

        # ── Transpile module-level variable declarations ──
        for gv in walked.get("global_vars", []):
            try:
                source = gv["source"]
                tree = _JS_PARSER.parse(source.encode("utf-8"))
                from morph.js.ast_builder import TSAstBuilder
                from morph.js.codegen import TSToCppTranslator
                builder = TSAstBuilder()
                stmt = builder.build_statement(tree.root_node.children[0])
                translator = TSToCppTranslator(indent_level=0)
                cpp = translator.translate(stmt)
                if cpp:
                    premain_functions.append(cpp)
                    self._extra_headers.update(translator._needed)
            except Exception:
                pass

        win_cfg = walked.get("windowConfig")

        for comp in walked.get("components", []):
            if not comp.get("exported", False):
                continue

            # Build state variable mapping
            state_vars_map: dict[str, str] = {}
            for sv in comp.get("state_vars", []):
                getter = sv.get("getter", "")
                setter = sv.get("setter", "")
                sig_name = f"__st_{getter}"
                if getter:
                    state_vars_map[getter] = f"{sig_name}.get()"
                if setter:
                    state_vars_map[setter] = f"{sig_name}.set"

            # ── Translate inner functions ──
            comp_premain = list(premain_functions)
            for fd in comp.get("inner_functions", []):
                try:
                    source = fd["source"]
                    tree = _JS_PARSER.parse(source.encode("utf-8"))
                    from morph.js.ast_builder import TSAstBuilder
                    from morph.js.codegen import TSToCppTranslator
                    builder = TSAstBuilder()
                    func_node = builder.build_statement(tree.root_node.children[0])
                    translator = TSToCppTranslator(indent_level=0,
                                                    state_vars=state_vars_map)
                    cpp = translator.translate(func_node)
                    if cpp:
                        comp_premain.append(cpp)
                        self._extra_headers.update(translator._needed)
                except Exception:
                    pass

            # ── Transpile morphEffect bodies ──
            effect_decls = []
            for ef in comp.get("morph_effects", []):
                try:
                    cb_source = ef["callback"]
                    deps_source = ef.get("deps", "")
                    tree = _JS_PARSER.parse(cb_source.encode("utf-8"))
                    from morph.js.ast_builder import TSAstBuilder
                    from morph.js.codegen import TSToCppTranslator
                    builder = TSAstBuilder()
                    arrow_fn_node = tree.root_node.children[0].children[0]
                    arrow_fn = builder.build_expression(arrow_fn_node)
                    translator = TSToCppTranslator(indent_level=0, event_handler=False,
                                                    state_vars=state_vars_map)
                    translator._fn_body_depth = 1
                    cpp_lambda = translator.translate(arrow_fn)
                    effect_decls.append({
                        "lambda": cpp_lambda,
                        "deps": deps_source,
                    })
                    self._extra_headers.update(translator._needed)
                except Exception:
                    pass
            # ── End morphEffect transpilation ──

            jsx = comp.get("jsx", {})
            tag = jsx.get("tag")

            if win_cfg:
                window_id = self._next_id()
                window_nodes = []
                children = jsx.get("children", []) if tag == "__fragment__" else [jsx]
                for child in children:
                    node = self._build_node(child, css_rules, tw_resolver,
                                            state_vars_map=state_vars_map,
                                            keyframes=ir_keyframes)
                    if node:
                        window_nodes.append(node)

                ir_windows.append(IRWindow(
                    window_id=window_id,
                    nodes=window_nodes,
                    startup_logs=comp.get("body_logs", []),
                    title=win_cfg.get("title", str(getattr(self.config, "name", "Untitled"))),
                    width=win_cfg.get("width", 800),
                    height=win_cfg.get("height", 600),
                    renderer=getattr(self.config, "renderer", "flash"),
                    premain_functions=comp_premain,
                    extra_headers=sorted(self._extra_headers),
                    state_vars=comp.get("state_vars", []),
                    effect_decls=effect_decls,
                    keyframes=ir_keyframes,
                ))
            elif tag == "morph-window":
                props = jsx.get("props", {})
                tw_styles = _resolve_tw(props, tw_resolver)

                window_nodes = []
                for child in jsx.get("children", []):
                    node = self._build_node(child, css_rules, tw_resolver,
                                            state_vars_map=state_vars_map,
                                            keyframes=ir_keyframes)
                    if node:
                        window_nodes.append(node)

                ir_windows.append(IRWindow(
                    window_id=self._next_id(),
                    nodes=window_nodes,
                    startup_logs=comp.get("body_logs", []),
                    title=props.get("title", str(getattr(self.config, "name", "Untitled"))),
                    width=_int_prop(props, "width", tw_styles, 800),
                    height=_int_prop(props, "height", tw_styles, 600),
                    renderer=getattr(self.config, "renderer", "flash"),
                    premain_functions=comp_premain,
                    extra_headers=sorted(self._extra_headers),
                    state_vars=comp.get("state_vars", []),
                    effect_decls=effect_decls,
                    keyframes=ir_keyframes,
                ))

        return ir_windows

    def _transpile_js_expr(self, js_source: str,
                           state_vars_map: dict[str, str] | None = None) -> str:
        """Transpile a JS expression like 'count + 1' to C++."""
        if not js_source.strip():
            return ""
        try:
            tree = _JS_PARSER.parse(js_source.encode("utf-8"))
            from morph.js.ast_builder import TSAstBuilder
            from morph.js.codegen import TSToCppTranslator
            builder = TSAstBuilder()
            expr = builder.build_expression(tree.root_node)
            translator = TSToCppTranslator(indent_level=0,
                                            state_vars=state_vars_map or {})
            # expr is a TSProgram — extract the inner expression directly
            # to avoid _translate_program adding #include headers
            if (hasattr(expr, 'statements') and expr.statements
                    and hasattr(expr.statements[0], 'expression')):
                inner = expr.statements[0].expression
                cpp = translator._translate_node(inner)
            else:
                # Empty / non-expression program — return empty string
                if (hasattr(expr, 'statements') and not expr.statements):
                    return ""
                cpp = translator.translate(expr)
            self._extra_headers.update(translator._needed)
            return cpp
        except Exception:
            return js_source  # fallback: use raw source

    def _build_node(
        self,
        jsx_node: dict,
        css_rules: dict,
        tw_resolver: TailwindResolver,
        ancestry: list[tuple[str, list[str]]] | None = None,
        state_vars_map: dict[str, str] | None = None,
        keyframes: dict[str, list[IRKeyframe]] | None = None,
    ) -> IRNode | None:
        tag = jsx_node.get("tag")
        if not tag:
            return None

        node_id = self._next_id()
        props = jsx_node.get("props", {})

        # ── Reactive text node (expression interpolation) ───
        if tag == "__expr__":
            raw_expr = jsx_node.get("text", "")
            cpp_expr = self._transpile_js_expr(raw_expr, state_vars_map)
            return IRNode(
                node_id=node_id,
                node_type="__text__",
                text_content="",
                reactive_text=cpp_expr,
                style=IRStyle(display="inline"),
                children=[],
                events=[],
            )

        # ── Conditional node ─────────────────────────────────
        if tag == "__conditional__":
            cond_expr = self._transpile_js_expr(jsx_node.get("condition", ""),
                                                 state_vars_map)
            then_nodes = [
                self._build_node(n, css_rules, tw_resolver, ancestry,
                                 state_vars_map=state_vars_map,
                                 keyframes=keyframes)
                for n in jsx_node.get("then_branch", [])
                if n is not None
            ]
            else_nodes = [
                self._build_node(n, css_rules, tw_resolver, ancestry,
                                 state_vars_map=state_vars_map,
                                 keyframes=keyframes)
                for n in jsx_node.get("else_branch", [])
                if n is not None
            ]
            return IRNode(
                node_id=node_id,
                node_type="__conditional__",
                style=IRStyle(),
                children=[],
                events=[],
                condition_expr=cond_expr,
                then_nodes=[n for n in then_nodes if n is not None],
                else_nodes=[n for n in else_nodes if n is not None],
            )

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

        # Extract reactive inline style expressions
        reactive_style: dict[str, str] = {}
        filtered_inline: dict[str, str] = {}
        for css_key, val in inline_raw.items():
            if isinstance(val, dict) and "__expr__" in val:
                cpp_expr = self._transpile_js_expr(val["__expr__"], state_vars_map)
                reactive_style[css_key] = cpp_expr
            else:
                filtered_inline[css_key] = val

        # Extract reactive className and resolve conditional class effects
        raw_class = props.get("className") or props.get("class") or ""
        reactive_class = ""
        class_conditional_effects: list[tuple[str, dict[str, str], dict[str, str]]] = []
        if isinstance(raw_class, dict):
            if "__template__" in raw_class:
                js_source = raw_class["__template__"]
                reactive_class = self._transpile_js_expr(js_source, state_vars_map)
                class_conditional_effects = self._analyze_class_template(js_source, tw_resolver, state_vars_map)
                raw_class = ""
            elif "__expr__" in raw_class:
                js_source = raw_class["__expr__"]
                reactive_class = self._transpile_js_expr(js_source, state_vars_map)
                class_conditional_effects = self._analyze_class_expression(js_source, tw_resolver, state_vars_map)
                raw_class = ""
            elif "__ref__" in raw_class:
                js_source = raw_class["__ref__"]
                reactive_class = self._transpile_js_expr(js_source, state_vars_map)
                raw_class = ""

        # Rebuild class_names from the (now-sanitized) raw_class
        class_names = _get_classes({"className": raw_class}) if isinstance(raw_class, str) else []
        tw_styles = _resolve_tw({"className": raw_class}, tw_resolver) if isinstance(raw_class, str) else {}
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
        matched = []               # non-:hover/:active rules
        hover_matched = []         # :hover pseudo-class on THIS element
        ancestor_hover_matched = []  # :hover on an ANCESTOR compound
        active_matched = []        # :active pseudo-class on THIS element
        ancestor_active_matched = []  # :active on an ANCESTOR compound
        for rule_key, rule_props in css_rules.items():
            has_state = ":hover" in rule_key or ":active" in rule_key
            if not has_state:
                if not matches_selector(rule_key, tag, class_names, node_id_attr, ancestry):
                    continue
                spec = calculate_specificity(rule_key)
                matched.append((spec, rule_props))
            else:
                # Strip pseudo-classes and check if the node matches the structural part
                match_key = rule_key.replace(":hover", "").replace(":active", "").strip()
                if not matches_selector(match_key, tag, class_names, node_id_attr, ancestry):
                    continue
                spec = calculate_specificity(rule_key)
                # Parse selector to determine whether each pseudo-class sits
                # on this element or on an ancestor compound
                selectors = parse_selector(rule_key)
                for pseudo in ("hover", "active"):
                    is_self_state = False
                    is_ancestor_state = False
                    ancestor_tag = None
                    for sel in selectors:
                        for i, comp in enumerate(sel.compounds):
                            if comp.pseudo and pseudo in comp.pseudo.split(":"):
                                if i == len(sel.compounds) - 1:
                                    is_self_state = True
                                else:
                                    ancestor_tag = comp.tag
                                    is_ancestor_state = True
                                break
                    if is_ancestor_state and ancestor_tag:
                        (ancestor_hover_matched if pseudo == "hover"
                         else ancestor_active_matched).append((spec, rule_props, ancestor_tag))
                    elif is_self_state:
                        (hover_matched if pseudo == "hover"
                         else active_matched).append((spec, rule_props))
        matched.sort(key=lambda x: x[0])   # lowest specificity first
        hover_matched.sort(key=lambda x: x[0])
        ancestor_hover_matched.sort(key=lambda x: x[0])
        active_matched.sort(key=lambda x: x[0])
        ancestor_active_matched.sort(key=lambda x: x[0])
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
        merged.update(filtered_inline)  # inline style overrides everything

        # ── Convert merged CSS → IRStyle fields ──────────────
        ir_kw, raw_styles = self._css_to_ir_kw(merged)

        # Build hover style: UA defaults first, then matching :hover CSS rules
        # (lowest priority — user rules override per-property)
        hover_merged = dict(_UA_HOVER_DEFAULTS.get(tag, {}))
        for _, rule_props in hover_matched:
            hover_merged.update(rule_props)
        hover_style = None
        if hover_merged:
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

        # Build active style: UA defaults first, then matching :active CSS rules
        # (lowest priority — user rules override per-property)
        active_merged = dict(_UA_ACTIVE_DEFAULTS.get(tag, {}))
        for _, rule_props in active_matched:
            active_merged.update(rule_props)
        active_style = None
        if active_merged:
            active_ir_kw, _ = self._css_to_ir_kw(active_merged, collect_raw=False)
            try:
                active_style = IRStyle(**active_ir_kw)
            except TypeError:
                pass

        # Build ancestor active rules (pairs of ancestor_tag → resolved IRStyle)
        ancestor_active_rules = []
        if ancestor_active_matched:
            by_tag: dict[str, dict] = {}
            for _, rule_props, ancestor_tag in ancestor_active_matched:
                existing = by_tag.get(ancestor_tag)
                if existing is None:
                    by_tag[ancestor_tag] = dict(rule_props)
                else:
                    existing.update(rule_props)
            for ancestor_tag, props in by_tag.items():
                rule_ir_kw, _ = self._css_to_ir_kw(props, collect_raw=False)
                try:
                    rule_style = IRStyle(**rule_ir_kw)
                    ancestor_active_rules.append((ancestor_tag, rule_style))
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
            child_node = self._build_node(child, css_rules, tw_resolver,
                                          child_ancestry,
                                          state_vars_map=state_vars_map,
                                          keyframes=keyframes)
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

        # JSX event props → IREvent trigger mapping
        _EVENT_PROPS = {
            "onClick":"click",
            "onKeyUp": "keyup",
            "onKeyDown": "keydown",
            "onDoubleClick": "dblclick",
            "onMouseDown":  "mousedown",
            "onMouseUp":    "mouseup",
            "onMouseEnter": "mouseenter",
            "onMouseLeave": "mouseleave",
        }

        # morph-* prefixed attributes
        for attr_key in ("morph-open", "morph-close", "morph-navigate"):
            target = props.get(attr_key)
            if target:
                action = attr_key.split("-")[1]
                events.append(IREvent(trigger="click", action=action, target=target))

        # JS event props → transpile to C++ lambda
        for jsx_prop, trigger in _EVENT_PROPS.items():
            val = props.get(jsx_prop)
            if isinstance(val, dict):
                if "__fn__" in val:
                    try:
                        fn_source = val["__fn__"]
                        tree = _JS_PARSER.parse(fn_source.encode("utf-8"))
                        arrow_fn_node = tree.root_node.children[0].children[0]
                        from morph.js.ast_builder import TSAstBuilder
                        from morph.js.codegen import TSToCppTranslator
                        builder = TSAstBuilder()
                        arrow_fn = builder.build_expression(arrow_fn_node)
                        translator = TSToCppTranslator(indent_level=0, event_handler=True,
                                                        state_vars=state_vars_map or {})
                        translator._fn_body_depth = 1  # emitted inside main() — [&] is valid
                        cpp_lambda = translator.translate(arrow_fn)
                        events.append(IREvent(trigger=trigger, action="call", target=cpp_lambda))
                        self._extra_headers.update(translator._needed)
                    except Exception:
                        pass  # transpilation failed — skip this event
                elif "__ref__" in val:
                    ref_name = val["__ref__"]
                    events.append(IREvent(trigger=trigger, action="call",
                                          target=f"[&](JsObject) -> void {{ {ref_name}(); }}"))

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

        # ── CSS animations ───────────────────────────────────
        # Parse the `animation` shorthand + longhands, then drop any whose
        # keyframe name is unknown (browsers ignore those entirely).
        animations: list[IRAnimation] = []
        if keyframes:
            for anim in parse_animations(merged):
                if anim.name in keyframes:
                    animations.append(anim)

        # Same for `:hover` rules — animations there only run while hovered.
        hover_animations: list[IRAnimation] = []
        if keyframes:
            for anim in parse_animations(hover_merged):
                if anim.name in keyframes:
                    hover_animations.append(anim)

        return IRNode(
            node_id=node_id,
            node_type=tag,
            style=node_style,
            hover_style=hover_style,
            active_style=active_style,
            children=children_nodes,
            events=events,
            attrs=attrs,
            raw_styles=raw_styles,
            transition_duration=trans_dur,
            transition_easing=trans_easing,
            ancestor_hover_rules=ancestor_hover_rules,
            ancestor_active_rules=ancestor_active_rules,
            reactive_class=reactive_class,
            reactive_style=reactive_style,
            class_conditional_effects=class_conditional_effects,
            animations=animations,
            hover_animations=hover_animations,
        )

    def _analyze_class_template(self, js_source: str, tw_resolver: TailwindResolver,
                                 state_vars_map: dict[str, str] | None = None
                                 ) -> list[tuple[str, dict[str, str], dict[str, str]]]:
        """Parse a template literal className expression.

        For each expression part that is a ternary, extract:
          - condition (C++ expression)
          - class names from consequent branch → resolved on_styles
          - class names from alternate branch → resolved off_styles

        Returns [(condition_cpp, on_styles, off_styles), ...]
        """
        effects: list[tuple[str, dict[str, str], dict[str, str]]] = []
        try:
            from morph.js.ast_builder import TSAstBuilder
            from morph.js.ast import TSLiteral, TSTemplateLiteral, TSTernaryExpression
            from morph.js.ast import TSNode
            tree = _JS_PARSER.parse(js_source.encode("utf-8"))
            builder = TSAstBuilder()
            prog = builder.build_expression(tree.root_node)

            # Unwrap TSProgram → TSExpressionStatement → inner expression
            inner = None
            if (hasattr(prog, 'statements') and prog.statements
                    and hasattr(prog.statements[0], 'expression')):
                inner = prog.statements[0].expression

            def _resolve_to_css(class_str: str) -> dict[str, str]:
                result: dict[str, str] = {}
                for cls in class_str.split():
                    cls = cls.strip()
                    if cls:
                        tw = tw_resolver.resolve(cls)
                        if tw:
                            result.update(tw)
                return result

            def _extract_class_string(node: TSNode) -> str:
                """Get the raw class string from a TSLiteral or str."""
                if isinstance(node, TSLiteral) and isinstance(node.value, str):
                    return node.value
                return ""

            from morph.js.codegen import TSToCppTranslator
            translator = TSToCppTranslator(indent_level=0, state_vars=state_vars_map or {})

            if isinstance(inner, TSTemplateLiteral):
                for part in inner.parts:
                    if isinstance(part, TSTernaryExpression):
                        cond_cpp = translator._translate_node(part.condition)
                        on_str = _extract_class_string(part.consequent)
                        off_str = _extract_class_string(part.alternate)
                        on_styles = _resolve_to_css(on_str)
                        off_styles = _resolve_to_css(off_str)
                        if on_styles or off_styles:
                            effects.append((cond_cpp, on_styles, off_styles))
        except Exception:
            pass
        return effects

    def _analyze_class_expression(self, js_source: str, tw_resolver: TailwindResolver,
                                   state_vars_map: dict[str, str] | None = None
                                   ) -> list[tuple[str, dict[str, str], dict[str, str]]]:
        """Analyze a non-template className expression (e.g., ternary).

        Extracts conditional class style effects from ternary expressions
        like 'count > 0 ? \"active\" : \"\"'.
        """
        effects: list[tuple[str, dict[str, str], dict[str, str]]] = []
        try:
            from morph.js.ast_builder import TSAstBuilder
            tree = _JS_PARSER.parse(js_source.encode("utf-8"))
            builder = TSAstBuilder()
            prog = builder.build_expression(tree.root_node)
            from morph.js.ast import TSLiteral, TSTernaryExpression
            from morph.js.codegen import TSToCppTranslator

            translator = TSToCppTranslator(indent_level=0, state_vars=state_vars_map or {})

            def _resolve_to_css(class_str: str) -> dict[str, str]:
                result: dict[str, str] = {}
                for cls in class_str.split():
                    cls = cls.strip()
                    if cls:
                        tw = tw_resolver.resolve(cls)
                        if tw:
                            result.update(tw)
                return result

            # Unwrap TSProgram → TSExpressionStatement → inner expression
            inner = None
            if (hasattr(prog, 'statements') and prog.statements
                    and hasattr(prog.statements[0], 'expression')):
                inner = prog.statements[0].expression

            if isinstance(inner, TSTernaryExpression):
                cond_cpp = translator._translate_node(inner.condition)
                on_str = (inner.consequent.value if isinstance(inner.consequent, TSLiteral)
                          and isinstance(inner.consequent.value, str) else "")
                off_str = (inner.alternate.value if isinstance(inner.alternate, TSLiteral)
                           and isinstance(inner.alternate.value, str) else "")
                on_styles = _resolve_to_css(on_str)
                off_styles = _resolve_to_css(off_str)
                if on_styles or off_styles:
                    effects.append((cond_cpp, on_styles, off_styles))
        except Exception:
            pass
        return effects

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

    # CSS properties the animation runtime can interpolate. Keyframe values
    # for everything else are ignored (browsers ignore non-animatable too).
    _ANIMATABLE_PROPS = {
        "opacity", "background-color", "color", "border-radius", "font-size",
        "width", "height", "left", "top", "transform",
    }

    def _keyframes_to_ir(self, keyframes: dict) -> dict[str, list[IRKeyframe]]:
        """Convert raw @keyframes (name → [(offset, declarations)]) to
        partial-styles keyframes the C++ runtime can sample.

        Values needing layout-time resolution (% lengths) and transforms are
        kept as raw CSS strings resolved against the element's box at
        runtime; everything else is baked to pixels/colors here.
        """
        result: dict[str, list[IRKeyframe]] = {}
        for name, kfs in keyframes.items():
            converted = []
            for offset, decls in kfs:
                raw: dict[str, str] = {}
                static: dict[str, str] = {}
                for prop, val in decls.items():
                    if prop not in self._ANIMATABLE_PROPS:
                        continue
                    if prop == "transform":
                        raw[prop] = val
                    elif needs_layout(val):
                        raw[prop] = val
                    else:
                        static[prop] = val
                ir_kw, _ = self._css_to_ir_kw(static, collect_raw=False)
                converted.append(IRKeyframe(
                    offset=offset,
                    style=IRStyle(**ir_kw) if ir_kw else IRStyle(),
                    declared=set(ir_kw),
                    raw=raw,
                ))
            result[name] = converted
        return result

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

    if field == "opacity":
        try:
            return max(0.0, min(1.0, float(raw)))
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

    if field == "z_index":
        if raw.strip().lower() == "auto":
            return None
        try:
            return int(float(raw))
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

    if field == "transform_ops":
        from morph.style.transforms import parse_transform
        ops = parse_transform(raw)
        # None = invalid (property ignored); [] = none/keyword (no transform)
        return ops if ops else None

    if field == "transform_origin":
        return _parse_transform_origin(raw)

    return None
