//! CSS property registry — single source of truth for supported properties
//! Mirrors Python's `morph/ir/css_registry.py` and `morph/style/properties.py`

pub static KNOWN_PROPERTIES: &[&str] = &[
    "display", "position", "top", "right", "bottom", "left", "z-index",
    "float", "clear", "box-sizing", "overflow", "overflow-x", "overflow-y",
    "flex", "flex-direction", "flex-wrap", "flex-flow", "justify-content",
    "align-items", "align-content", "align-self", "flex-grow", "flex-shrink",
    "flex-basis", "gap", "row-gap", "column-gap", "order",
    "width", "height", "min-width", "max-width", "min-height", "max-height",
    "margin", "margin-top", "margin-right", "margin-bottom", "margin-left",
    "padding", "padding-top", "padding-right", "padding-bottom", "padding-left",
    "color", "background-color", "background", "font-size", "font-weight",
    "font-family", "line-height", "text-align", "text-decoration", "text-transform",
    "letter-spacing", "white-space", "word-break",
    "border", "border-width", "border-color", "border-style", "border-radius",
    "border-top", "border-right", "border-bottom", "border-left",
    "opacity", "visibility", "cursor", "box-shadow", "transform", "transform-origin",
    "transition", "transition-duration", "transition-timing-function", "transition-delay",
    "animation", "animation-duration", "animation-timing-function", "animation-delay",
    "animation-iteration-count", "animation-direction", "animation-fill-mode",
    "scrollbar-width", "scrollbar-color",
];

pub static CSS_TO_IR: &[(&str, &str)] = &[
    ("background-color", "bg_color"),
    ("color", "color"),
    ("width", "width"),
    ("height", "height"),
    ("min-width", "min_width"),
    ("max-width", "max_width"),
    ("min-height", "min_height"),
    ("max-height", "max_height"),
    ("margin", "margin"),
    ("padding", "padding"),
    ("border-radius", "border_radius"),
    ("font-size", "font_size"),
    ("font-weight", "font_weight"),
    ("text-align", "text_align"),
    ("display", "display"),
    ("flex-direction", "flex_dir"),
    ("flex-grow", "flex_grow"),
    ("flex-shrink", "flex_shrink"),
    ("flex-basis", "flex_basis"),
    ("gap", "gap"),
    ("position", "position"),
    ("left", "left"),
    ("right", "right"),
    ("top", "top"),
    ("bottom", "bottom"),
    ("justify-content", "justify_content"),
    ("align-items", "align_items"),
    ("flex-wrap", "flex_wrap"),
    ("cursor", "cursor"),
    ("overflow", "overflow"),
    ("border-width", "border_width"),
    ("border-color", "border_color"),
    ("border-style", "border_style"),
    ("box-sizing", "box_sizing"),
    ("z-index", "z_index"),
    ("opacity", "opacity"),
    ("transform", "transform"),
    ("transform-origin", "transform_origin"),
];

pub fn is_known_property(prop: &str) -> bool {
    KNOWN_PROPERTIES.contains(&prop)
}

pub fn property_feature(prop: &str) -> Option<&'static str> {
    match prop {
        "display" | "flex-direction" | "justify-content" | "align-items" | "gap" | "flex-wrap" | "flex-grow" | "flex-shrink" => Some("flex"),
        "border-radius" => Some("radius"),
        "opacity" => Some("opacity"),
        "transform" | "transform-origin" => Some("transform"),
        "animation" | "animation-duration" => Some("animation"),
        "scrollbar-width" | "scrollbar-color" => Some("scrollbar"),
        "position" | "top" | "right" | "bottom" | "left" | "z-index" => Some("positioning"),
        _ => None,
    }
}
