from morph.ir.node import IRNode, IRViewport, IRWindow


class FeatureSet:
    """Scans IR and decides which C++ headers + feature defines to emit."""

    def __init__(self):
        self.features: set[str] = set()

    def _scan_style(self, s) -> None:
        if s.border_radius > 0:
            self.features.add("radius")
        if s.font_weight not in ("normal", ""):
            self.features.add("bold")
        if s.overflow in ("auto", "scroll"):
            self.features.add("scroll")
        if (s.scrollbar_width != 8.0 or
            s.scrollbar_track_color != (0.85, 0.85, 0.85, 0.4) or
            s.scrollbar_thumb_color != (0.5, 0.5, 0.5, 0.6) or
            s.scrollbar_border_radius != 4.0):
            self.features.add("scroll")
        if s.position != "static":
            self.features.add("position")
        if s.left is not None or s.right is not None or s.top is not None or s.bottom is not None:
            self.features.add("position")
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
        if s.gap > 0:
            self.features.add("flex")
        if (s.justify_content != "flex-start" or s.align_items != "stretch" or
            s.flex_wrap != "nowrap" or
            s.flex_grow != 0.0 or s.flex_shrink != 1.0 or s.flex_basis != "auto"):
            self.features.add("flex")
        if s.cursor not in ("default", "", None):
            self.features.add("cursor")
        if s.border_width > 0 or s.border_style not in ("", "none"):
            self.features.add("border")

    def scan(self, windows: list[IRWindow]) -> None:
        for win in windows:
            for node in self._walk(win.nodes):
                if node.node_type == "__text__":
                    self.features.add("text")
                if node.node_type == "button":
                    self.features.add("button")
                    self.features.add("radius")
                if node.node_type == "input":
                    self.features.add("input")
                if node.node_type == "img":
                    self.features.add("image")
                self._scan_style(node.style)
                if node.hover_style is not None:
                    self.features.add("hover")
                    self._scan_style(node.hover_style)
                if node.events:
                    self.features.add("event")
                if isinstance(node, IRViewport):
                    self.features.add("viewport")

        # Dirty rendering: enable if any dynamic behavior detected
        if any(f in self.features for f in ["scroll", "event", "cursor", "animation", "hover"]):
            self.features.add("dirty_rendering")

    def required_headers(self) -> list[str]:
        headers: list[str] = ["ui/rect.h"]
        if "text" in self.features:
            headers.append("ui/text.h")
        if "button" in self.features:
            headers.append("ui/button.h")
        if "input" in self.features:
            headers.append("ui/input.h")
        if "event" in self.features:
            headers.append("core/event.h")
        if "viewport" in self.features:
            headers.append("ui/viewport_node.h")
        if "image" in self.features:
            headers.append("ui/image.h")
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
        if "image" in self.features:
            defines.append("MORPH_FEATURE_IMAGE")
        if "dirty_rendering" in self.features or "scroll" in self.features:
            defines.append("MORPH_FEATURE_DIRTY_RENDERING")
        return defines

    def needs_freetype(self) -> bool:
        return "text" in self.features

    def _walk(self, nodes):
        for node in nodes:
            yield node
            yield from self._walk(node.children)
