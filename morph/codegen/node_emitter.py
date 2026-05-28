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
                  parent_style: IRStyle | None = None) -> str:
        """Return C++ code for a single node and all its children (recursive)."""
        if isinstance(node, IRViewport):
            return self._emit_viewport(node, parent_id)

        lines = []
        indent = "    "

        if node.node_type == "__text__":
            escaped = node.text_content.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n").replace("\t", "\\t").replace("\r", "\\r")
            lines.append(f"TextNode* {node.node_id} = new TextNode(\"{escaped}\");")
            lines.append(f"{indent}{node.node_id}->x = {fmt(node.x)};")
            lines.append(f"{indent}{node.node_id}->y = {fmt(node.y)};")
            lines.append(f"{indent}{node.node_id}->w = {fmt(node.w)};")
            lines.append(f"{indent}{node.node_id}->h = {fmt(node.h)};")
            lines.append(self._set_style(node, indent, parent_style))
            lines.append(self._set_hover_style(node, indent))
            if parent_id:
                lines.append(self._set_transition(node, indent))
            if parent_id:
                lines.append(f"{parent_id}->addChild({node.node_id});")
            return "\n".join(lines)

        if node.node_type == "button":
            lines.append(f"ButtonNode* {node.node_id} = new ButtonNode();")
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
        lines.append(self._set_hover_style(node, indent))
        lines.append(self._set_transition(node, indent))

        for event in node.events:
            lines.append(self._emit_event(event, node.node_id, indent))

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
            lines.append(self.emit_node(child, node.node_id, resolved))

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
            if s.display == "flex" and s.flex_dir != "column":
                lines.append(f"{prefix}.flexDirection = \"{s.flex_dir}\";")
            if s.gap > 0:
                lines.append(f"{prefix}.gap = {fmt(s.gap)};")
            if s.justify_content != "flex-start":
                lines.append(f"{prefix}.justifyContent = \"{s.justify_content}\";")
            if s.align_items != "stretch":
                lines.append(f"{prefix}.alignItems = \"{s.align_items}\";")
            if s.flex_wrap != "nowrap":
                lines.append(f"{prefix}.flexWrap = \"{s.flex_wrap}\";")

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

        lines.append(f"{node.node_id}->m_baseStyle = {prefix}; // snapshot for hover restore")
        return "\n".join(f"{indent}{l}" for l in lines)

    def _set_hover_style(self, node: IRNode, indent: str) -> str:
        s = node.hover_style
        if s is None:
            return ""
        base = node.style
        hv = f"{node.node_id}->hoverStyle"
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
            if s.flex_dir != "column" and s.flex_dir != base.flex_dir:
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

        # ── Feature: CURSOR ──
        if "cursor" in self.features:
            if s.cursor != "default" and s.cursor != base.cursor:
                overrides.append(f"{hv}->cursor = \"{s.cursor}\";")

        if not overrides:
            return ""
        lines = [f"{hv} = new MorphStyle({node.node_id}->style); // copy base, then override"]
        lines += [f"{indent}{l}" for l in overrides]
        return "\n" + "\n".join(lines)

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

    def _emit_event(self, event, node_id: str, indent: str) -> str:
        from morph.codegen.event_emitter import emit_event
        code = emit_event(event, node_id)
        return f"{indent}{node_id}->onClick = [&wm]() {{ {code} }};"

    def _emit_viewport(self, node: IRViewport, parent_id: str | None) -> str:
        lines = [
            f"ViewportNode* {node.viewport_id} = new ViewportNode("
            f"    new {node.driver_class}()"
            f");",
        ]
        if parent_id:
            lines.append(f"{parent_id}->addChild({node.viewport_id});")
        return "\n".join(lines)
