import json
import math
from morph.ir.node import IRWindow, IRNode
from morph.ir.style import IRStyle


def _clean_inf(val):
    """Replace float('inf') / NaN with None recursively."""
    if isinstance(val, float) and (math.isinf(val) or math.isnan(val)):
        return None
    if isinstance(val, dict):
        return {k: _clean_inf(v) for k, v in val.items()}
    if isinstance(val, (list, tuple)):
        return [_clean_inf(v) for v in val]
    return val


class IRSerializer:
    """Serializes IR trees to JSON for the dev socket."""

    def to_dict(self, windows: list[IRWindow]) -> dict:
        return {
            "type": "app",
            "windows": [self._window(w) for w in windows],
        }

    def to_json(self, windows: list[IRWindow]) -> str:
        raw = self.to_dict(windows)
        return json.dumps(_clean_inf(raw), ensure_ascii=False)

    def _window(self, w: IRWindow) -> dict:
        return {
            "id":           w.window_id,
            "title":        w.title,
            "width":        w.width,
            "height":       w.height,
            "visible":      w.visible,
            "renderer":     w.renderer,
            "nodes":        [self._node(n) for n in w.nodes],
            "startup_logs": w.startup_logs,
            "premain_functions": w.premain_functions,
            "extra_headers": w.extra_headers,
            "state_vars":   w.state_vars,
            "effect_decls": w.effect_decls,
        }

    @staticmethod
    def _style_dict(s: IRStyle) -> dict:
        return {
            "bg_color":      list(s.bg_color),
            "color":         list(s.color),
            "width":         s.width,
            "min_width":     s.min_width,
            "max_width":     s.max_width,
            "height":        s.height,
            "min_height":    s.min_height,
            "max_height":    s.max_height,
            "margin":        list(s.margin),
            "margin_auto":   list(s.margin_auto),
            "padding":       list(s.padding),
            "border_radius": s.border_radius,
            "font_size":     s.font_size,
            "font_weight":   s.font_weight,
            "text_align":    s.text_align,
            "display":       s.display,
            "flex_dir":      s.flex_dir,
            "flex_grow":     s.flex_grow,
            "flex_shrink":   s.flex_shrink,
            "flex_basis":    s.flex_basis,
            "gap":                    s.gap,
            "overflow":               s.overflow,
            "position":               s.position,
            "left":                   s.left,
            "right":                  s.right,
            "top":                    s.top,
            "bottom":                 s.bottom,
            "justify_content":        s.justify_content,
            "align_items":            s.align_items,
            "flex_wrap":              s.flex_wrap,
            "cursor":                 s.cursor,
            "scrollbar_width":         s.scrollbar_width,
            "scrollbar_track_color":   list(s.scrollbar_track_color),
            "scrollbar_thumb_color":   list(s.scrollbar_thumb_color),
            "scrollbar_border_radius": s.scrollbar_border_radius,
            "border_width":            s.border_width,
            "border_color":            list(s.border_color),
            "border_style":            s.border_style,
            "box_sizing":              s.box_sizing,
            "z_index":                 s.z_index,
        }

    def _node(self, n) -> dict:
        s = n.style
        result = {
            "id":      n.node_id,
            "type":    n.node_type,
            "x": n.x, "y": n.y,
            "w": n.w, "h": n.h,
            "text":    n.text_content,
            "attrs":   n.attrs,
            "style": self._style_dict(s),
            "raw_styles": n.raw_styles,
            "children": [self._node(c) for c in n.children],
            "events":   [{"trigger": e.trigger, "action": e.action,
                          "target": e.target} for e in n.events],
        }
        if n.reactive_text:
            result["reactive_text"] = n.reactive_text
        if n.reactive_class:
            result["reactive_class"] = n.reactive_class
        if n.reactive_style:
            result["reactive_style"] = n.reactive_style
        if n.class_conditional_effects:
            result["class_conditional_effects"] = [
                [cond, on, off] for cond, on, off in n.class_conditional_effects
            ]
        if n.condition_expr:
            result["condition_expr"] = n.condition_expr
            result["then_nodes"] = [self._node(tn) for tn in n.then_nodes]
            result["else_nodes"] = [self._node(en) for en in n.else_nodes]
        if n.hover_style is not None:
            result["hover_style"] = self._style_dict(n.hover_style)
        if n.active_style is not None:
            result["active_style"] = self._style_dict(n.active_style)
        if n.transition_duration > 0:
            result["transition_duration"] = n.transition_duration
            result["transition_easing"] = n.transition_easing
        if n.ancestor_hover_rules:
            result["ancestor_hover_rules"] = [
                {"ancestor_tag": tag, "style": self._style_dict(s)}
                for tag, s in n.ancestor_hover_rules
            ]
        if n.ancestor_active_rules:
            result["ancestor_active_rules"] = [
                {"ancestor_tag": tag, "style": self._style_dict(s)}
                for tag, s in n.ancestor_active_rules
            ]
        return result
