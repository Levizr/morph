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
            ..Default::default()
        }
    }
}
