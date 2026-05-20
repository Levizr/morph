from morph.ir.node import IRNode, IRViewport, IRWindow


class FeatureSet:
    """Scans IR and decides which C++ headers to include."""

    def __init__(self):
        self.features: set[str] = set()

    def scan(self, windows: list[IRWindow]) -> None:
        for win in windows:
            for node in self._walk(win.nodes):
                if node.style.border_radius > 0:
                    self.features.add("radius")
                if node.node_type == "button":
                    self.features.add("button")
                if node.node_type == "input":
                    self.features.add("input")
                if node.events:
                    self.features.add("event")
                if isinstance(node, IRViewport):
                    self.features.add("viewport")

    def required_headers(self) -> list[str]:
        base = ["widgets/morph_rect.h", "widgets/morph_text.h"]
        mapping = {
            "radius":   "widgets/morph_radius.h",
            "button":   "widgets/morph_button.h",
            "input":    "widgets/morph_input.h",
            "event":    "core/event.h",
            "viewport": "viewport/viewport_node.h",
        }
        return base + [mapping[f] for f in self.features if f in mapping]

    def _walk(self, nodes):
        for node in nodes:
            yield node
            yield from self._walk(node.children)
