from __future__ import annotations

import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRNode, IRViewport
from morph.ir.style import IRStyle

_TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), "templates")


def fmt(v: float) -> str:
    """Format a float for C++ literal — always includes a decimal point."""
    if v == float("inf") or v == float("-inf") or v != v:
        return "0.0f"
    s = f"{v:.1f}"
    return s.rstrip("0").rstrip(".") + ".0f" if "." not in s else s + "f"


def transform_assignments(prefix: str, m: tuple[float, ...],
                          ptr: bool = False) -> list[str]:
    """C++ assignments writing a resolved transform matrix to a style.

    ``ptr=True`` when ``prefix`` is a pointer (e.g. ``node->hoverStyle``).
    """
    op = "->" if ptr else "."
    lines = [f"{prefix}{op}transformSet = true;"]
    for i, v in enumerate(m):
        lines.append(f"{prefix}{op}matrix[{i}] = {fmt(v)};")
    return lines


def transform_origin_assignments(prefix: str, origin: tuple[float, float],
                                 ptr: bool = False) -> list[str]:
    """C++ assignments writing a resolved transform-origin to a style.

    Fractions (0..1) of the element's own border-box size; the runtime
    default is the center (0.5, 0.5), so only non-default origins are
    emitted by callers.
    """
    op = "->" if ptr else "."
    return [
        f"{prefix}{op}originSet = true;",
        f"{prefix}{op}originX = {fmt(origin[0])};",
        f"{prefix}{op}originY = {fmt(origin[1])};",
    ]


_ANIM_EASING_TO_CPP = {
    "linear": "Easing::Linear",
    "ease-in": "Easing::EaseIn",
    "ease-out": "Easing::EaseOut",
    "ease-in-out": "Easing::EaseInOut",
}

_ANIM_DIRECTION_TO_CPP = {
    "normal": "AnimDirection::Normal",
    "reverse": "AnimDirection::Reverse",
    "alternate": "AnimDirection::Alternate",
    "alternate-reverse": "AnimDirection::AlternateReverse",
}

_ANIM_FILL_MODE_TO_CPP = {
    "none": "AnimFillMode::None",
    "forwards": "AnimFillMode::Forwards",
    "backwards": "AnimFillMode::Backwards",
    "both": "AnimFillMode::Both",
}

# Raw keyframe CSS property → KeyframeProperty enum member.
_RAW_PROP_TO_ENUM = {
    "opacity": "Opacity",
    "background-color": "BgColor",
    "color": "Color",
    "border-radius": "BorderRadius",
    "font-size": "FontSize",
    "width": "Width",
    "height": "Height",
    "left": "Left",
    "top": "Top",
    "transform": "Transform",
}


def keyframe_registration_code(keyframes: dict[str, list],
                               features: set[str]) -> str:
    """C++ registering the global @keyframes registry (prod builds).

    Emitted only when the animation feature is active; a no-op otherwise.
    """
    if "animation" not in features or not keyframes:
        return ""

    # IRStyle field → (KeyframeProperty name, value formatting)
    def _style_values(kf) -> list[tuple[str, str]]:
        s = kf.style
        # `declared` (set by the IR builder) distinguishes explicit
        # declarations like `opacity: 1` from unset fields — default-value
        # comparisons would silently drop them.  Fall back to the legacy
        # comparisons for hand-built keyframes without the field.
        declared = getattr(kf, "declared", None)
        def is_on(ir_field, default_cond):
            return ir_field in declared if declared is not None else default_cond

        out: list[tuple[str, str]] = []
        if is_on("opacity", s.opacity != 1.0):
            out.append(("Opacity", fmt(s.opacity)))
        if is_on("bg_color", s.bg_color != (0, 0, 0, 0)):
            out.append(("BgColor", f"{s.bg_color[0]:.4f}f, {s.bg_color[1]:.4f}f, "
                                  f"{s.bg_color[2]:.4f}f, {s.bg_color[3]:.4f}f"))
        if is_on("color", s.color != (0, 0, 0, 1)):
            out.append(("Color", f"{s.color[0]:.4f}f, {s.color[1]:.4f}f, "
                                 f"{s.color[2]:.4f}f, {s.color[3]:.4f}f"))
        if is_on("border_radius", s.border_radius > 0):
            out.append(("BorderRadius", fmt(s.border_radius)))
        if is_on("font_size", s.font_size != 16.0):
            out.append(("FontSize", fmt(s.font_size)))
        if is_on("width", s.width is not None):
            out.append(("Width", fmt(s.width)))
        if is_on("height", s.height is not None):
            out.append(("Height", fmt(s.height)))
        if is_on("left", s.left is not None):
            out.append(("Left", fmt(s.left)))
        if is_on("top", s.top is not None):
            out.append(("Top", fmt(s.top)))
        return out

    lines = ["// ── @keyframes registry (feature: animation) ──",
             "morphClearKeyframes();"]
    for name, kfs in sorted(keyframes.items()):
        for kf in kfs:
            lines.append(f"morphAddKeyframe(\"{name}\", {fmt(kf.offset)}, {{")
            for prop, val in _style_values(kf):
                lines.append(f"    {{KeyframeProperty::{prop}, {{{val}}}}},")
            for prop, css in kf.raw.items():
                enum_name = _RAW_PROP_TO_ENUM.get(prop)
                if enum_name is None:
                    continue  # unsupported raw property — skipped
                escaped = css.replace("\\", "\\\\").replace("\"", "\\\"")
                lines.append(
                    f"    {{KeyframeProperty::{enum_name}, {{}}, \"{escaped}\"}},")
            lines.append("});")
    return "\n".join(lines)


class NodeEmitter:
    """Generates C++ instantiation code for an IR node tree.

    Only emits style fields whose associated feature is enabled in the
    feature set — ensures zero references to struct members that don't
    exist when the corresponding feature define is absent.
    """

    def __init__(self, features: set[str] | None = None):
        self.env = Environment(
            loader=FileSystemLoader(_TEMPLATES_DIR),
            trim_blocks=True,
            lstrip_blocks=True,
        )
        self.features = features or set()

    def emit_node(self, node: IRNode, parent_id: str | None = None,
                  parent_style: IRStyle | None = None,
                  list_mode: bool = False) -> str:
        """Return C++ code for a single node and all its children (recursive).

        ``list_mode`` — emitted inside a list item factory: reactive effects
        capture the node pointer by value plus ``&__it``/``&__index`` (the
        binding refs) instead of the enclosing scope, and conditionals keep
        their state in a heap shared_ptr so nothing dangles after the factory
        returns. Effects are always registered on the node so destruction
        marks them dead."""
        if isinstance(node, IRViewport):
            return self._emit_viewport(node, parent_id)

        lines = []
        indent = "    "

        if node.node_type == "__list__":
            return "\n".join(self._emit_list(node, parent_id, indent, list_mode))

        if node.node_type == "__text__":
            caps = "__LCAPS__" if list_mode else ""
            if node.reactive_text:
                lines.append(f"TextNode* {node.node_id} = new TextNode(\"\");")
            else:
                escaped = node.text_content.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r")
                lines.append(f"TextNode* {node.node_id} = new TextNode(\"{escaped}\");")
            lines.append(f"{indent}{node.node_id}->x = {fmt(node.x)};")
            lines.append(f"{indent}{node.node_id}->y = {fmt(node.y)};")
            lines.append(f"{indent}{node.node_id}->w = {fmt(node.w)};")
            lines.append(f"{indent}{node.node_id}->h = {fmt(node.h)};")
            lines.append(self._set_style(node, indent, parent_style))
            lines.append(self._set_animations(node, indent))
            lines.append(self._set_hover_animations(node, indent))
            lines.append(self._set_hover_style(node, indent))
            lines.append(self._set_active_style(node, indent))
            lines.append(self._set_ancestor_hover_rules(node, indent))
            lines.append(self._set_ancestor_active_rules(node, indent))
            if parent_id:
                lines.append(self._set_transition(node, indent))
            if parent_id:
                lines.append(f"{parent_id}->addChild({node.node_id});")
            if node.reactive_text:
                lines.append(f"{node.node_id}->m_associatedEffects.push_back(morph::create_effect([{node.node_id}{caps}]() {{")
                lines.append(f"{indent}{node.node_id}->setText(morph::str({node.reactive_text}));")
                lines.append(f"}}));")
            return "\n".join(lines)

        # ── Conditional node ─────────────────────────────────
        if node.node_type == "__conditional__":
            if list_mode:
                return "\n".join(self._emit_conditional_list_mode(node, parent_id, indent))
            child_then = f"__cond_then_{node.node_id}"
            child_else = f"__cond_else_{node.node_id}"
            lines.append(f"RectNode* {node.node_id} = new RectNode(0.0f, 0.0f, 0.0f, 0.0f);")
            # Per-build heap slots for the branch caches. The effect lambda
            # owns them via shared_ptr value-captures, so:
            #  - they outlive this build block (effects fire later, when the
            #    block's stack frame is long gone — capturing these locals by
            #    reference segfaulted prod builds),
            #  - a rebuilt branch (logout → login) gets FRESH slots instead
            #    of stale pointers into freed subtrees.
            lines.append(f"auto {child_then} = std::make_shared<MorphNode*>(nullptr);")
            lines.append(f"auto {child_else} = std::make_shared<MorphNode*>(nullptr);")
            if parent_id:
                lines.append(f"{parent_id}->addChild({node.node_id});")
            # Build then/else branch code once (without parent wiring)
            then_code = ""
            then_root_var = ""
            for tn in node.then_nodes:
                then_code = self.emit_node(tn, None, None)
                then_root_var = tn.node_id
                break  # V1: single root per branch
            else_code = ""
            else_root_var = ""
            for en in node.else_nodes:
                else_code = self.emit_node(en, None, None)
                else_root_var = en.node_id
                break

            def _emit_branch(cache_slot: str, other_slot: str,
                             branch_code: str, root_var: str) -> None:
                """Teardown the opposite branch, lazily build this one."""
                bi2 = indent + "    "
                cache_deref = f"*{cache_slot}"
                other_deref = f"*{other_slot}"
                lines.append(f"{bi2}if ({other_deref}) {{")
                lines.append(f"{bi2}    {node.node_id}->removeChild({other_deref});")
                lines.append(f"{bi2}    delete {other_deref};")
                lines.append(f"{bi2}    {other_deref} = nullptr;")
                lines.append(f"{bi2}}}")
                if branch_code and root_var:
                    lines.append(f"{bi2}if (!{cache_deref}) {{")
                    for line in branch_code.split("\n"):
                        lines.append(f"{bi2}    {line}")
                    lines.append(f"{bi2}    {node.node_id}->addChild({root_var});")
                    lines.append(f"{bi2}    {node.node_id}->style.explicitWidth = {root_var}->style.explicitWidth;")
                    lines.append(f"{bi2}    {node.node_id}->style.explicitHeight = {root_var}->style.explicitHeight;")
                    lines.append(f"{bi2}    {cache_deref} = {root_var};")
                    lines.append(f"{bi2}}}")

            # Emit the effect. The container is captured BY VALUE: raw
            # pointer, valid exactly as long as the node itself — and the
            # effect is registered on the node so deleting the node kills
            # the effect before that pointer can go stale.
            bi = indent + "    "
            lines.append(
                f"{node.node_id}->m_associatedEffects.push_back("
                f"morph::create_effect([{node.node_id}, {child_then}, {child_else}]() {{")
            lines.append(f"{bi}if ({node.condition_expr}) {{")
            _emit_branch(child_then, child_else, then_code, then_root_var)
            lines.append(f"{bi}}} else {{")
            _emit_branch(child_else, child_then, else_code, else_root_var)
            lines.append(f"{bi}}}")
            lines.append(f"{bi}{node.node_id}->markDirty(LayoutDirty);")
            lines.append(f"}}));")
            return "\n".join(lines)

        if node.node_type == "button":
            lines.append(f"ButtonNode* {node.node_id} = new ButtonNode();")
            lines.append(f"{indent}{node.node_id}->type = \"button\";")
            lines.append(f"{indent}{node.node_id}->x = {fmt(node.x)};")
            lines.append(f"{indent}{node.node_id}->y = {fmt(node.y)};")
            lines.append(f"{indent}{node.node_id}->w = {fmt(node.w)};")
            lines.append(f"{indent}{node.node_id}->h = {fmt(node.h)};")
        elif node.node_type == "input":
            def _esc(s: str) -> str:
                return s.replace("\\", "\\\\").replace("\"", "\\\"")
            lines.append(f"InputNode* {node.node_id} = new InputNode();")
            lines.append(f"{indent}{node.node_id}->setValue(\"{_esc(node.attrs.get('value', ''))}\");")
            if "placeholder" in node.attrs:
                lines.append(f"{indent}{node.node_id}->placeholder = \"{_esc(node.attrs['placeholder'])}\";")
            if "maxLength" in node.attrs:
                lines.append(f"{indent}{node.node_id}->maxLength = {node.attrs['maxLength']};")
            if "minLength" in node.attrs:
                lines.append(f"{indent}{node.node_id}->minLength = {node.attrs['minLength']};")
            if node.attrs.get("disabled", "").lower() in ("true", "1"):
                lines.append(f"{indent}{node.node_id}->disabled = true;")
            if "type" in node.attrs:
                lines.append(f"{indent}{node.node_id}->inputType = \"{_esc(node.attrs['type'])}\";")
            lines.append(f"{indent}{node.node_id}->x = {fmt(node.x)};")
            lines.append(f"{indent}{node.node_id}->y = {fmt(node.y)};")
            lines.append(f"{indent}{node.node_id}->w = {fmt(node.w)};")
            lines.append(f"{indent}{node.node_id}->h = {fmt(node.h)};")
        elif node.node_type == "img":
            src = node.attrs.get("src", "")
            alt = node.attrs.get("alt", "")
            escaped_src = src.replace("\\", "\\\\").replace("\"", "\\\"")
            escaped_alt = alt.replace("\\", "\\\\").replace("\"", "\\\"")
            lines.append(f"ImageNode* {node.node_id} = new ImageNode(\"{escaped_src}\", \"{escaped_alt}\");")
            lines.append(f"{indent}{node.node_id}->x = {fmt(node.x)};")
            lines.append(f"{indent}{node.node_id}->y = {fmt(node.y)};")
            lines.append(f"{indent}{node.node_id}->w = {fmt(node.w)};")
            lines.append(f"{indent}{node.node_id}->h = {fmt(node.h)};")
        else:
            lines.append(f"RectNode* {node.node_id} = new RectNode("
                         f"{fmt(node.x)}, {fmt(node.y)}, {fmt(node.w)}, {fmt(node.h)});")

        lines.append(self._set_style(node, indent, parent_style))
        lines.append(self._set_animations(node, indent))
        lines.append(self._set_hover_animations(node, indent))
        lines.append(self._set_hover_style(node, indent))
        lines.append(self._set_active_style(node, indent))
        lines.append(self._set_transition(node, indent))

        for event in node.events:
            lines.append(self._emit_event(event, node.node_id, indent))

        lines.append(self._set_ancestor_hover_rules(node, indent))
        lines.append(self._set_ancestor_active_rules(node, indent))

        lines.append("")

        if parent_id:
            lines.append(f"{parent_id}->addChild({node.node_id});")

        s = node.style
        resolved = IRStyle(
            color=s.color if s.color != (0, 0, 0, 1) else (
                parent_style.color if parent_style and parent_style.color != (0, 0, 0, 1) else (0, 0, 0, 1)),
            font_size=s.font_size if s.font_size != 16.0 else (
                parent_style.font_size if parent_style and parent_style.font_size != 16.0 else 16.0),
            font_weight=s.font_weight if s.font_weight != "normal" else (
                parent_style.font_weight if parent_style and parent_style.font_weight != "normal" else "normal"),
            text_align=s.text_align if s.text_align != "left" else (
                parent_style.text_align if parent_style and parent_style.text_align != "left" else "left"),
            max_width=s.max_width,
        )

        for child in node.children:
            lines.append(self.emit_node(child, node.node_id, resolved, list_mode=list_mode))

        lines.append(self._emit_reactive_effects(node, indent, list_mode=list_mode))

        return "\n".join(lines)

    def _emit_list(self, node: IRNode, parent_id: str | None,
                   indent: str, list_mode: bool) -> list[str]:
        """Emit a keyed list container + its wiring.

        arrayFn reads the array source (auto-subscribing); itemFactory is the
        generated static factory; keyFn produces stable keys; the reconcile
        effect re-runs on any signal read by arrayFn.
        """
        caps = "__LCAPS__" if list_mode else ""
        lines = [f"morph::ListContainer* {node.node_id} = new morph::ListContainer("
                 f"{fmt(node.x)}, {fmt(node.y)}, {fmt(node.w)}, {fmt(node.h)});"]
        if parent_id:
            lines.append(f"{parent_id}->addChild({node.node_id});")
        lines.append(self._set_style(node, indent, None))
        lines.append(self._set_animations(node, indent))
        lines.append(self._set_hover_animations(node, indent))
        lines.append(self._set_hover_style(node, indent))
        lines.append(self._set_active_style(node, indent))
        lines.append(self._set_transition(node, indent))
        lines.append(f"{node.node_id}->arrayFn = [{caps}]() {{ return {node.list_expr}; }};")
        lines.append(f"{node.node_id}->itemFactory = __list_factory_{node.node_id};")
        if node.list_key_expr:
            # keyFn uses its own params — explicit captures of __it/__index
            # would conflict with the parameter names on nested lists
            lines.append(f"{node.node_id}->keyFn = [](const JsValue& __it, int __index) -> std::string {{")
            lines.append(f"    return morph::list_key({node.list_key_expr}, __index);")
            lines.append(f"}};")
        # Reconcile effect: create_effect's immediate run performs the initial
        # fill; later runs resubscribe to whatever signals arrayFn reads.
        lines.append(f"{node.node_id}->m_associatedEffects.push_back(morph::create_effect([{node.node_id}{caps}]() {{")
        lines.append(f"    {node.node_id}->reconcile({node.node_id}->arrayFn());")
        lines.append(f"}}));")
        return lines

    def _emit_conditional_list_mode(self, node: IRNode, parent_id: str | None,
                                    indent: str) -> list[str]:
        """Emit a conditional inside a list item factory.

        Unlike the top-level version this cannot capture the enclosing scope
        ([&] would dangle after the factory returns). The branch state lives
        in a heap shared_ptr and the effect is registered on the container so
        deletion marks it dead; the effect only touches the nodes it owns."""
        lines = [f"RectNode* {node.node_id} = new RectNode(0.0f, 0.0f, 0.0f, 0.0f);"]
        if parent_id:
            lines.append(f"{parent_id}->addChild({node.node_id});")
        st = f"__condst_{node.node_id}"
        br = f"__condbr_{node.node_id}"
        lines.append(f"auto {st} = std::make_shared<MorphNode*>(nullptr);")
        lines.append(f"auto {br} = std::make_shared<int>(0); // 1=then, 2=else")
        then_code = ""
        then_root_var = ""
        for tn in node.then_nodes:
            then_code = self.emit_node(tn, None, None, list_mode=True)
            then_root_var = tn.node_id
            break
        else_code = ""
        else_root_var = ""
        for en in node.else_nodes:
            else_code = self.emit_node(en, None, None, list_mode=True)
            else_root_var = en.node_id
            break
        bi = indent + "    "
        lines.append(f"{node.node_id}->m_associatedEffects.push_back(morph::create_effect([{node.node_id}, {st}, {br}__LCAPS__]() {{")
        lines.append(f"{bi}if ({node.condition_expr}) {{")
        lines.append(f"{bi}    if (*{st} && *{br} != 1) {{")
        lines.append(f"{bi}        {node.node_id}->removeChild(*{st});")
        lines.append(f"{bi}        delete *{st};")
        lines.append(f"{bi}        *{st} = nullptr;")
        lines.append(f"{bi}    }}")
        if then_code and then_root_var:
            lines.append(f"{bi}    if (!*{st}) {{")
            for line in then_code.split("\n"):
                lines.append(f"{bi}        {line}")
            lines.append(f"{bi}        {node.node_id}->addChild({then_root_var});")
            lines.append(f"{bi}        {node.node_id}->style.explicitWidth = {then_root_var}->style.explicitWidth;")
            lines.append(f"{bi}        {node.node_id}->style.explicitHeight = {then_root_var}->style.explicitHeight;")
            lines.append(f"{bi}        *{st} = {then_root_var};")
            lines.append(f"{bi}        *{br} = 1;")
            lines.append(f"{bi}    }}")
        lines.append(f"{bi}}} else {{")
        lines.append(f"{bi}    if (*{st} && *{br} != 2) {{")
        lines.append(f"{bi}        {node.node_id}->removeChild(*{st});")
        lines.append(f"{bi}        delete *{st};")
        lines.append(f"{bi}        *{st} = nullptr;")
        lines.append(f"{bi}    }}")
        if else_code and else_root_var:
            lines.append(f"{bi}    if (!*{st}) {{")
            for line in else_code.split("\n"):
                lines.append(f"{bi}        {line}")
            lines.append(f"{bi}        {node.node_id}->addChild({else_root_var});")
            lines.append(f"{bi}        {node.node_id}->style.explicitWidth = {else_root_var}->style.explicitWidth;")
            lines.append(f"{bi}        {node.node_id}->style.explicitHeight = {else_root_var}->style.explicitHeight;")
            lines.append(f"{bi}        *{st} = {else_root_var};")
            lines.append(f"{bi}        *{br} = 2;")
            lines.append(f"{bi}    }}")
        lines.append(f"{bi}}}")
        lines.append(f"{bi}{node.node_id}->markDirty(LayoutDirty);")
        lines.append(f"}}));")
        return lines

    def emit_item_factory(self, node: IRNode) -> str:
        """Emit the file-scope item factory for a __list__ node.

        ``static MorphNode* __list_factory_<id>(morph::ListItemBinding& __b)``
        builds one item subtree per call; ``__b`` lives on the heap (the
        container owns it) so effects may capture ``&__b.item``/``&__b.index``.
        """
        tmpl = node.item_template
        if tmpl is None:
            return ""
        body = self.emit_node(tmpl, None, None, list_mode=True)
        caps = ""
        if "__it" in body:
            caps += ", &__it"
        if "__index" in body:
            caps += ", &__index"
        body = body.replace("__LCAPS__", caps)
        lines = [f"static MorphNode* __list_factory_{node.node_id}(morph::ListItemBinding& __b) {{"]
        if "__it" in caps:
            lines.append("    JsValue& __it = __b.item;")
        if "__index" in caps:
            lines.append("    int& __index = __b.index;")
        lines.append(body)
        lines.append(f"    return {tmpl.node_id};")
        lines.append("}")
        return "\n".join(lines)

    def _emit_reactive_effects(self, node: IRNode, indent: str = "    ",
                               list_mode: bool = False) -> str:
        caps = "__LCAPS__" if list_mode else ""
        """Emit reactive className / inline style / conditional class effects
        (prod mode). Mirrors logic_emitter._emit_node_effects but captures node
        pointers directly instead of going through the dev NodeRegistry."""
        from morph.codegen.logic_emitter import (
            _CSS_TO_STYLE_FIELD, _css_field_reset_assignments,
            _css_val_to_cpp_assignments,
        )
        lines = []
        node_id = node.node_id

        # ── Reactive inline style ──
        if node.reactive_style:
            for css_prop, cpp_expr in node.reactive_style.items():
                field_info = _CSS_TO_STYLE_FIELD.get(css_prop)
                if field_info is None:
                    continue
                field_name, val_type = field_info
                if val_type == "float":
                    lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
                    lines.append(f"{indent}{node_id}->interruptStateTransitions();")
                    lines.append(f"{indent}{node_id}->style.{field_name} = (float)({cpp_expr});")
                    lines.append(f"{indent}{node_id}->markDirty(LayoutDirty);")
                    lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
                    lines.append(f"}}));")
                elif val_type == "string":
                    lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
                    lines.append(f"{indent}{node_id}->interruptStateTransitions();")
                    lines.append(f"{indent}{node_id}->style.{field_name} = morph::str({cpp_expr});")
                    lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
                    lines.append(f"}}));")
                elif val_type == "color":
                    lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
                    lines.append(f"{indent}{node_id}->interruptStateTransitions();")
                    lines.append(f"{indent}morph::setColor({node_id}->style.{field_name}, morph::str({cpp_expr}));")
                    lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
                    lines.append(f"}}));")
                elif val_type == "transform":
                    lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
                    lines.append(f"{indent}{node_id}->interruptStateTransitions();")
                    lines.append(f"{indent}morph::setCssTransform({node_id}->style, morph::str({cpp_expr}), {node_id}->w, {node_id}->h);")
                    lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
                    lines.append(f"}}));")

        # ── Propagate reactive font-size to direct TextNode children ──
        if node.reactive_style:
            for css_prop, cpp_expr in node.reactive_style.items():
                if css_prop != "font-size":
                    continue
                field_info = _CSS_TO_STYLE_FIELD.get(css_prop)
                if field_info is None or field_info[1] != "float":
                    continue
                for child in node.children:
                    if child.node_type == "__text__":
                        lines.append(f"{child.node_id}->m_associatedEffects.push_back(morph::create_effect([{child.node_id}{caps}]() {{")
                        lines.append(f"{indent}{child.node_id}->interruptStateTransitions();")
                        lines.append(f"{indent}{child.node_id}->style.fontSize = (float)({cpp_expr});")
                        lines.append(f"{indent}{child.node_id}->markDirty(LayoutDirty);")
                        lines.append(f"{indent}{child.node_id}->markDirty(PaintDirty);")
                        lines.append(f"}}));")

        # ── Reactive attrs (src/alt on img, value on input) ──
        if node.reactive_attrs:
            for attr_key, cpp_expr in node.reactive_attrs.items():
                lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
                if attr_key == "src" and node.node_type == "img":
                    lines.append(f"{indent}std::string _src = morph::str({cpp_expr});")
                    lines.append(f"{indent}if ({node_id}->src != _src) {{")
                    lines.append(f"{indent}    {node_id}->src = _src;")
                    lines.append(f"{indent}    {node_id}->loaded = false;")
                    lines.append(f"{indent}}}{node_id}->markDirty(PaintDirty);")
                elif attr_key == "value" and node.node_type == "input":
                    lines.append(f"{indent}{node_id}->setValue(morph::str({cpp_expr}));")
                    lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
                else:
                    lines.append(f"{indent}{node_id}->{attr_key} = morph::str({cpp_expr});")
                    lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
                lines.append(f"}}));")

        # ── Reactive className (stores string on node) ──
        if node.reactive_class:
            lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
            lines.append(f"{indent}{node_id}->setClassName(morph::str({node.reactive_class}));")
            lines.append(f"}}));")

        # ── Conditional class style effects (direct condition → style) ──
        for cond_cpp, on_styles, off_styles in node.class_conditional_effects:
            def _assigns(styles: dict[str, str]) -> list[str]:
                out: list[str] = []
                for css_prop, css_val in styles.items():
                    for a in _css_val_to_cpp_assignments(css_prop, css_val):
                        out.append(a.replace("n->", f"{node_id}->"))
                return out

            def _resets(styles: dict[str, str]) -> list[str]:
                out: list[str] = []
                reset_fields = set()
                for css_prop in styles:
                    fi = _CSS_TO_STYLE_FIELD.get(css_prop)
                    if fi:
                        reset_fields.add(fi[0])
                for fname in sorted(reset_fields):
                    for a in _css_field_reset_assignments(fname):
                        out.append(a.replace("n->", f"{node_id}->"))
                return out

            lines.append(f"{node_id}->m_associatedEffects.push_back(morph::create_effect([{node_id}{caps}]() {{")
            lines.append(f"{indent}{node_id}->interruptStateTransitions();")
            lines.append(f"{indent}if ({cond_cpp}) {{")
            for a in _assigns(on_styles):
                lines.append(f"{indent}    {a}")
            lines.append(f"{indent}}} else {{")
            if off_styles:
                for a in _assigns(off_styles):
                    lines.append(f"{indent}    {a}")
            else:
                for a in _resets(on_styles):
                    lines.append(f"{indent}    {a}")
            lines.append(f"{indent}}}")
            lines.append(f"{indent}{node_id}->markDirty(PaintDirty);")
            lines.append(f"}}));")

        return "\n".join(lines)

    def _set_style(self, node: IRNode, indent: str,
                   parent_style: IRStyle | None = None) -> str:
        s = node.style
        lines = []
        prefix = f"{node.node_id}->style"

        # ── Always-available fields (StyleBase) ──
        if s.bg_color != (0, 0, 0, 0):
            lines.append(f"{prefix}.bgColor[0] = {s.bg_color[0]:.4f}f;")
            lines.append(f"{prefix}.bgColor[1] = {s.bg_color[1]:.4f}f;")
            lines.append(f"{prefix}.bgColor[2] = {s.bg_color[2]:.4f}f;")
            lines.append(f"{prefix}.bgColor[3] = {s.bg_color[3]:.4f}f;")
        if s.border_radius > 0:
            lines.append(f"{prefix}.borderRadius = {fmt(s.border_radius)};")
        fs = s.font_size if s.font_size != 16.0 else (
            parent_style.font_size if parent_style and parent_style.font_size != 16.0 else 16.0)
        if fs != 16.0:
            lines.append(f"{prefix}.fontSize = {fmt(fs)};")
        fw = s.font_weight if s.font_weight != "normal" else (
            parent_style.font_weight if parent_style and parent_style.font_weight != "normal" else "normal")
        if fw != "normal":
            lines.append(f"{prefix}.fontWeight = \"{fw}\";")
        # For text nodes, emit explicit color + set flag so runtime can distinguish
        # "inherited default (0,0,0,1)" from "explicitly set to black (0,0,0,1)".
        # When not overridden, runtime reads parent->style.color for hover support.
        if node.node_type == "__text__":
            c = s.color
        else:
            c = s.color if s.color != (0, 0, 0, 1) else (
                parent_style.color if parent_style and parent_style.color != (0, 0, 0, 1) else (0, 0, 0, 1))
        if c != (0, 0, 0, 1):
            lines.append(f"{prefix}.color[0]   = {c[0]:.4f}f;")
            lines.append(f"{prefix}.color[1]   = {c[1]:.4f}f;")
            lines.append(f"{prefix}.color[2]   = {c[2]:.4f}f;")
            if node.node_type == "__text__":
                lines.append(f"{prefix}.color[3]   = {c[3]:.4f}f;")
                lines.append(f"{node.node_id}->m_colorOverridden = true;")
            elif s.color == (0, 0, 0, 1) and parent_style and parent_style.color != (0, 0, 0, 1):
                lines.append(f"{node.node_id}->m_colorInherited = true;")
        if s.padding != (0, 0, 0, 0):
            lines.append(f"{prefix}.padding[0] = {fmt(s.padding[0])};")
            lines.append(f"{prefix}.padding[1] = {fmt(s.padding[1])};")
            lines.append(f"{prefix}.padding[2] = {fmt(s.padding[2])};")
            lines.append(f"{prefix}.padding[3] = {fmt(s.padding[3])};")
        if s.margin != (0, 0, 0, 0):
            for i in range(4):
                v = s.margin[i]
                if v != 0.0 or s.margin_auto[i]:
                    coded = fmt(v) if v != float("inf") else "-1.0f"
                    lines.append(f"{prefix}.margin[{i}] = {coded};")
                    if s.margin_auto[i]:
                        lines.append(f"{prefix}.marginAuto[{i}] = true;")
        if s.width is not None:
            lines.append(f"{prefix}.explicitWidth = {fmt(s.width)};")
        if s.height is not None:
            lines.append(f"{prefix}.explicitHeight = {fmt(s.height)};")
        if s.min_width is not None:
            lines.append(f"{prefix}.minWidth = {fmt(s.min_width)};")
        if s.max_width is not None:
            lines.append(f"{prefix}.maxWidth = {fmt(s.max_width)};")
        if s.min_height is not None:
            lines.append(f"{prefix}.minHeight = {fmt(s.min_height)};")
        if s.max_height is not None:
            lines.append(f"{prefix}.maxHeight = {fmt(s.max_height)};")
        if s.overflow != "visible":
            lines.append(f"{prefix}.overflow = \"{s.overflow}\";")
        if s.display != "block":
            lines.append(f"{prefix}.display = \"{s.display}\";")
        if s.position != "static":
            lines.append(f"{prefix}.position = \"{s.position}\";")
        if s.box_sizing != "content-box":
            lines.append(f"{prefix}.boxSizing = \"{s.box_sizing}\";")
        ta = s.text_align if s.text_align != "left" else (
            parent_style.text_align if parent_style and parent_style.text_align != "left" else "left")
        if ta != "left":
            lines.append(f"{prefix}.textAlign = \"{ta}\";")

        # ── Feature: FLEX ──
        if "flex" in self.features:
            if s.display == "flex" and s.flex_dir != "row":
                lines.append(f"{prefix}.flexDirection = \"{s.flex_dir}\";")
            if s.gap > 0:
                lines.append(f"{prefix}.gap = {fmt(s.gap)};")
            if s.justify_content != "flex-start":
                lines.append(f"{prefix}.justifyContent = \"{s.justify_content}\";")
            if s.align_items != "stretch":
                lines.append(f"{prefix}.alignItems = \"{s.align_items}\";")
            if s.flex_wrap != "nowrap":
                lines.append(f"{prefix}.flexWrap = \"{s.flex_wrap}\";")
            if s.flex_grow != 0.0:
                lines.append(f"{prefix}.flexGrow = {fmt(s.flex_grow)};")
            if s.flex_shrink != 1.0:
                lines.append(f"{prefix}.flexShrink = {fmt(s.flex_shrink)};")
            if s.flex_basis != "auto":
                lines.append(f"{prefix}.flexBasis = \"{s.flex_basis}\";")

        # ── Feature: POSITION ──
        if "position" in self.features:
            if s.left is not None:
                lines.append(f"{prefix}.left = {fmt(s.left)};")
            if s.right is not None:
                lines.append(f"{prefix}.right = {fmt(s.right)};")
            if s.top is not None:
                lines.append(f"{prefix}.top = {fmt(s.top)};")
            if s.bottom is not None:
                lines.append(f"{prefix}.bottom = {fmt(s.bottom)};")

        # ── Feature: ZINDEX ──
        if "zindex" in self.features:
            if s.z_index is not None:
                lines.append(f"{prefix}.zIndex = {int(s.z_index)};")
                lines.append(f"{prefix}.zIndexSet = true;")

        # ── Feature: OPACITY ──
        if "opacity" in self.features:
            if s.opacity != 1.0:
                lines.append(f"{prefix}.opacity = {fmt(s.opacity)};")

        # ── Feature: CURSOR ──
        if "cursor" in self.features:
            if s.cursor != "default":
                lines.append(f"{prefix}.cursor = \"{s.cursor}\";")

        # ── Feature: BORDER ──
        if "border" in self.features:
            if s.border_width > 0:
                lines.append(f"{prefix}.borderWidth = {fmt(s.border_width)};")
            if s.border_color != (0.0, 0.0, 0.0, 1.0):
                bc = s.border_color
                lines.append(f"{prefix}.borderColor[0] = {bc[0]:.4f}f;")
                lines.append(f"{prefix}.borderColor[1] = {bc[1]:.4f}f;")
                lines.append(f"{prefix}.borderColor[2] = {bc[2]:.4f}f;")
                lines.append(f"{prefix}.borderColor[3] = {bc[3]:.4f}f;")
            if s.border_style not in ("", "none"):
                lines.append(f"{prefix}.borderStyle = \"{s.border_style}\";")

        # ── Feature: SCROLL ──
        if "scroll" in self.features:
            if s.scrollbar_width != 8.0:
                lines.append(f"{prefix}.scrollbarWidth = {fmt(s.scrollbar_width)};")
            if s.scrollbar_track_color != (0.85, 0.85, 0.85, 0.4):
                tc = s.scrollbar_track_color
                lines.append(f"{prefix}.scrollbarTrackColor[0] = {tc[0]:.4f}f;")
                lines.append(f"{prefix}.scrollbarTrackColor[1] = {tc[1]:.4f}f;")
                lines.append(f"{prefix}.scrollbarTrackColor[2] = {tc[2]:.4f}f;")
                lines.append(f"{prefix}.scrollbarTrackColor[3] = {tc[3]:.4f}f;")
            if s.scrollbar_thumb_color != (0.5, 0.5, 0.5, 0.6):
                tc = s.scrollbar_thumb_color
                lines.append(f"{prefix}.scrollbarThumbColor[0] = {tc[0]:.4f}f;")
                lines.append(f"{prefix}.scrollbarThumbColor[1] = {tc[1]:.4f}f;")
                lines.append(f"{prefix}.scrollbarThumbColor[2] = {tc[2]:.4f}f;")
                lines.append(f"{prefix}.scrollbarThumbColor[3] = {tc[3]:.4f}f;")
            if s.scrollbar_border_radius != 4.0:
                lines.append(f"{prefix}.scrollbarBorderRadius = {fmt(s.scrollbar_border_radius)};")

        # ── Feature: TRANSFORM ──
        if "transform" in self.features:
            if s.transform_matrix is not None:
                lines.extend(transform_assignments(prefix, s.transform_matrix))
            if s.transform_origin_resolved is not None \
                    and s.transform_origin_resolved != (0.5, 0.5):
                lines.extend(transform_origin_assignments(
                    prefix, s.transform_origin_resolved))

        return "\n".join(f"{indent}{l}" for l in lines)

    def _set_ancestor_hover_rules(self, node: IRNode, indent: str) -> str:
        return self._set_ancestor_state_rules(node, "ancestor_hover_rules",
                                              "m_ancestorHoverRules", indent)

    def _set_ancestor_active_rules(self, node: IRNode, indent: str) -> str:
        return self._set_ancestor_state_rules(node, "ancestor_active_rules",
                                              "m_ancestorActiveRules", indent)

    def _set_ancestor_state_rules(self, node: IRNode, attr: str, member: str, indent: str) -> str:
        rules = getattr(node, attr)
        if not rules:
            return ""
        lines = []
        for tag, rule_style in rules:
            var = f"_st_{node.node_id}_{member}_{tag}"
            lines.append(f"AncestorHoverRule {var};")
            lines.append(f"{var}.ancestorTag = \"{tag}\";")
            s = rule_style
            # Emit style fields (same pattern as _set_style but no inheritance)
            if s.bg_color != (0, 0, 0, 0):
                for i, ch in enumerate("rgba"):
                    lines.append(f"{var}.style.bgColor[{i}] = {s.bg_color[i]:.4f}f;")
            if s.color != (0, 0, 0, 1):
                for i, ch in enumerate("rgba"):
                    lines.append(f"{var}.style.color[{i}] = {s.color[i]:.4f}f;")
            if s.border_radius > 0:
                lines.append(f"{var}.style.borderRadius = {fmt(s.border_radius)};")
            if s.padding != (0, 0, 0, 0):
                for i in range(4):
                    lines.append(f"{var}.style.padding[{i}] = {fmt(s.padding[i])};")
            if s.margin != (0, 0, 0, 0):
                for i in range(4):
                    if s.margin[i] != 0.0 or s.margin_auto[i]:
                        v = fmt(s.margin[i]) if s.margin[i] != float("inf") else "-1.0f"
                        lines.append(f"{var}.style.margin[{i}] = {v};")
                        if s.margin_auto[i]:
                            lines.append(f"{var}.style.marginAuto[{i}] = true;")
            if s.width is not None:
                lines.append(f"{var}.style.explicitWidth = {fmt(s.width)};")
            if s.height is not None:
                lines.append(f"{var}.style.explicitHeight = {fmt(s.height)};")
            if s.min_width is not None:
                lines.append(f"{var}.style.minWidth = {fmt(s.min_width)};")
            if s.max_width is not None:
                lines.append(f"{var}.style.maxWidth = {fmt(s.max_width)};")
            if s.min_height is not None:
                lines.append(f"{var}.style.minHeight = {fmt(s.min_height)};")
            if s.max_height is not None:
                lines.append(f"{var}.style.maxHeight = {fmt(s.max_height)};")
            if s.font_size != 16.0:
                lines.append(f"{var}.style.fontSize = {fmt(s.font_size)};")
            if s.font_weight != "normal":
                lines.append(f"{var}.style.fontWeight = \"{s.font_weight}\";")
            if s.text_align != "left":
                lines.append(f"{var}.style.textAlign = \"{s.text_align}\";")
            if s.display != "block":
                lines.append(f"{var}.style.display = \"{s.display}\";")
            if s.overflow != "visible":
                lines.append(f"{var}.style.overflow = \"{s.overflow}\";")
            if s.position != "static":
                lines.append(f"{var}.style.position = \"{s.position}\";")
            if s.box_sizing != "content-box":
                lines.append(f"{var}.style.boxSizing = \"{s.box_sizing}\";")
            if s.cursor != "default":
                lines.append(f"{var}.style.cursor = \"{s.cursor}\";")
            if s.border_width > 0:
                lines.append(f"{var}.style.borderWidth = {fmt(s.border_width)};")
            if s.border_color != (0.0, 0.0, 0.0, 1.0):
                for i in range(4):
                    lines.append(f"{var}.style.borderColor[{i}] = {s.border_color[i]:.4f}f;")
            if s.border_style not in ("", "none"):
                lines.append(f"{var}.style.borderStyle = \"{s.border_style}\";")
            if "zindex" in self.features and s.z_index is not None:
                lines.append(f"{var}.style.zIndex = {int(s.z_index)};")
                lines.append(f"{var}.style.zIndexSet = true;")
            if "opacity" in self.features and s.opacity != 1.0:
                lines.append(f"{var}.style.opacity = {fmt(s.opacity)};")
            if "flex" in self.features:
                if s.flex_dir != "row":
                    lines.append(f"{var}.style.flexDirection = \"{s.flex_dir}\";")
                if s.justify_content != "flex-start":
                    lines.append(f"{var}.style.justifyContent = \"{s.justify_content}\";")
                if s.align_items != "stretch":
                    lines.append(f"{var}.style.alignItems = \"{s.align_items}\";")
                if s.flex_wrap != "nowrap":
                    lines.append(f"{var}.style.flexWrap = \"{s.flex_wrap}\";")
                if s.flex_grow != 0.0:
                    lines.append(f"{var}.style.flexGrow = {fmt(s.flex_grow)};")
                if s.flex_shrink != 1.0:
                    lines.append(f"{var}.style.flexShrink = {fmt(s.flex_shrink)};")
            if "transform" in self.features:
                if s.transform_matrix is not None:
                    lines.extend(transform_assignments(f"{var}.style", s.transform_matrix))
                if s.transform_origin_resolved is not None \
                        and s.transform_origin_resolved != (0.5, 0.5):
                    lines.extend(transform_origin_assignments(
                        f"{var}.style", s.transform_origin_resolved))
            lines.append(f"{node.node_id}->{member}.push_back({var});")
        return "\n" + "\n".join(f"{indent}{l}" for l in lines)

    def _set_hover_style(self, node: IRNode, indent: str) -> str:
        return self._set_state_style(node, "hover_style", "hoverStyle", indent,
                                     alloc=not node.hover_animations)

    def _set_active_style(self, node: IRNode, indent: str) -> str:
        return self._set_state_style(node, "active_style", "activeStyle", indent)

    def _set_state_style(self, node: IRNode, attr: str, cpp_member: str,
                         indent: str, alloc: bool = True) -> str:
        s = getattr(node, attr)
        if s is None:
            return ""
        base = node.style
        hv = f"{node.node_id}->{cpp_member}"
        overrides = []

        # Colors
        if s.bg_color != (0, 0, 0, 0) and s.bg_color != base.bg_color:
            for i, ch in enumerate("rgba"):
                overrides.append(f"{hv}->bgColor[{i}] = {s.bg_color[i]:.4f}f;")
        if s.color != (0, 0, 0, 1) and s.color != base.color:
            for i, ch in enumerate("rgba"):
                overrides.append(f"{hv}->color[{i}]   = {s.color[i]:.4f}f;")
        if s.border_color != (0.0, 0.0, 0.0, 1.0) and s.border_color != base.border_color:
            for i, ch in enumerate("rgba"):
                overrides.append(f"{hv}->borderColor[{i}] = {s.border_color[i]:.4f}f;")

        # Dimensions
        if s.border_radius > 0 and s.border_radius != base.border_radius:
            overrides.append(f"{hv}->borderRadius = {fmt(s.border_radius)};")
        if s.border_width > 0 and s.border_width != base.border_width:
            overrides.append(f"{hv}->borderWidth = {fmt(s.border_width)};")
        if s.border_style not in ("", "none") and s.border_style != base.border_style:
            overrides.append(f"{hv}->borderStyle = \"{s.border_style}\";")

        # Spacing
        if s.padding != (0, 0, 0, 0) and s.padding != base.padding:
            for i in range(4):
                overrides.append(f"{hv}->padding[{i}] = {fmt(s.padding[i])};")
        if s.margin != (0, 0, 0, 0) and s.margin != base.margin:
            for i in range(4):
                v = s.margin[i]
                if v != 0.0 or s.margin_auto[i]:
                    coded = fmt(v) if v != float("inf") else "-1.0f"
                    overrides.append(f"{hv}->margin[{i}] = {coded};")
                    overrides.append(f"{hv}->marginAuto[{i}] = {'true' if s.margin_auto[i] else 'false'};")

        # Typography
        if s.font_size != 16.0 and s.font_size != base.font_size:
            overrides.append(f"{hv}->fontSize = {fmt(s.font_size)};")
        if s.font_weight != "normal" and s.font_weight != base.font_weight:
            overrides.append(f"{hv}->fontWeight = \"{s.font_weight}\";")

        # Sizing
        if s.width is not None and s.width != base.width:
            overrides.append(f"{hv}->explicitWidth = {fmt(s.width)};")
        if s.height is not None and s.height != base.height:
            overrides.append(f"{hv}->explicitHeight = {fmt(s.height)};")
        if s.min_width is not None and s.min_width != base.min_width:
            overrides.append(f"{hv}->minWidth = {fmt(s.min_width)};")
        if s.max_width is not None and s.max_width != base.max_width:
            overrides.append(f"{hv}->maxWidth = {fmt(s.max_width)};")
        if s.min_height is not None and s.min_height != base.min_height:
            overrides.append(f"{hv}->minHeight = {fmt(s.min_height)};")
        if s.max_height is not None and s.max_height != base.max_height:
            overrides.append(f"{hv}->maxHeight = {fmt(s.max_height)};")

        # Display / layout
        if s.display != "block" and s.display != base.display:
            overrides.append(f"{hv}->display = \"{s.display}\";")
        if s.overflow != "visible" and s.overflow != base.overflow:
            overrides.append(f"{hv}->overflow = \"{s.overflow}\";")
        if s.position != "static" and s.position != base.position:
            overrides.append(f"{hv}->position = \"{s.position}\";")
        if s.box_sizing != "content-box" and s.box_sizing != base.box_sizing:
            overrides.append(f"{hv}->boxSizing = \"{s.box_sizing}\";")

        # ── Feature: FLEX ──
        if "flex" in self.features:
            if s.flex_dir != "row" and s.flex_dir != base.flex_dir:
                overrides.append(f"{hv}->flexDirection = \"{s.flex_dir}\";")
            if s.gap > 0 and s.gap != base.gap:
                overrides.append(f"{hv}->gap = {fmt(s.gap)};")
            if s.justify_content != "flex-start" and s.justify_content != base.justify_content:
                overrides.append(f"{hv}->justifyContent = \"{s.justify_content}\";")
            if s.align_items != "stretch" and s.align_items != base.align_items:
                overrides.append(f"{hv}->alignItems = \"{s.align_items}\";")
            if s.flex_wrap != "nowrap" and s.flex_wrap != base.flex_wrap:
                overrides.append(f"{hv}->flexWrap = \"{s.flex_wrap}\";")

        # ── Feature: POSITION ──
        if "position" in self.features:
            if s.left is not None and s.left != base.left:
                overrides.append(f"{hv}->left = {fmt(s.left)};")
            if s.right is not None and s.right != base.right:
                overrides.append(f"{hv}->right = {fmt(s.right)};")
            if s.top is not None and s.top != base.top:
                overrides.append(f"{hv}->top = {fmt(s.top)};")
            if s.bottom is not None and s.bottom != base.bottom:
                overrides.append(f"{hv}->bottom = {fmt(s.bottom)};")

        # ── Feature: ZINDEX ──
        if "zindex" in self.features:
            if s.z_index is not None and s.z_index != base.z_index:
                overrides.append(f"{hv}->zIndex = {int(s.z_index)};")
                overrides.append(f"{hv}->zIndexSet = true;")

        # ── Feature: OPACITY ──
        if "opacity" in self.features:
            if s.opacity != 1.0 and s.opacity != base.opacity:
                overrides.append(f"{hv}->opacity = {fmt(s.opacity)};")

        # ── Feature: CURSOR ──
        if "cursor" in self.features:
            if s.cursor != "default" and s.cursor != base.cursor:
                overrides.append(f"{hv}->cursor = \"{s.cursor}\";")

        # ── Feature: TRANSFORM ──
        if "transform" in self.features:
            if s.transform_matrix is not None and s.transform_matrix != base.transform_matrix:
                overrides.extend(transform_assignments(hv, s.transform_matrix, ptr=True))
            if s.transform_origin_resolved is not None \
                    and s.transform_origin_resolved != base.transform_origin_resolved:
                overrides.extend(transform_origin_assignments(
                    hv, s.transform_origin_resolved, ptr=True))

        if not overrides:
            return ""
        lines = []
        if alloc:
            lines.append(f"{hv} = new MorphStyle(); // delta only")
        lines += [f"{indent}{l}" for l in overrides]
        return "\n" + "\n".join(lines)

    def _set_animations(self, node: IRNode, indent: str) -> str:
        """Emit `animation` configs (feature: animation) onto a node.

        Each animation is a CssAnimation aggregate:
        {name, duration, easing, delay, iterations, direction, fillMode, running}
        """
        if "animation" not in self.features or not node.animations:
            return ""
        lines = []
        prefix = f"{node.node_id}->style.animations"
        for a in node.animations:
            easing = _ANIM_EASING_TO_CPP.get(a.easing, "Easing::Linear")
            direction = _ANIM_DIRECTION_TO_CPP.get(a.direction,
                                                   "AnimDirection::Normal")
            fill = _ANIM_FILL_MODE_TO_CPP.get(a.fill_mode,
                                              "AnimFillMode::None")
            running = "true" if a.play_state == "running" else "false"
            iters = fmt(a.iterations) if a.iterations >= 0 else "-1.0f"
            lines.append(
                f"{prefix}.push_back(CssAnimation{{"
                f"\"{a.name}\", {fmt(a.duration)}, {easing}, {fmt(a.delay)}, "
                f"{iters}, {direction}, {fill}, {running}}});")
        return "\n".join(f"{indent}{l}" for l in lines)

    def _set_hover_animations(self, node: IRNode, indent: str) -> str:
        """Emit `:hover` animation configs onto the node's hoverStyle delta.

        The runtime swaps ``style.animations`` ↔ ``hoverStyle->animations`` on
        hover enter/leave, so the animation only runs while hovered.
        """
        if "animation" not in self.features or not node.hover_animations:
            return ""
        lines = [f"{node.node_id}->hoverStyle = new MorphStyle(); // delta only"]
        prefix = f"{node.node_id}->hoverStyle->animations"
        for a in node.hover_animations:
            easing = _ANIM_EASING_TO_CPP.get(a.easing, "Easing::Linear")
            direction = _ANIM_DIRECTION_TO_CPP.get(a.direction,
                                                   "AnimDirection::Normal")
            fill = _ANIM_FILL_MODE_TO_CPP.get(a.fill_mode,
                                              "AnimFillMode::None")
            running = "true" if a.play_state == "running" else "false"
            iters = fmt(a.iterations) if a.iterations >= 0 else "-1.0f"
            lines.append(
                f"{prefix}.push_back(CssAnimation{{"
                f"\"{a.name}\", {fmt(a.duration)}, {easing}, {fmt(a.delay)}, "
                f"{iters}, {direction}, {fill}, {running}}});")
        return "\n".join(f"{indent}{l}" for l in lines)

    def _set_transition(self, node: IRNode, indent: str) -> str:
        if node.transition_duration <= 0:
            return ""
        dur = fmt(node.transition_duration)
        easing_map = {
            "linear": "Easing::Linear",
            "ease-in": "Easing::EaseIn",
            "ease-out": "Easing::EaseOut",
            "ease-in-out": "Easing::EaseInOut",
        }
        easing = easing_map.get(node.transition_easing, "Easing::EaseInOut")
        return f"{indent}{node.node_id}->m_transitionDuration = {dur};\n" \
               f"{indent}{node.node_id}->m_transitionEasing = {easing};"

    _EVENT_MEMBER = {
        "click": "onClick",
        "keyup": "onKeyUp",
        "keydown": "onKeyDown",
        "dblclick":   "onDoubleClick",
        "mousedown":  "onMouseDown",
        "mouseup":    "onMouseUp",
        "mouseenter": "onMouseEnter",
        "mouseleave": "onMouseLeave",
        "change":     "onChange",
        "input":      "onInput",
        "focus":      "onFocus",
        "blur":       "onBlur",
    }

    def _emit_event(self, event, node_id: str, indent: str) -> str:
        from morph.codegen.event_emitter import emit_event
        member = self._EVENT_MEMBER.get(event.trigger, "onClick")
        rhs = emit_event(event, node_id)
        return f"{indent}{node_id}->{member} = {rhs};"

    def _emit_viewport(self, node: IRViewport, parent_id: str | None) -> str:
        lines = [
            f"ViewportNode* {node.viewport_id} = new ViewportNode("
            f"    new {node.driver_class}()"
            f");",
        ]
        if parent_id:
            lines.append(f"{parent_id}->addChild({node.viewport_id});")
        return "\n".join(lines)
