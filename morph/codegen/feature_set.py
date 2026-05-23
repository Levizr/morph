from morph.ir.node import IRNode, IRViewport, IRWindow


class FeatureSet:
    """Scans IR and decides which C++ headers + feature defines to emit."""

    def __init__(self):
        self.features: set[str] = set()

    def scan(self, windows: list[IRWindow]) -> None:
        for win in windows:
            for node in self._walk(win.nodes):
                if node.node_type == "__text__":
                    self.features.add("text")
                if node.node_type == "button":
                    self.features.add("button")
                if node.node_type == "input":
                    self.features.add("input")
                if node.style.border_radius > 0:
                    self.features.add("radius")
                if node.style.font_weight not in ("normal", ""):
                    self.features.add("bold")
                if node.style.overflow in ("auto", "scroll"):
                    self.features.add("scroll")
                if node.style.position != "static":
                    self.features.add("position")
                if node.style.display == "flex":
                    self.features.add("flex")
                if node.style.cursor not in ("default", "", None):
                    self.features.add("cursor")
                if node.events:
                    self.features.add("event")
                if isinstance(node, IRViewport):
                    self.features.add("viewport")

    def required_headers(self) -> list[str]:
        headers: list[str] = []
        # MorphRect is always needed (every div/text container is a RectNode)
        headers.append("widgets/morph_rect.h")
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
        return defines

    def needs_freetype(self) -> bool:
        return "text" in self.features

    def _walk(self, nodes):
        for node in nodes:
            yield node
            yield from self._walk(node.children)
