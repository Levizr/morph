use anyhow::Result;
use morph_ir::{IRWindow, IRNode};
use std::path::Path;

pub mod cpp;
pub mod rust;

pub use cpp::CppEmitter;
pub use rust::RustEmitter;

/// Feature detection from IR nodes.
pub struct FeatureSet {
    pub flex: bool,
    pub radius: bool,
    pub opacity: bool,
    pub transform: bool,
    pub animation: bool,
    pub scrollbar: bool,
    pub positioning: bool,
}

impl FeatureSet {
    pub fn scan(windows: &[IRWindow]) -> Self {
        let mut flex = false;
        let mut radius = false;
        let mut opacity = false;
        let mut transform = false;
        let mut animation = false;
        let mut scrollbar = false;
        let mut positioning = false;

        for window in windows {
            Self::scan_nodes(&window.nodes, &mut flex, &mut radius, &mut opacity, &mut transform, &mut animation, &mut scrollbar, &mut positioning);
        }

        Self { flex, radius, opacity, transform, animation, scrollbar, positioning }
    }

    fn scan_nodes(
        nodes: &[IRNode],
        flex: &mut bool,
        radius: &mut bool,
        opacity: &mut bool,
        transform: &mut bool,
        animation: &mut bool,
        scrollbar: &mut bool,
        positioning: &mut bool,
    ) {
        for node in nodes {
            if node.style.flex_grow > 0.0 || node.style.flex_shrink != 1.0 || node.style.gap > 0.0 {
                *flex = true;
            }
            if node.style.border_radius > 0.0 {
                *radius = true;
            }
            if node.style.opacity < 1.0 {
                *opacity = true;
            }
            if !node.animations.is_empty() {
                *animation = true;
            }
            if node.style.position != "static" {
                *positioning = true;
            }
            Self::scan_nodes(&node.children, flex, radius, opacity, transform, animation, scrollbar, positioning);
        }
    }

    pub fn required_defines(&self) -> Vec<String> {
        let mut defines = Vec::new();
        if self.flex { defines.push("MORPH_FEATURE_FLEX".to_string()); }
        if self.radius { defines.push("MORPH_FEATURE_RADIUS".to_string()); }
        if self.opacity { defines.push("MORPH_FEATURE_OPACITY".to_string()); }
        if self.transform { defines.push("MORPH_FEATURE_TRANSFORM".to_string()); }
        if self.animation { defines.push("MORPH_FEATURE_ANIMATION".to_string()); }
        if self.scrollbar { defines.push("MORPH_FEATURE_SCROLLBAR".to_string()); }
        if self.positioning { defines.push("MORPH_FEATURE_POSITIONING".to_string()); }
        defines
    }

    pub fn required_headers(&self) -> Vec<String> {
        let mut headers = vec!["ui/rect.h".to_string(), "ui/text.h".to_string()];
        if self.flex { headers.push("ui/flex.h".to_string()); }
        if self.radius { headers.push("ui/radius.h".to_string()); }
        if self.animation { headers.push("ui/animation.h".to_string()); }
        if self.scrollbar { headers.push("ui/scrollbar.h".to_string()); }
        if self.positioning { headers.push("ui/position.h".to_string()); }
        headers
    }
}
