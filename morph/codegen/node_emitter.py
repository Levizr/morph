from __future__ import annotations

import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRNode, IRViewport
from morph.ir.style import IRStyle

_TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), "templates")


def fmt(v: float) -> str:
    """Format a float for C++ literal — always includes a decimal point."""
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
            if parent_id:
                lines.append(f"{parent_id}->addChild({node.node_id});")
            return "\n".join(lines)

        if node.node_type == "button":
            lines.append(f"ButtonNode* {node.node_id} = new ButtonNode();")
            lines.append(f"{indent}{node.node_id}->x = {fmt(node.x)};")
            lines.append(f"{indent}{node.node_id}->y = {fmt(node.y)};")
            lines.append(f"{indent}{node.node_id}->w = {fmt(node.w)};")
            lines.append(f"{indent}{node.node_id}->h = {fmt(node.h)};")
        else:
            lines.append(f"RectNode* {node.node_id} = new RectNode("
                         f"{fmt(node.x)}, {fmt(node.y)}, {fmt(node.w)}, {fmt(node.h)});")

        lines.append(self._set_style(node, indent, parent_style))

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
        c = s.color if s.color != (0, 0, 0, 1) else (
            parent_style.color if parent_style and parent_style.color != (0, 0, 0, 1) else (0, 0, 0, 1))
        if c != (0, 0, 0, 1):
            lines.append(f"{prefix}.color[0]   = {c[0]:.4f}f;")
            lines.append(f"{prefix}.color[1]   = {c[1]:.4f}f;")
            lines.append(f"{prefix}.color[2]   = {c[2]:.4f}f;")
        if s.padding != (0, 0, 0, 0):
            lines.append(f"{prefix}.padding[0] = {fmt(s.padding[0])};")
            lines.append(f"{prefix}.padding[1] = {fmt(s.padding[1])};")
            lines.append(f"{prefix}.padding[2] = {fmt(s.padding[2])};")
            lines.append(f"{prefix}.padding[3] = {fmt(s.padding[3])};")
        if s.margin != (0, 0, 0, 0):
            lines.append(f"{prefix}.margin[0] = {fmt(s.margin[0])};")
            lines.append(f"{prefix}.margin[1] = {fmt(s.margin[1])};")
            lines.append(f"{prefix}.margin[2] = {fmt(s.margin[2])};")
            lines.append(f"{prefix}.margin[3] = {fmt(s.margin[3])};")
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

        return "\n".join(f"{indent}{l}" for l in lines)

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
