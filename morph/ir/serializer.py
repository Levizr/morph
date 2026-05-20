import json
from dataclasses import asdict
from morph.ir.node import IRWindow


class IRSerializer:
    """Serializes IR trees to JSON for the dev socket."""

    def to_dict(self, windows: list[IRWindow]) -> dict:
        return {
            "type": "app",
            "windows": [self._window(w) for w in windows],
        }

    def to_json(self, windows: list[IRWindow]) -> str:
        return json.dumps(self.to_dict(windows))

    def _window(self, w: IRWindow) -> dict:
        return {
            "id":      w.window_id,
            "title":   w.title,
            "width":   w.width,
            "height":  w.height,
            "visible": w.visible,
            "nodes":   [self._node(n) for n in w.nodes],
        }

    def _node(self, n) -> dict:
        return {
            "id":           n.node_id,
            "type":         n.node_type,
            "x": n.x, "y": n.y,
            "w": n.w, "h": n.h,
            "text":         n.text_content,
            "style":        {
                "bg_color":      list(n.style.bg_color),
                "color":         list(n.style.color),
                "border_radius": n.style.border_radius,
            },
            "children":     [self._node(c) for c in n.children],
            "events":       [{"trigger": e.trigger, "action": e.action,
                              "target": e.target} for e in n.events],
        }
