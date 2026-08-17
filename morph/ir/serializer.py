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
            "keyframes":    self._keyframes_dict(w.keyframes),
        }

    @staticmethod
    def _keyframes_dict(keyframes: dict[str, list]) -> dict:
        """Serialize the global @keyframes registry.

        Each entry is {"offset": float, "style": style_dict, "raw": {...}}
        with a *partial* style (only the fields the keyframe declares).
        C++ samples and interpolates these per frame.
        """
        return {
            name: [
                {
                    "offset": kf.offset,
                    "style": IRSerializer._keyframe_style_dict(kf),
                    "raw": kf.raw,
                }
                for kf in kfs
            ]
            for name, kfs in keyframes.items()
        }

    @staticmethod
    def _keyframe_style_dict(kf) -> dict:
        """Partial style dict — only fields the keyframe explicitly declares.

        Default-compare heuristics would drop legitimate declarations like
        `opacity: 1` or `background-color: #000`; `declared` is set by the
        IR builder, so presence in the JSON always means "declared".
        """
        full = IRSerializer._style_dict(kf.style)
        declared = getattr(kf, "declared", None)
        if declared:
            return {k: v for k, v in full.items() if k in declared}
        # Fallback for hand-built keyframes without `declared`: keep only
        # fields that differ from the style defaults.
        s = kf.style
        keep = set()
        if s.opacity != 1.0:
            keep.add("opacity")
        if s.bg_color != (0, 0, 0, 0):
            keep.add("bg_color")
        if s.color != (0, 0, 0, 1):
            keep.add("color")
        if s.border_radius != 0.0:
            keep.add("border_radius")
        if s.font_size != 16.0:
            keep.add("font_size")
        for f in ("width", "height", "left", "top"):
            if getattr(s, f) is not None:
                keep.add(f)
        return {k: v for k, v in full.items() if k in keep}

    @staticmethod
    def _animations_dict(anims) -> list[dict]:
        return [
            {
                "name": a.name,
                "duration": a.duration,
                "easing": a.easing,
                "delay": a.delay,
                "iterations": a.iterations,
                "direction": a.direction,
                "fill_mode": a.fill_mode,
                "play_state": a.play_state,
            }
            for a in anims
        ]

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
            "opacity":                 s.opacity,
            "transform_ops":           list(s.transform_ops) if s.transform_ops else None,
            "transform_matrix":        list(s.transform_matrix) if s.transform_matrix else None,
            "transform_origin":        list(s.transform_origin_resolved) if s.transform_origin_resolved else None,
            "transform_origin_raw":    list(s.transform_origin) if s.transform_origin else None,
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
        if n.animations:
            result["animations"] = self._animations_dict(n.animations)
        if n.hover_animations:
            result["hover_animations"] = self._animations_dict(n.hover_animations)
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
