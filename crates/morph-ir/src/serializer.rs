//! IR → JSON serializer for the dev socket.
//!
//! Mirrors `morph/ir/serializer.py` (`IRSerializer`): windows, keyframes,
//! recursive nodes and full style dicts, using the exact key names the C++
//! dev runtime (`runtime/cpp/dev/ir_deserializer.h`) parses. Optional node
//! fields are only emitted when set, matching Python's `to_dict`.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::transforms::{LengthComp, LengthUnit, TransformOp};
use crate::{IRAnimation, IRKeyframe, IRNode, IRStyle, IRWindow};

/// Serializes IR trees to JSON for the dev socket.
pub struct IRSerializer;

impl IRSerializer {
    /// Build the root payload. `logic_so_path` is attached when the logic
    /// library has been compiled, so the dev runtime can dlopen it.
    pub fn to_dict(windows: &[IRWindow], logic_so_path: Option<&str>) -> Value {
        let mut root = Map::new();
        root.insert("type".to_string(), Value::String("app".to_string()));
        root.insert(
            "windows".to_string(),
            Value::Array(windows.iter().map(Self::window).collect()),
        );
        if let Some(path) = logic_so_path {
            root.insert("logic_so_path".to_string(), Value::String(path.to_string()));
        }
        Value::Object(root)
    }

    /// Serialize the root payload to a JSON string.
    pub fn to_json(
        windows: &[IRWindow],
        logic_so_path: Option<&str>,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string(&Self::to_dict(windows, logic_so_path))
    }

    fn window(w: &IRWindow) -> Value {
        let mut out = Map::new();
        out.insert("id".to_string(), Value::String(w.window_id.clone()));
        out.insert("title".to_string(), Value::String(w.title.clone()));
        out.insert("width".to_string(), Value::from(w.width));
        out.insert("height".to_string(), Value::from(w.height));
        out.insert("visible".to_string(), Value::from(w.visible));
        out.insert("renderer".to_string(), Value::String(w.renderer.clone()));
        out.insert("nodes".to_string(), Value::Array(w.nodes.iter().map(Self::node).collect()));
        out.insert("startup_logs".to_string(), strings(&w.startup_logs));
        out.insert("premain_functions".to_string(), strings(&w.premain_functions));
        out.insert("extra_headers".to_string(), strings(&w.extra_headers));
        out.insert(
            "state_vars".to_string(),
            Value::Array(w.state_vars.iter().map(string_map).collect()),
        );
        out.insert(
            "effect_decls".to_string(),
            Value::Array(w.effect_decls.iter().map(string_map).collect()),
        );
        out.insert(
            "shared_vars".to_string(),
            Value::Array(w.shared_vars.iter().map(string_map).collect()),
        );
        out.insert(
            "channel_subs".to_string(),
            Value::Array(w.channel_subs.iter().map(string_map).collect()),
        );
        out.insert(
            "cpp_imports".to_string(),
            Value::Array(w.cpp_imports.iter().map(string_map).collect()),
        );
        out.insert("keyframes".to_string(), Self::keyframes_dict(&w.keyframes));
        if let Some(v) = w.min_width {
            out.insert("min_width".to_string(), Value::from(v));
        }
        if let Some(v) = w.max_width {
            out.insert("max_width".to_string(), Value::from(v));
        }
        if let Some(v) = w.min_height {
            out.insert("min_height".to_string(), Value::from(v));
        }
        if let Some(v) = w.max_height {
            out.insert("max_height".to_string(), Value::from(v));
        }
        Value::Object(out)
    }

    fn keyframes_dict(keyframes: &HashMap<String, Vec<IRKeyframe>>) -> Value {
        let mut names: Vec<&String> = keyframes.keys().collect();
        names.sort();
        let mut out = Map::new();
        for name in names {
            let frames = keyframes.get(name).map_or(&[] as &[IRKeyframe], Vec::as_slice);
            out.insert(name.clone(), Value::Array(frames.iter().map(Self::keyframe).collect()));
        }
        Value::Object(out)
    }

    fn keyframe(kf: &IRKeyframe) -> Value {
        let mut out = Map::new();
        out.insert("offset".to_string(), num(kf.offset));
        out.insert("style".to_string(), Self::keyframe_style(kf));
        out.insert("raw".to_string(), string_map(&kf.raw));
        Value::Object(out)
    }

    /// Partial style dict — only fields the keyframe explicitly declares.
    ///
    /// Mirrors `_keyframe_style_dict`: presence in the JSON always means
    /// "declared", so default-compare heuristics never drop a legitimate
    /// declaration like `opacity: 1`.
    fn keyframe_style(kf: &IRKeyframe) -> Value {
        let full = Self::style(&kf.style);
        let keep: HashSet<&str> = if kf.declared.is_empty() {
            fallback_declared(&kf.style)
        } else {
            kf.declared.iter().map(String::as_str).collect()
        };
        match full {
            Value::Object(map) => {
                Value::Object(map.into_iter().filter(|(k, _)| keep.contains(k.as_str())).collect())
            }
            other => other,
        }
    }

    fn animations(anims: &[IRAnimation]) -> Value {
        Value::Array(
            anims
                .iter()
                .map(|a| {
                    let mut out = Map::new();
                    out.insert("name".to_string(), Value::String(a.name.clone()));
                    out.insert("duration".to_string(), num(a.duration));
                    out.insert("easing".to_string(), Value::String(a.easing.clone()));
                    out.insert("delay".to_string(), num(a.delay));
                    out.insert("iterations".to_string(), num(a.iterations));
                    out.insert("direction".to_string(), Value::String(a.direction.clone()));
                    out.insert("fill_mode".to_string(), Value::String(a.fill_mode.clone()));
                    out.insert("play_state".to_string(), Value::String(a.play_state.clone()));
                    Value::Object(out)
                })
                .collect(),
        )
    }

    fn style(s: &IRStyle) -> Value {
        let mut out = Map::new();
        out.insert("bg_color".to_string(), floats(&s.bg_color));
        out.insert("color".to_string(), floats(&s.color));
        out.insert("width".to_string(), opt_num(s.width));
        out.insert("min_width".to_string(), opt_num(s.min_width));
        out.insert("max_width".to_string(), opt_num(s.max_width));
        out.insert("height".to_string(), opt_num(s.height));
        out.insert("min_height".to_string(), opt_num(s.min_height));
        out.insert("max_height".to_string(), opt_num(s.max_height));
        out.insert("margin".to_string(), floats(&s.margin));
        out.insert(
            "margin_auto".to_string(),
            Value::Array(s.margin_auto.iter().map(|b| Value::from(*b)).collect()),
        );
        out.insert("padding".to_string(), floats(&s.padding));
        out.insert("border_radius".to_string(), num(s.border_radius));
        out.insert("font_size".to_string(), num(s.font_size));
        out.insert("font_weight".to_string(), Value::String(s.font_weight.clone()));
        out.insert("text_align".to_string(), Value::String(s.text_align.clone()));
        out.insert("display".to_string(), Value::String(s.display.clone()));
        out.insert("flex_dir".to_string(), Value::String(s.flex_dir.clone()));
        out.insert("flex_grow".to_string(), num(s.flex_grow));
        out.insert("flex_shrink".to_string(), num(s.flex_shrink));
        out.insert("flex_basis".to_string(), Value::String(s.flex_basis.clone()));
        out.insert("gap".to_string(), num(s.gap));
        out.insert("overflow".to_string(), Value::String(s.overflow.clone()));
        out.insert("position".to_string(), Value::String(s.position.clone()));
        out.insert("left".to_string(), opt_num(s.left));
        out.insert("right".to_string(), opt_num(s.right));
        out.insert("top".to_string(), opt_num(s.top));
        out.insert("bottom".to_string(), opt_num(s.bottom));
        out.insert("justify_content".to_string(), Value::String(s.justify_content.clone()));
        out.insert("align_items".to_string(), Value::String(s.align_items.clone()));
        out.insert("flex_wrap".to_string(), Value::String(s.flex_wrap.clone()));
        out.insert("cursor".to_string(), Value::String(s.cursor.clone()));
        out.insert("scrollbar_width".to_string(), num(s.scrollbar_width));
        out.insert("scrollbar_track_color".to_string(), floats(&s.scrollbar_track_color));
        out.insert("scrollbar_thumb_color".to_string(), floats(&s.scrollbar_thumb_color));
        out.insert("scrollbar_border_radius".to_string(), num(s.scrollbar_border_radius));
        out.insert("border_width".to_string(), num(s.border_width));
        out.insert("border_color".to_string(), floats(&s.border_color));
        out.insert("border_style".to_string(), Value::String(s.border_style.clone()));
        out.insert("box_sizing".to_string(), Value::String(s.box_sizing.clone()));
        out.insert("z_index".to_string(), s.z_index.map_or(Value::Null, Value::from));
        out.insert("opacity".to_string(), num(s.opacity));
        out.insert(
            "transform_ops".to_string(),
            match s.transform_ops.as_deref().unwrap_or(&[]) {
                [] => Value::Null,
                ops => Value::Array(ops.iter().map(Self::transform_op).collect()),
            },
        );
        out.insert(
            "transform_matrix".to_string(),
            s.transform_matrix.map_or(Value::Null, |m| floats(&m)),
        );
        out.insert(
            "transform_origin".to_string(),
            s.transform_origin_resolved
                .map_or(Value::Null, |(x, y)| Value::Array(vec![num(x), num(y)])),
        );
        out.insert(
            "transform_origin_raw".to_string(),
            s.transform_origin.map_or(Value::Null, |((x, x_pct), (y, y_pct))| {
                Value::Array(vec![
                    Value::Array(vec![num(x), Value::from(x_pct)]),
                    Value::Array(vec![num(y), Value::from(y_pct)]),
                ])
            }),
        );
        Value::Object(out)
    }

    /// Serialize a transform op in the same tuple shape Python emits
    /// (`("rotate", 45.0)` → `["rotate", 45.0]`).
    fn transform_op(op: &TransformOp) -> Value {
        match op {
            TransformOp::Matrix(m) => {
                Value::Array(vec![Value::String("matrix".to_string()), floats(m)])
            }
            TransformOp::Matrix3d(m) => {
                Value::Array(vec![Value::String("matrix3d".to_string()), floats(m)])
            }
            TransformOp::Perspective(v) => {
                Value::Array(vec![Value::String("perspective".to_string()), num(*v)])
            }
            TransformOp::Rotate(d) => {
                Value::Array(vec![Value::String("rotate".to_string()), num(*d)])
            }
            TransformOp::RotateX(d) => {
                Value::Array(vec![Value::String("rotatex".to_string()), num(*d)])
            }
            TransformOp::RotateY(d) => {
                Value::Array(vec![Value::String("rotatey".to_string()), num(*d)])
            }
            TransformOp::RotateZ(d) => {
                Value::Array(vec![Value::String("rotatez".to_string()), num(*d)])
            }
            TransformOp::Rotate3d(x, y, z, d) => {
                Value::Array(vec![Value::String("rotate3d".to_string()), floats(&[*x, *y, *z, *d])])
            }
            TransformOp::Translate(a, b) => Value::Array(vec![
                Value::String("translate".to_string()),
                Value::Array(vec![length_comp(*a), length_comp(*b)]),
            ]),
            TransformOp::Translate3d(a, b, c) => Value::Array(vec![
                Value::String("translate3d".to_string()),
                Value::Array(vec![length_comp(*a), length_comp(*b), length_comp(*c)]),
            ]),
            TransformOp::Scale(x, y) => Value::Array(vec![
                Value::String("scale".to_string()),
                Value::Array(vec![num(*x), num(*y)]),
            ]),
            TransformOp::Scale3d(x, y, z) => Value::Array(vec![
                Value::String("scale3d".to_string()),
                Value::Array(vec![num(*x), num(*y), num(*z)]),
            ]),
            TransformOp::Skew(x, y) => Value::Array(vec![
                Value::String("skew".to_string()),
                Value::Array(vec![num(*x), num(*y)]),
            ]),
        }
    }

    fn node(n: &IRNode) -> Value {
        let mut out = Map::new();
        out.insert("id".to_string(), Value::String(n.node_id.clone()));
        out.insert("type".to_string(), Value::String(n.node_type.clone()));
        out.insert("x".to_string(), num(n.x));
        out.insert("y".to_string(), num(n.y));
        out.insert("w".to_string(), num(n.w));
        out.insert("h".to_string(), num(n.h));
        out.insert("text".to_string(), Value::String(n.text_content.clone()));
        out.insert("attrs".to_string(), string_map(&n.attrs));
        out.insert("style".to_string(), Self::style(&n.style));
        out.insert(
            "children".to_string(),
            Value::Array(n.children.iter().map(Self::node).collect()),
        );
        out.insert(
            "events".to_string(),
            Value::Array(
                n.events
                    .iter()
                    .map(|e| {
                        let mut ev = Map::new();
                        ev.insert("trigger".to_string(), Value::String(e.trigger.clone()));
                        ev.insert("action".to_string(), Value::String(e.action.clone()));
                        ev.insert("target".to_string(), Value::String(e.target.clone()));
                        Value::Object(ev)
                    })
                    .collect(),
            ),
        );
        if !n.reactive_text.is_empty() {
            out.insert("reactive_text".to_string(), Value::String(n.reactive_text.clone()));
        }
        if !n.reactive_class.is_empty() {
            out.insert("reactive_class".to_string(), Value::String(n.reactive_class.clone()));
        }
        if !n.reactive_style.is_empty() {
            out.insert("reactive_style".to_string(), string_map(&n.reactive_style));
        }
        if !n.reactive_attrs.is_empty() {
            out.insert("reactive_attrs".to_string(), string_map(&n.reactive_attrs));
        }
        if !n.class_conditional_effects.is_empty() {
            out.insert(
                "class_conditional_effects".to_string(),
                Value::Array(
                    n.class_conditional_effects
                        .iter()
                        .map(|fx| {
                            Value::Array(vec![
                                Value::String(fx.condition.clone()),
                                string_map(&fx.on_styles),
                                string_map(&fx.off_styles),
                            ])
                        })
                        .collect(),
                ),
            );
        }
        if !n.condition_expr.is_empty() {
            out.insert("condition_expr".to_string(), Value::String(n.condition_expr.clone()));
            out.insert(
                "then_nodes".to_string(),
                Value::Array(n.then_nodes.iter().map(Self::node).collect()),
            );
            out.insert(
                "else_nodes".to_string(),
                Value::Array(n.else_nodes.iter().map(Self::node).collect()),
            );
        }
        if !n.list_expr.is_empty() {
            out.insert("list_expr".to_string(), Value::String(n.list_expr.clone()));
            out.insert("list_key_expr".to_string(), Value::String(n.list_key_expr.clone()));
        }
        if let Some(tmpl) = n.item_template.as_deref() {
            out.insert("item_template".to_string(), Self::node(tmpl));
        }
        if let Some(hover) = n.hover_style.as_ref() {
            out.insert("hover_style".to_string(), Self::style(hover));
        }
        if let Some(active) = n.active_style.as_ref() {
            out.insert("active_style".to_string(), Self::style(active));
        }
        if n.transition_duration > 0.0 {
            out.insert("transition_duration".to_string(), num(n.transition_duration));
            out.insert("transition_easing".to_string(), Value::String(n.transition_easing.clone()));
        }
        if !n.animations.is_empty() {
            out.insert("animations".to_string(), Self::animations(&n.animations));
        }
        if !n.hover_animations.is_empty() {
            out.insert("hover_animations".to_string(), Self::animations(&n.hover_animations));
        }
        Value::Object(out)
    }
}

/// Finite float as JSON, mirroring Python's `_clean_inf` (inf/NaN → null).
fn num(v: f32) -> Value {
    if v.is_finite() {
        Value::from(v)
    } else {
        Value::Null
    }
}

fn opt_num(v: Option<f32>) -> Value {
    v.map_or(Value::Null, num)
}

fn floats(values: &[f32]) -> Value {
    Value::Array(values.iter().map(|v| num(*v)).collect())
}

fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().map(|s| Value::String(s.clone())).collect())
}

fn string_map(map: &HashMap<String, String>) -> Value {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort();
    Value::Object(
        keys.into_iter()
            .map(|k| {
                let v = map.get(k).map_or("", String::as_str);
                (k.clone(), Value::String(v.to_string()))
            })
            .collect(),
    )
}

fn length_comp(comp: LengthComp) -> Value {
    let unit = match comp.1 {
        LengthUnit::Px => "px",
        LengthUnit::Pct => "%",
    };
    Value::Array(vec![num(comp.0), Value::String(unit.to_string())])
}

/// Fallback declared-field set when a keyframe carries no explicit
/// `declared` list, mirroring Python's `_keyframe_style_dict` fallback.
// Exact equality is intentional change/dirty detection.
#[allow(clippy::float_cmp)]
fn fallback_declared(style: &IRStyle) -> HashSet<&'static str> {
    let mut keep = HashSet::new();
    if style.opacity != 1.0 {
        keep.insert("opacity");
    }
    if style.bg_color != [0.0, 0.0, 0.0, 0.0] {
        keep.insert("bg_color");
    }
    if style.color != [0.0, 0.0, 0.0, 1.0] {
        keep.insert("color");
    }
    if style.border_radius != 0.0 {
        keep.insert("border_radius");
    }
    if style.font_size != 16.0 {
        keep.insert("font_size");
    }
    if style.width.is_some() {
        keep.insert("width");
    }
    if style.height.is_some() {
        keep.insert("height");
    }
    if style.left.is_some() {
        keep.insert("left");
    }
    if style.top.is_some() {
        keep.insert("top");
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_window() -> IRWindow {
        let mut window = IRWindow {
            window_id: "win_0".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            renderer: String::new(),
            ..Default::default()
        };
        let mut node = IRNode {
            node_id: "node_0000".to_string(),
            node_type: "div".to_string(),
            text_content: "hi".to_string(),
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            ..Default::default()
        };
        node.style.bg_color = [1.0, 0.0, 0.0, 1.0];
        node.attrs.insert("data-x".to_string(), "1".to_string());
        window.nodes.push(node);
        window
    }

    #[test]
    fn root_shape_windows_and_type() {
        let value = IRSerializer::to_dict(&[sample_window()], None);
        assert_eq!(value["type"], Value::String("app".to_string()));
        assert_eq!(value["windows"][0]["id"], Value::String("win_0".to_string()));
        assert_eq!(value["windows"][0]["title"], Value::String("Test".to_string()));
        assert!(value.get("logic_so_path").is_none());
    }

    #[test]
    fn logic_so_path_attached_at_root() {
        let value = IRSerializer::to_dict(&[sample_window()], Some("/tmp/logic.so"));
        assert_eq!(value["logic_so_path"], Value::String("/tmp/logic.so".to_string()));
    }

    #[test]
    fn node_shape_matches_dev_protocol() {
        let value = IRSerializer::to_dict(&[sample_window()], None);
        let node = &value["windows"][0]["nodes"][0];
        assert_eq!(node["id"], Value::String("node_0000".to_string()));
        assert_eq!(node["type"], Value::String("div".to_string()));
        assert_eq!(node["text"], Value::String("hi".to_string()));
        assert_eq!(node["x"], Value::from(1.0f32));
        assert_eq!(node["attrs"]["data-x"], Value::String("1".to_string()));
        assert_eq!(
            node["style"]["bg_color"],
            Value::Array(vec![
                Value::from(1.0f32),
                Value::from(0.0f32),
                Value::from(0.0f32),
                Value::from(1.0f32),
            ])
        );
        assert!(node.get("reactive_text").is_none());
        assert!(node.get("hover_style").is_none());
        assert!(node.get("animations").is_none());
    }

    #[test]
    fn unset_optionals_serialize_as_null() {
        let value = IRSerializer::to_dict(&[sample_window()], None);
        let style = &value["windows"][0]["nodes"][0]["style"];
        assert_eq!(style["width"], Value::Null);
        assert_eq!(style["z_index"], Value::Null);
        assert_eq!(style["transform_ops"], Value::Null);
        assert_eq!(style["transform_matrix"], Value::Null);
        assert_eq!(style["transform_origin"], Value::Null);
    }

    #[test]
    fn non_finite_floats_become_null() {
        let mut window = sample_window();
        window.nodes[0].style.opacity = f32::INFINITY;
        let value = IRSerializer::to_dict(&[window], None);
        assert_eq!(value["windows"][0]["nodes"][0]["style"]["opacity"], Value::Null);
    }

    #[test]
    fn transform_serialization_shape() {
        use crate::transforms::TransformOp;
        let mut window = sample_window();
        window.nodes[0].style.transform_ops =
            Some(vec![TransformOp::Rotate(45.0), TransformOp::Scale(2.0, 3.0)]);
        window.nodes[0].style.transform_matrix = Some([1.0; 16]);
        window.nodes[0].style.transform_origin_resolved = Some((0.0, 1.0));
        window.nodes[0].style.transform_origin = Some(((0.0, false), (100.0, true)));
        let value = IRSerializer::to_dict(&[window], None);
        let style = &value["windows"][0]["nodes"][0]["style"];
        assert_eq!(
            style["transform_ops"],
            serde_json::json!([["rotate", 45.0], ["scale", [2.0, 3.0]]])
        );
        assert_eq!(style["transform_matrix"][0], Value::from(1.0f32));
        assert_eq!(style["transform_origin"], serde_json::json!([0.0, 1.0]));
        assert_eq!(style["transform_origin_raw"], serde_json::json!([[0.0, false], [100.0, true]]));
    }

    #[test]
    fn keyframes_keep_only_declared_fields() {
        let mut window = sample_window();
        let mut kf = IRKeyframe {
            offset: 0.5,
            style: IRStyle::new(),
            declared: vec!["opacity".to_string()],
            raw: HashMap::new(),
        };
        kf.style.opacity = 0.25;
        kf.style.bg_color = [1.0, 1.0, 1.0, 1.0];
        window.keyframes.insert("fade".to_string(), vec![kf]);
        let value = IRSerializer::to_dict(&[window], None);
        let frame = &value["windows"][0]["keyframes"]["fade"][0];
        assert_eq!(frame["offset"], Value::from(0.5f32));
        assert_eq!(frame["style"]["opacity"], Value::from(0.25f32));
        assert!(frame["style"].get("bg_color").is_none());
    }

    #[test]
    fn to_json_round_trips() {
        let text = IRSerializer::to_json(&[sample_window()], None).unwrap();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["type"], Value::String("app".to_string()));
    }
}
