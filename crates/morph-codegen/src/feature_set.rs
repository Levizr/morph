use std::collections::HashSet;
use morph_ir::{IRWindow, IRNode, IRStyle};

#[derive(Default)]
pub struct FeatureSet {
    pub features: HashSet<String>,
}

impl FeatureSet {
    pub fn new() -> Self { Self::default() }

    fn scan_style(&mut self, s: &IRStyle) {
        if s.border_radius > 0.0 { self.features.insert("radius".into()); }
        if s.font_weight != "normal" && !s.font_weight.is_empty() { self.features.insert("bold".into()); }
        if s.overflow == "auto" || s.overflow == "scroll" { self.features.insert("scroll".into()); }
        if s.scrollbar_width != 8.0
            || s.scrollbar_track_color != [0.85, 0.85, 0.85, 0.4]
            || s.scrollbar_thumb_color != [0.5, 0.5, 0.5, 0.6]
            || s.scrollbar_border_radius != 4.0 {
            self.features.insert("scroll".into());
        }
        if s.position != "static" { self.features.insert("position".into()); }
        if s.left.is_some() || s.right.is_some() || s.top.is_some() || s.bottom.is_some() {
            self.features.insert("position".into());
        }
        if s.z_index.is_some() { self.features.insert("zindex".into()); }
        if (s.opacity - 1.0).abs() > f32::EPSILON { self.features.insert("opacity".into()); }
        if s.display == "none" { self.features.insert("display_none".into()); }
        if s.display == "inline" || s.display == "inline-block" { self.features.insert("inline".into()); }
        if s.margin.iter().any(|&m| m != 0.0) { self.features.insert("margin_collapse".into()); }
        if s.min_width.is_some() || s.max_width.is_some() || s.min_height.is_some() || s.max_height.is_some() {
            self.features.insert("min_max".into());
        }
        if s.box_sizing != "content-box" { self.features.insert("border_box".into()); }
        if s.display == "flex" { self.features.insert("flex".into()); }
        if s.gap > 0.0 { self.features.insert("flex".into()); }
        if s.justify_content != "flex-start" || s.align_items != "stretch" || s.flex_wrap != "nowrap" || s.flex_grow != 0.0 || s.flex_shrink != 1.0 || s.flex_basis != "auto" {
            self.features.insert("flex".into());
        }
        if s.cursor != "default" && !s.cursor.is_empty() { self.features.insert("cursor".into()); }
        if s.border_width > 0.0 || (s.border_style != "" && s.border_style != "none") {
            self.features.insert("border".into());
        }
        if s.transform_ops.is_some() || s.transform_origin.is_some() {
            self.features.insert("transform".into());
        }
    }

    fn scan_reactive(&mut self, reactive_style: &std::collections::HashMap<String, String>) {
        for prop in reactive_style.keys() {
            for f in Self::reactive_feature(prop) {
                self.features.insert(f.into());
            }
        }
    }

    fn reactive_feature(prop: &str) -> Vec<&'static str> {
        match prop {
            "z-index" => vec!["zindex"],
            "opacity" => vec!["opacity"],
            "position" | "left" | "right" | "top" | "bottom" => vec!["position"],
            "cursor" => vec!["cursor"],
            "border-width" | "border-style" | "border-color" => vec!["border"],
            "scrollbar-width" | "scrollbar-track-color" | "scrollbar-thumb-color" | "scrollbar-border-radius" => vec!["scroll"],
            "flex-direction" | "flex-wrap" | "flex-basis" | "flex-grow" | "flex-shrink" | "justify-content" | "align-items" | "gap" => vec!["flex"],
            "overflow" => vec!["scroll"],
            "display" => vec!["flex", "display_none", "inline"],
            "font-weight" => vec!["bold"],
            "border-radius" => vec!["radius"],
            "min-width" | "max-width" | "min-height" | "max-height" => vec!["min_max"],
            "box-sizing" => vec!["border_box"],
            "margin" => vec!["margin_collapse"],
            "transform" => vec!["transform"],
            "animation" | "animation-name" | "animation-duration" => vec!["animation"],
            _ => vec![],
        }
    }

    pub fn scan(&mut self, windows: &[IRWindow]) {
        for win in windows {
            if win.renderer == "forge" { self.features.insert("forge".into()); }
            for kfs in win.keyframes.values() {
                for kf in kfs {
                    self.scan_style(&kf.style);
                }
            }
            for node in Self::walk(&win.nodes) {
                if node.node_type == "__text__" { self.features.insert("text".into()); }
                if node.node_type == "button" {
                    self.features.insert("button".into());
                    self.features.insert("radius".into());
                }
                if node.node_type == "input" {
                    self.features.insert("input".into());
                    self.features.insert("text".into());
                    self.features.insert("cursor".into());
                    self.features.insert("radius".into());
                    self.features.insert("event".into());
                }
                if node.node_type == "img" { self.features.insert("image".into()); }
                if node.node_type == "__list__" { self.features.insert("list".into()); }
                if node.item_template.is_some() { self.features.insert("list".into()); }
                self.scan_style(&node.style);
                if let Some(ref hs) = node.hover_style { self.features.insert("hover".into()); self.scan_style(hs); }
                if let Some(ref hs) = node.active_style { self.features.insert("active".into()); self.scan_style(hs); }
                if !node.events.is_empty() { self.features.insert("event".into()); }
                if !node.animations.is_empty() || !node.hover_animations.is_empty() { self.features.insert("animation".into()); }
                if !node.reactive_style.is_empty() { self.scan_reactive(&node.reactive_style); }
            }
        }
        if ["scroll","event","cursor","animation","hover","active"].iter().any(|f| self.features.contains(*f)) {
            self.features.insert("dirty_rendering".into());
        }
    }

    pub fn required_headers(&self) -> Vec<String> {
        let mut h = vec!["ui/rect.h".to_string()];
        if self.features.contains("text") { h.push("ui/text.h".into()); }
        if self.features.contains("button") { h.push("ui/button.h".into()); }
        if self.features.contains("input") { h.push("ui/input.h".into()); }
        if self.features.contains("event") { h.push("core/event.h".into()); }
        if self.features.contains("image") { h.push("ui/image.h".into()); }
        if self.features.contains("list") { h.push("ui/morph_list.h".into()); }
        h
    }

    pub fn required_defines(&self) -> Vec<String> {
        let mut d = Vec::new();
        if self.features.contains("scroll") { d.push("MORPH_FEATURE_SCROLL".into()); }
        if self.features.contains("radius") { d.push("MORPH_FEATURE_RADIUS".into()); }
        if self.features.contains("text") { d.push("MORPH_FEATURE_TEXT".into()); }
        if self.features.contains("bold") { d.push("MORPH_FEATURE_BOLD".into()); }
        if self.features.contains("position") { d.push("MORPH_FEATURE_POSITION".into()); }
        if self.features.contains("zindex") { d.push("MORPH_FEATURE_ZINDEX".into()); }
        if self.features.contains("opacity") { d.push("MORPH_FEATURE_OPACITY".into()); }
        if self.features.contains("flex") { d.push("MORPH_FEATURE_FLEX".into()); }
        if self.features.contains("cursor") { d.push("MORPH_FEATURE_CURSOR".into()); }
        if self.features.contains("border") { d.push("MORPH_FEATURE_BORDER".into()); }
        if self.features.contains("transform") { d.push("MORPH_FEATURE_TRANSFORM".into()); }
        if self.features.contains("animation") { d.push("MORPH_FEATURE_ANIMATION".into()); }
        if self.features.contains("display_none") { d.push("MORPH_FEATURE_DISPLAY_NONE".into()); }
        if self.features.contains("inline") { d.push("MORPH_FEATURE_INLINE".into()); }
        if self.features.contains("margin_collapse") { d.push("MORPH_FEATURE_MARGIN_COLLAPSE".into()); }
        if self.features.contains("min_max") { d.push("MORPH_FEATURE_MIN_MAX".into()); }
        if self.features.contains("border_box") { d.push("MORPH_FEATURE_BORDER_BOX".into()); }
        if self.features.contains("image") { d.push("MORPH_FEATURE_IMAGE".into()); }
        if self.features.contains("input") { d.push("MORPH_FEATURE_INPUT".into()); }
        if self.features.contains("dirty_rendering") || self.features.contains("scroll") { d.push("MORPH_FEATURE_DIRTY_RENDERING".into()); }
        if self.features.contains("forge") { d.push("MORPH_RENDERER_FORGE".into()); }
        d
    }

    fn walk(nodes: &[IRNode]) -> Vec<&IRNode> {
        let mut out = Vec::new();
        for n in nodes {
            out.push(n);
            out.extend(Self::walk(&n.children));
            out.extend(Self::walk(&n.then_nodes));
            out.extend(Self::walk(&n.else_nodes));
            if let Some(ref tmpl) = n.item_template {
                out.push(tmpl);
                out.extend(Self::walk(&tmpl.children));
            }
        }
        out
    }
}
