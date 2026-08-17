from morph.ir.node import IRNode, IRViewport, IRWindow


class FeatureSet:
    """Scans IR and decides which C++ headers + feature defines to emit."""

    def __init__(self):
        self.features: set[str] = set()

    def _scan_style(self, s) -> None:
        if s.border_radius is not None and s.border_radius > 0:
            self.features.add("radius")
        if s.font_weight not in ("normal", ""):
            self.features.add("bold")
        if s.overflow in ("auto", "scroll"):
            self.features.add("scroll")
        if (s.scrollbar_width != 8.0 or
            s.scrollbar_track_color != (0.85, 0.85, 0.85, 0.4) or
            s.scrollbar_thumb_color != (0.5, 0.5, 0.5, 0.6) or
            s.scrollbar_border_radius is not None and s.scrollbar_border_radius != 4.0):
            self.features.add("scroll")
        if s.position != "static":
            self.features.add("position")
        if s.left is not None or s.right is not None or s.top is not None or s.bottom is not None:
            self.features.add("position")
        if s.z_index is not None:
            self.features.add("zindex")
        if s.opacity != 1.0:
            self.features.add("opacity")
        if s.display == "none":
            self.features.add("display_none")
        if s.display in ("inline", "inline-block"):
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
        if s.gap is not None and s.gap > 0:
            self.features.add("flex")
        if (s.justify_content != "flex-start" or s.align_items != "stretch" or
            s.flex_wrap != "nowrap" or
            s.flex_grow != 0.0 or s.flex_shrink != 1.0 or s.flex_basis != "auto"):
            self.features.add("flex")
        if s.cursor not in ("default", "", None):
            self.features.add("cursor")
        if (s.border_width is not None and s.border_width > 0) or s.border_style not in ("", "none"):
            self.features.add("border")
        if s.transform_ops or s.transform_origin:
            self.features.add("transform")

    # CSS property (from reactive inline styles) → feature(s) it needs. These
    # matter because feature-gated style fields (zIndex, position offsets,
    # cursor, border, scrollbar, flex) live behind #ifdef MORPH_FEATURE_* — a
    # reactive assignment must enable them or the generated C++ won't compile.
    _REACTIVE_CSS_TO_FEATURE: dict[str, tuple[str, ...]] = {
        "z-index": ("zindex",),
        "opacity": ("opacity",),
        "position": ("position",),
        "left": ("position",), "right": ("position",),
        "top": ("position",), "bottom": ("position",),
        "cursor": ("cursor",),
        "border-width": ("border",), "border-style": ("border",),
        "border-color": ("border",),
        "scrollbar-width": ("scroll",),
        "scrollbar-track-color": ("scroll",),
        "scrollbar-thumb-color": ("scroll",),
        "scrollbar-border-radius": ("scroll",),
        "flex-direction": ("flex",), "flex-wrap": ("flex",),
        "flex-basis": ("flex",), "flex-grow": ("flex",),
        "flex-shrink": ("flex",),
        "justify-content": ("flex",), "align-items": ("flex",),
        "gap": ("flex",),
        "overflow": ("scroll",),
        "display": ("flex", "display_none", "inline"),
        "font-weight": ("bold",),
        "border-radius": ("radius",),
        "min-width": ("min_max",), "max-width": ("min_max",),
        "min-height": ("min_max",), "max-height": ("min_max",),
        "box-sizing": ("border_box",),
        "margin": ("margin_collapse",),
        "transform": ("transform",),
        # CSS animations need the runtime animation driver + keyframe engine.
        "animation": ("animation",),
        "animation-name": ("animation",), "animation-duration": ("animation",),
        "animation-timing-function": ("animation",), "animation-delay": ("animation",),
        "animation-iteration-count": ("animation",), "animation-direction": ("animation",),
        "animation-fill-mode": ("animation",), "animation-play-state": ("animation",),
    }

    def _scan_reactive(self, reactive_style: dict[str, str]) -> None:
        for css_prop in reactive_style:
            for feature in self._REACTIVE_CSS_TO_FEATURE.get(css_prop, ()):
                self.features.add(feature)

    def scan(self, windows: list[IRWindow]) -> None:
        for win in windows:
            if getattr(win, "renderer", "flash") == "forge":
                self.features.add("forge")
            # Keyframes may animate features the base styles never touch
            # (opacity, position offsets, transform) — enable them here so
            # the generated code compiles against the feature-gated fields.
            for kfs in getattr(win, "keyframes", {}).values():
                for kf in kfs:
                    self._scan_style(kf.style)
                    if "opacity" in kf.raw:
                        self.features.add("opacity")
                    if "transform" in kf.raw:
                        self.features.add("transform")
                    if "left" in kf.raw or "top" in kf.raw:
                        self.features.add("position")
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
                if node.active_style is not None:
                    self.features.add("active")
                    self._scan_style(node.active_style)
                if node.events:
                    self.features.add("event")
                if node.animations or node.hover_animations:
                    # CSS `animation` + @keyframes driver (dead-code eliminated
                    # from prod builds that never use animations).
                    self.features.add("animation")
                if isinstance(node, IRViewport):
                    self.features.add("viewport")
                if node.reactive_style:
                    self._scan_reactive(node.reactive_style)

        # Dirty rendering: enable if any dynamic behavior detected
        if any(f in self.features for f in ["scroll", "event", "cursor", "animation", "hover", "active"]):
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
        if "zindex" in self.features:
            defines.append("MORPH_FEATURE_ZINDEX")
        if "opacity" in self.features:
            defines.append("MORPH_FEATURE_OPACITY")
        if "flex" in self.features:
            defines.append("MORPH_FEATURE_FLEX")
        if "cursor" in self.features:
            defines.append("MORPH_FEATURE_CURSOR")
        if "border" in self.features:
            defines.append("MORPH_FEATURE_BORDER")
        if "transform" in self.features:
            defines.append("MORPH_FEATURE_TRANSFORM")
        if "animation" in self.features:
            defines.append("MORPH_FEATURE_ANIMATION")
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
        # ── Renderer selection defines ──
        # Default to flash; forge is explicitly enabled via "forge" key in config
        if "forge" in self.features:
            defines.append("MORPH_RENDERER_FORGE")
        return defines

    def needs_freetype(self) -> bool:
        return "text" in self.features

    def _walk(self, nodes):
        for node in nodes:
            yield node
            yield from self._walk(node.children)
            yield from self._walk(node.then_nodes)
            yield from self._walk(node.else_nodes)
