import json
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
            "id":           w.window_id,
            "title":        w.title,
            "width":        w.width,
            "height":       w.height,
            "visible":      w.visible,
            "nodes":        [self._node(n) for n in w.nodes],
            "startup_logs": w.startup_logs,
        }

    def _node(self, n) -> dict:
        s = n.style
        return {
            "id":      n.node_id,
            "type":    n.node_type,
            "x": n.x, "y": n.y,
            "w": n.w, "h": n.h,
            "text":    n.text_content,
            "style": {
                "bg_color":      list(s.bg_color),
                "color":         list(s.color),
                "width":         s.width,
                "height":        s.height,
                "margin":        list(s.margin),
                "padding":       list(s.padding),
                "border_radius": s.border_radius,
                "font_size":     s.font_size,
                "font_weight":   s.font_weight,
                "text_align":    s.text_align,
                "display":       s.display,
                "flex_dir":      s.flex_dir,
                "flex":          s.flex,
                "gap":                    s.gap,
                "overflow":               s.overflow,
                "scrollbar_width":         s.scrollbar_width,
                "scrollbar_track_color":   list(s.scrollbar_track_color),
                "scrollbar_thumb_color":   list(s.scrollbar_thumb_color),
                "scrollbar_border_radius": s.scrollbar_border_radius,
            },
            "children": [self._node(c) for c in n.children],
            "events":   [{"trigger": e.trigger, "action": e.action,
                          "target": e.target} for e in n.events],
        }
