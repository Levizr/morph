from __future__ import annotations

import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRNode, IRViewport
from morph.ir.style import IRStyle

_TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), "templates")


class NodeEmitter:
    """Generates C++ instantiation code for an IR node tree."""

    def __init__(self):
        self.env = Environment(
            loader=FileSystemLoader(_TEMPLATES_DIR),
            trim_blocks=True,
            lstrip_blocks=True,
        )

    def emit_node(self, node: IRNode, parent_id: str | None = None,
                  parent_style: IRStyle | None = None) -> str:
        """Return C++ code for a single node and all its children (recursive)."""
        if isinstance(node, IRViewport):
            return self._emit_viewport(node, parent_id)

        lines = []
        indent = "    "

        # ── Text node ────────────────────────────────────────
        if node.node_type == "__text__":
            lines.append(f"TextNode* {node.node_id} = new TextNode(\"{node.text_content}\");")
            lines.append(f"{indent}{node.node_id}->x = {node.x}f;")
            lines.append(f"{indent}{node.node_id}->y = {node.y}f;")
            lines.append(f"{indent}{node.node_id}->w = {node.w}f;")
            lines.append(f"{indent}{node.node_id}->h = {node.h}f;")
            lines.append(self._set_style(node, indent, parent_style))
            if parent_id:
                lines.append(f"{parent_id}->addChild({node.node_id});")
            return "\n".join(lines)

        # ── Button (has click events) ────────────────────────
        if node.node_type == "button":
            lines.append(f"ButtonNode* {node.node_id} = new ButtonNode();")
            lines.append(f"{indent}{node.node_id}->x = {node.x}f;")
            lines.append(f"{indent}{node.node_id}->y = {node.y}f;")
            lines.append(f"{indent}{node.node_id}->w = {node.w}f;")
            lines.append(f"{indent}{node.node_id}->h = {node.h}f;")

        # ── Generic rectangular node ─────────────────────────
        else:
            lines.append(f"RectNode* {node.node_id} = new RectNode("
                         f"{node.x}f, {node.y}f, {node.w}f, {node.h}f);")

        lines.append(self._set_style(node, indent, parent_style))

        # ── Events ───────────────────────────────────────────
        for event in node.events:
            lines.append(self._emit_event(event, node.node_id, indent))

        lines.append("")

        # ── Attach to parent ─────────────────────────────────
        if parent_id:
            lines.append(f"{parent_id}->addChild({node.node_id});")

        # ── Build resolved style for child inheritance ───────
        s = node.style
        resolved = IRStyle(
            color=s.color if s.color != (0, 0, 0, 1) else (
                parent_style.color if parent_style and parent_style.color != (0, 0, 0, 1) else (0, 0, 0, 1)),
            font_size=s.font_size if s.font_size != 16.0 else (
                parent_style.font_size if parent_style and parent_style.font_size != 16.0 else 16.0),
            font_weight=s.font_weight if s.font_weight != "normal" else (
                parent_style.font_weight if parent_style and parent_style.font_weight != "normal" else "normal"),
        )

        # ── Recurse children ─────────────────────────────────
        for child in node.children:
            lines.append(self.emit_node(child, node.node_id, resolved))

        return "\n".join(lines)

    def _set_style(self, node: IRNode, indent: str,
                   parent_style: IRStyle | None = None) -> str:
        s = node.style
        lines = []
        prefix = f"{node.node_id}->style"
        if s.bg_color != (0, 0, 0, 0):
            lines.append(f"{prefix}.bgColor[0] = {s.bg_color[0]:.4f}f;")
            lines.append(f"{prefix}.bgColor[1] = {s.bg_color[1]:.4f}f;")
            lines.append(f"{prefix}.bgColor[2] = {s.bg_color[2]:.4f}f;")
            lines.append(f"{prefix}.bgColor[3] = {s.bg_color[3]:.4f}f;")
        if s.border_radius > 0:
            lines.append(f"{prefix}.borderRadius = {s.border_radius}f;")
        fs = s.font_size if s.font_size != 16.0 else (
            parent_style.font_size if parent_style and parent_style.font_size != 16.0 else 16.0)
        if fs != 16.0:
            lines.append(f"{prefix}.fontSize = {fs}f;")
        fw = s.font_weight if s.font_weight != "normal" else (
            parent_style.font_weight if parent_style and parent_style.font_weight != "normal" else "normal")
        if fw != "normal":
            lines.append(f"{prefix}.fontWeight = \"{fw}\";")
        # Color inherits from parent like browser cascade
        c = s.color if s.color != (0, 0, 0, 1) else (
            parent_style.color if parent_style and parent_style.color != (0, 0, 0, 1) else (0, 0, 0, 1))
        if c != (0, 0, 0, 1):
            lines.append(f"{prefix}.color[0]   = {c[0]:.4f}f;")
            lines.append(f"{prefix}.color[1]   = {c[1]:.4f}f;")
            lines.append(f"{prefix}.color[2]   = {c[2]:.4f}f;")
        if s.padding != (0, 0, 0, 0):
            lines.append(f"{prefix}.padding[0] = {s.padding[0]}f;")
            lines.append(f"{prefix}.padding[1] = {s.padding[1]}f;")
            lines.append(f"{prefix}.padding[2] = {s.padding[2]}f;")
            lines.append(f"{prefix}.padding[3] = {s.padding[3]}f;")
        if s.margin != (0, 0, 0, 0):
            lines.append(f"{prefix}.margin[0] = {s.margin[0]}f;")
            lines.append(f"{prefix}.margin[1] = {s.margin[1]}f;")
            lines.append(f"{prefix}.margin[2] = {s.margin[2]}f;")
            lines.append(f"{prefix}.margin[3] = {s.margin[3]}f;")
        if s.gap > 0:
            lines.append(f"{prefix}.gap = {s.gap}f;")
        if s.width is not None:
            lines.append(f"{prefix}.explicitWidth = {s.width}f;")
        if s.height is not None:
            lines.append(f"{prefix}.explicitHeight = {s.height}f;")
        if s.overflow != "visible":
            lines.append(f"{prefix}.overflow = \"{s.overflow}\";")
        if s.scrollbar_width != 8.0:
            lines.append(f"{prefix}.scrollbarWidth = {s.scrollbar_width}f;")
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
            lines.append(f"{prefix}.scrollbarBorderRadius = {s.scrollbar_border_radius}f;")
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
