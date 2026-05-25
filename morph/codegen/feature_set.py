from morph.ir.node import IRNode, IRViewport, IRWindow


class FeatureSet:
    """Scans IR and decides which C++ headers + feature defines to emit."""

    def __init__(self):
        self.features: set[str] = set()

    def scan(self, windows: list[IRWindow]) -> None:
        for win in windows:
            for node in self._walk(win.nodes):
                s = node.style
                if node.node_type == "__text__":
                    self.features.add("text")
                if node.node_type == "button":
                    self.features.add("button")
                    self.features.add("radius")
                if node.node_type == "input":
                    self.features.add("input")
                if s.border_radius > 0:
                    self.features.add("radius")
                if s.font_weight not in ("normal", ""):
                    self.features.add("bold")
                if s.overflow in ("auto", "scroll"):
                    self.features.add("scroll")
                # Detect scrollbar customization as scroll feature
                if (s.scrollbar_width != 8.0 or
                    s.scrollbar_track_color != (0.85, 0.85, 0.85, 0.4) or
                    s.scrollbar_thumb_color != (0.5, 0.5, 0.5, 0.6) or
                    s.scrollbar_border_radius != 4.0):
                    self.features.add("scroll")
                if s.position != "static":
                    self.features.add("position")
                # Detect position offsets as position feature even if position is static
                if s.left is not None or s.right is not None or s.top is not None or s.bottom is not None:
                    self.features.add("position")
                # ── Layout feature detection ──
                if s.display == "none":
                    self.features.add("display_none")
                if s.display == "inline":
                    self.features.add("inline")
                if any(m != 0 for m in s.margin):
                    self.features.add("margin_collapse")
                if (s.min_width is not None or s.max_width is not None or
                    s.min_height is not None or s.max_height is not None):
                    self.features.add("min_max")
                if s.box_sizing != "content-box":
                    self.features.add("border_box")

                if s.display == "flex":
                    self.features.add("flex")
                # Detect gap as flex feature (meaningful only with flex layout)
                if s.gap > 0:
                    self.features.add("flex")
                # Flex-related fields imply flex feature
                if s.justify_content != "flex-start" or s.align_items != "stretch" or s.flex_wrap != "nowrap":
                    self.features.add("flex")
                if s.cursor not in ("default", "", None):
                    self.features.add("cursor")
                if s.border_width > 0 or s.border_style not in ("", "none"):
                    self.features.add("border")
                if node.events:
                    self.features.add("event")
                if isinstance(node, IRViewport):
                    self.features.add("viewport")

    def required_headers(self) -> list[str]:
        headers: list[str] = ["widgets/morph_rect.h"]
        if "text" in self.features:
            headers.append("widgets/morph_text.h")
        if "button" in self.features:
            headers.append("widgets/morph_button.h")
        if "input" in self.features:
            headers.append("widgets/morph_input.h")
        if "event" in self.features:
            headers.append("core/event.h")
        if "viewport" in self.features:
            headers.append("viewport/viewport_node.h")
        return headers

    def required_defines(self) -> list[str]:
        defines: list[str] = []
        if "scroll" in self.features:
            defines.append("MORPH_FEATURE_SCROLL")
        if "radius" in self.features:
            defines.append("MORPH_FEATURE_RADIUS")
        if "text" in self.features:
            defines.append("MORPH_FEATURE_TEXT")
        if "bold" in self.features:
            defines.append("MORPH_FEATURE_BOLD")
        if "position" in self.features:
            defines.append("MORPH_FEATURE_POSITION")
        if "flex" in self.features:
            defines.append("MORPH_FEATURE_FLEX")
        if "cursor" in self.features:
            defines.append("MORPH_FEATURE_CURSOR")
        if "border" in self.features:
            defines.append("MORPH_FEATURE_BORDER")
        # ── Layout feature defines ──
        if "display_none" in self.features:
            defines.append("MORPH_FEATURE_DISPLAY_NONE")
        if "inline" in self.features:
            defines.append("MORPH_FEATURE_INLINE")
        if "margin_collapse" in self.features:
            defines.append("MORPH_FEATURE_MARGIN_COLLAPSE")
        if "min_max" in self.features:
            defines.append("MORPH_FEATURE_MIN_MAX")
        if "border_box" in self.features:
            defines.append("MORPH_FEATURE_BORDER_BOX")
        return defines

    def needs_freetype(self) -> bool:
        return "text" in self.features

    def _walk(self, nodes):
        for node in nodes:
            yield node
            yield from self._walk(node.children)
