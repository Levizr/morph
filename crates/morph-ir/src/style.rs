use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IRStyle {
    pub bg_color: [f32; 4],
    pub color: [f32; 4],
    pub width: Option<f32>,
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub height: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
    pub margin: [f32; 4],
    pub margin_auto: [bool; 4],
    pub padding: [f32; 4],
    pub border_radius: f32,
    pub font_size: f32,
    pub font_weight: String,
    pub text_align: String,
    pub display: String,
    pub flex_dir: String,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: String,
    pub gap: f32,
    pub position: String,
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub top: Option<f32>,
    pub bottom: Option<f32>,
    pub justify_content: String,
    pub align_items: String,
    pub flex_wrap: String,
    pub cursor: String,
    pub overflow: String,
    pub border_width: f32,
    pub border_color: [f32; 4],
    pub border_style: String,
    pub box_sizing: String,
    pub z_index: Option<i32>,
    pub opacity: f32,
    // ── Scrollbar (feature: scrollbar) ───────────────────────────────
    pub scrollbar_width: f32,
    pub scrollbar_track_color: [f32; 4],
    pub scrollbar_thumb_color: [f32; 4],
    pub scrollbar_border_radius: f32,
    // ── Transform (feature: transform) ───────────────────────────────
    pub transform_ops: Option<Vec<(String, Vec<f32>)>>,
    pub transform_matrix: Option<[f32; 16]>,
    pub transform_origin: Option<((f32, bool), (f32, bool))>,
    pub transform_origin_resolved: Option<(f32, f32)>,
}

impl IRStyle {
    pub fn new() -> Self {
        Self {
            bg_color: [0.0, 0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
            width: None,
            height: None,
            font_size: 16.0,
            display: "block".to_string(),
            position: "static".to_string(),
            flex_dir: "row".to_string(),
            justify_content: "flex-start".to_string(),
            align_items: "stretch".to_string(),
            overflow: "visible".to_string(),
            opacity: 1.0,
            border_style: "none".to_string(),
            box_sizing: "content-box".to_string(),
            cursor: "default".to_string(),
            flex_shrink: 1.0,
            font_weight: "normal".to_string(),
            text_align: "left".to_string(),
            flex_basis: "auto".to_string(),
            flex_wrap: "nowrap".to_string(),
            scrollbar_width: 8.0,
            scrollbar_track_color: [0.85, 0.85, 0.85, 0.4],
            scrollbar_thumb_color: [0.5, 0.5, 0.5, 0.6],
            scrollbar_border_radius: 4.0,
            ..Default::default()
        }
    }

    /// True when no meaningful style delta has been applied for a pseudo
    /// (hover/active) bucket, i.e. every field is still at its default.
    pub fn is_empty_style(&self) -> bool {
        self.bg_color == [0.0, 0.0, 0.0, 0.0]
            && self.color == [0.0, 0.0, 0.0, 1.0]
            && self.border_color == [0.0, 0.0, 0.0, 1.0]
            && self.border_width == 0.0
            && self.border_style == "none"
            && self.width.is_none()
            && self.height.is_none()
            && self.min_width.is_none()
            && self.max_width.is_none()
            && self.min_height.is_none()
            && self.max_height.is_none()
            && self.padding == [0.0, 0.0, 0.0, 0.0]
            && self.margin == [0.0, 0.0, 0.0, 0.0]
            && self.border_radius == 0.0
            && self.font_size == 16.0
            && self.font_weight == "normal"
            && self.text_align == "left"
            && self.display == "block"
            && self.flex_dir == "row"
            && self.gap == 0.0
            && self.position == "static"
            && self.justify_content == "flex-start"
            && self.align_items == "stretch"
            && self.flex_wrap == "nowrap"
            && self.cursor == "default"
            && self.overflow == "visible"
            && self.box_sizing == "content-box"
            && self.opacity == 1.0
            && self.z_index.is_none()
            && self.left.is_none()
            && self.right.is_none()
            && self.top.is_none()
            && self.bottom.is_none()
    }
}
