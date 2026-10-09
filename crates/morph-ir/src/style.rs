use serde::{Deserialize, Serialize};

use crate::gradient::IRGradient;

/// A CSS length with browser-style units. Absolute physical units
/// (`in/cm/mm/q/pt/pc`) are folded to px at parse time (96dpi); everything
/// else resolves at layout against its own base:
/// `%` → containing block, `em/ex/ch` → element font, `rem` → root font,
/// `vw/vh/vmin/vmax` (+ `sv*/lv*/dv*` aliases) → viewport.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Length {
    Px(f32),
    Pct(f32),
    Em(f32),
    Rem(f32),
    Vw(f32),
    Vh(f32),
    Vmin(f32),
    Vmax(f32),
    Ch(f32),
    Ex(f32),
}

impl Default for Length {
    fn default() -> Self {
        Length::Px(0.0)
    }
}

impl Length {
    pub fn px(v: f32) -> Self {
        Length::Px(v)
    }
    /// True only for an explicit zero (`0`, `0px`). Relative zero
    /// (`0%`, `0em`…) still carries a unit the runtime must see, so it
    /// is emitted rather than skipped.
    pub fn is_zero(self) -> bool {
        matches!(self, Length::Px(v) if v == 0.0)
    }
    /// Raw numeric value regardless of unit (for serialization/codegen).
    pub fn number(self) -> f32 {
        match self {
            Length::Px(v)
            | Length::Pct(v)
            | Length::Em(v)
            | Length::Rem(v)
            | Length::Vw(v)
            | Length::Vh(v)
            | Length::Vmin(v)
            | Length::Vmax(v)
            | Length::Ch(v)
            | Length::Ex(v) => v,
        }
    }
    /// Canonical CSS spelling for the serializer (`50%`, `1.5em`, `10vw`).
    /// `Px` stays a bare number so existing JSON stays byte-identical.
    pub fn to_css(self) -> String {
        match self {
            Length::Px(v) => format_css_num(v),
            Length::Pct(v) => format!("{}%", format_css_num(v)),
            Length::Em(v) => format!("{}em", format_css_num(v)),
            Length::Rem(v) => format!("{}rem", format_css_num(v)),
            Length::Vw(v) => format!("{}vw", format_css_num(v)),
            Length::Vh(v) => format!("{}vh", format_css_num(v)),
            Length::Vmin(v) => format!("{}vmin", format_css_num(v)),
            Length::Vmax(v) => format!("{}vmax", format_css_num(v)),
            Length::Ch(v) => format!("{}ch", format_css_num(v)),
            Length::Ex(v) => format!("{}ex", format_css_num(v)),
        }
    }
}

fn format_css_num(v: f32) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IRStyle {
    pub bg_color: [f32; 4],
    pub bg_gradient: Option<IRGradient>,
    pub color: [f32; 4],
    pub width: Option<Length>,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub height: Option<Length>,
    pub min_height: Option<Length>,
    pub max_height: Option<Length>,
    pub margin: [Length; 4],
    pub margin_auto: [bool; 4],
    pub padding: [Length; 4],
    pub border_radius: Length,
    // Per-corner radius (longhand, clockwise from top-left)
    pub border_top_left_radius: Option<Length>,
    pub border_top_right_radius: Option<Length>,
    pub border_bottom_right_radius: Option<Length>,
    pub border_bottom_left_radius: Option<Length>,
    pub font_size: Length,
    pub font_weight: String,
    pub text_align: String,
    pub display: String,
    pub flex_dir: String,
    pub flex_grow: f32,
    pub flex_shrink: f32,
    pub flex_basis: String,
    pub gap: Length,
    pub position: String,
    pub left: Option<Length>,
    pub right: Option<Length>,
    pub top: Option<Length>,
    pub bottom: Option<Length>,
    pub justify_content: String,
    pub align_items: String,
    pub align_self: String,
    pub flex_wrap: String,
    pub cursor: String,
    pub overflow_x: String,
    pub overflow_y: String,
    pub border_width: Length,
    pub border_color: [f32; 4],
    pub border_style: String,
    // Per-side borders (longhand)
    pub border_top_width: Option<Length>,
    pub border_right_width: Option<Length>,
    pub border_bottom_width: Option<Length>,
    pub border_left_width: Option<Length>,
    pub border_top_color: Option<[f32; 4]>,
    pub border_right_color: Option<[f32; 4]>,
    pub border_bottom_color: Option<[f32; 4]>,
    pub border_left_color: Option<[f32; 4]>,
    pub border_top_style: Option<String>,
    pub border_right_style: Option<String>,
    pub border_bottom_style: Option<String>,
    pub border_left_style: Option<String>,
    // border-image (for gradient borders)
    pub border_image: Option<String>,
    pub border_image_slice: Option<f32>,
    pub border_gradient: Option<IRGradient>,
    pub box_sizing: String,
    pub z_index: Option<i32>,
    pub opacity: f32,
    // ── Scrollbar (feature: scrollbar) ───────────────────────────────
    pub scrollbar_width: f32,
    pub scrollbar_track_color: [f32; 4],
    pub scrollbar_thumb_color: [f32; 4],
    pub scrollbar_border_radius: f32,
    // ── Transform (feature: transform) ───────────────────────────────
    pub transform_ops: Option<Vec<crate::transforms::TransformOp>>,
    pub transform_matrix: Option<[f32; 16]>,
    pub transform_origin: Option<((f32, bool), (f32, bool))>,
    pub transform_origin_resolved: Option<(f32, f32)>,
}

impl IRStyle {
    pub fn new() -> Self {
        Self {
            bg_color: [0.0, 0.0, 0.0, 0.0],
            color: [0.0, 0.0, 0.0, 1.0],
            border_color: [0.0, 0.0, 0.0, 1.0],
            width: None,
            height: None,
            font_size: Length::Px(16.0),
            display: "block".to_string(),
            position: "static".to_string(),
            flex_dir: "row".to_string(),
            justify_content: "flex-start".to_string(),
            align_items: "stretch".to_string(),
            align_self: "auto".to_string(),
            overflow_x: "visible".to_string(),
            overflow_y: "visible".to_string(),
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
    // Exact equality is intentional change/dirty detection.
    #[allow(clippy::float_cmp)]
    pub fn is_empty_style(&self) -> bool {
        self.bg_color == [0.0, 0.0, 0.0, 0.0]
            && self.bg_gradient.is_none()
            && self.color == [0.0, 0.0, 0.0, 1.0]
            && self.border_color == [0.0, 0.0, 0.0, 1.0]
            && self.border_width == Length::Px(0.0)
            && self.border_style == "none"
            && self.border_top_width.is_none()
            && self.border_right_width.is_none()
            && self.border_bottom_width.is_none()
            && self.border_left_width.is_none()
            && self.border_top_color.is_none()
            && self.border_right_color.is_none()
            && self.border_bottom_color.is_none()
            && self.border_left_color.is_none()
            && self.border_top_style.is_none()
            && self.border_right_style.is_none()
            && self.border_bottom_style.is_none()
            && self.border_left_style.is_none()
            && self.border_image.is_none()
            && self.border_image_slice.is_none()
            && self.border_gradient.is_none()
            && self.width.is_none()
            && self.height.is_none()
            && self.min_width.is_none()
            && self.max_width.is_none()
            && self.min_height.is_none()
            && self.max_height.is_none()
            && self.padding == [Length::Px(0.0); 4]
            && self.margin == [Length::Px(0.0); 4]
            && self.border_radius == Length::Px(0.0)
            && self.border_top_left_radius.is_none()
            && self.border_top_right_radius.is_none()
            && self.border_bottom_right_radius.is_none()
            && self.border_bottom_left_radius.is_none()
            && self.font_size == Length::Px(16.0)
            && self.font_weight == "normal"
            && self.text_align == "left"
            && self.display == "block"
            && self.flex_dir == "row"
            && self.gap == Length::Px(0.0)
            && self.position == "static"
            && self.justify_content == "flex-start"
            && self.align_items == "stretch"
            && self.align_self == "auto"
            && self.flex_wrap == "nowrap"
            && self.cursor == "default"
            && self.overflow_x == "visible"
            && self.overflow_y == "visible"
            && self.box_sizing == "content-box"
            && self.opacity == 1.0
            && self.z_index.is_none()
            && self.left.is_none()
            && self.right.is_none()
            && self.top.is_none()
            && self.bottom.is_none()
            && self.transform_matrix.is_none()
            && self.transform_origin.is_none()
    }
}
