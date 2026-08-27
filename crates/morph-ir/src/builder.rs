use std::collections::HashMap;

use crate::css_registry;
use crate::node::{IRWindow, IRNode, IREvent};
use crate::style::IRStyle;
use crate::tailwind::TailwindResolver;

pub struct IRBuilder {
    tailwind: TailwindResolver,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self { tailwind: TailwindResolver::new() }
    }

    pub fn build(
        &self,
        source: &morph_parser::MxSource,
        css_rules: &HashMap<String, morph_parser::CssRule>,
    ) -> Vec<IRWindow> {
        let mut windows = Vec::new();
        let all_state: Vec<HashMap<String, String>> = source.components.iter().flat_map(|c| {
            c.state_vars.iter().map(|sv| {
                let mut m = HashMap::new();
                m.insert("getter".into(), sv.getter.clone());
                m.insert("setter".into(), sv.setter.clone());
                m.insert("init".into(), sv.init.clone());
                m
            })
        }).collect();
        let all_effects: Vec<HashMap<String, String>> = source.components.iter().flat_map(|c| {
            c.effects.iter().map(|e| {
                let mut m = HashMap::new();
                m.insert("callback".into(), e.callback.clone());
                m.insert("deps".into(), e.deps.clone());
                m
            })
        }).collect();
        let wc = source.window_config.as_ref();
        let mut window = IRWindow {
            window_id: "w0".into(),
            title: wc.map(|w| w.title.clone()).unwrap_or_else(|| "Morph App".into()),
            width: wc.map(|w| w.width).unwrap_or(800),
            height: wc.map(|w| w.height).unwrap_or(600),
            visible: true,
            min_width: wc.and_then(|w| w.min_width),
            max_width: wc.and_then(|w| w.max_width),
            min_height: wc.and_then(|w| w.min_height),
            max_height: wc.and_then(|w| w.max_height),
            modal: wc.map(|w| w.modal).unwrap_or(false),
            nodes: vec![],
            startup_logs: source.console_logs.clone(),
            premain_functions: vec![],
            extra_headers: vec![],
            state_vars: all_state,
            effect_decls: all_effects,
            cpp_imports: source.cpp_imports.iter().map(|ci| {
                let mut m = HashMap::new();
                m.insert("path".into(), ci.path.clone());
                m.insert("specifiers".into(), ci.specifiers.join(", "));
                m
            }).collect(),
            keyframes: HashMap::new(),
        };
        for comp in &source.components {
            let node = self.build_node(&comp.jsx, "root", css_rules, 0);
            window.nodes.push(node);
        }
        windows.push(window);
        windows
    }

    fn build_node(
        &self,
        jsx: &morph_parser::JsxNode,
        node_id: &str,
        css_rules: &HashMap<String, morph_parser::CssRule>,
        depth: usize,
    ) -> IRNode {
        match jsx {
            morph_parser::JsxNode::Element { tag, props, children, line, col, .. } => {
                let mut node = IRNode {
                    node_id: node_id.to_string(),
                    node_type: tag.clone(),
                    ..Default::default()
                };
                let mut style = IRStyle::new();
                apply_ua_defaults(&mut style, tag);
                for (selector, rule) in css_rules {
                    if selector_matches(tag, props, selector) {
                        for (prop, val) in &rule.properties {
                            apply_css_prop(&mut style, prop, val);
                        }
                    }
                }
                if let Some(morph_parser::JsxPropValue::String(cls)) = props.get("className").or_else(|| props.get("class")) {
                    for (prop, val) in self.tailwind.resolve_many(cls) {
                        apply_css_prop(&mut style, &prop, &val);
                    }
                }
                if let Some(morph_parser::JsxPropValue::Style(map)) = props.get("style") {
                    for (prop, val) in map {
                        match val {
                            morph_parser::StyleValue::Static(s) => apply_css_prop(&mut style, prop, s),
                            morph_parser::StyleValue::Expr(e) => { node.reactive_style.insert(prop.clone(), e.clone()); }
                        }
                    }
                }
                node.style = style;
                for (k, v) in props {
                    match (k.as_str(), v) {
                        ("id", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("id".into(), s.clone()); }
                        ("src", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("src".into(), s.clone()); }
                        ("placeholder", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("placeholder".into(), s.clone()); }
                        ("type", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("type".into(), s.clone()); }
                        ("className", morph_parser::JsxPropValue::String(s)) => { node.reactive_class = s.clone(); }
                        ("class", morph_parser::JsxPropValue::String(s)) => { node.reactive_class = s.clone(); }
                        ("onClick", morph_parser::JsxPropValue::Fn(f)) => { node.events.push(IREvent { trigger: "click".into(), action: "call".into(), target: f.clone() }); }
                        ("onInput", morph_parser::JsxPropValue::Fn(f)) => { node.events.push(IREvent { trigger: "input".into(), action: "call".into(), target: f.clone() }); }
                        _ => {}
                    }
                }
                let text_parts: Vec<String> = children.iter().filter_map(|c| if let morph_parser::JsxNode::Text(t) = c { Some(t.clone()) } else { None }).collect();
                if !text_parts.is_empty() { node.text_content = text_parts.join(""); }
                for (i, child) in children.iter().enumerate() {
                    let child_id = format!("{node_id}_{i}");
                    let child_node = self.build_node(child, &child_id, css_rules, depth+1);
                    if child_node.node_type == "__text__" && child_node.text_content.trim().is_empty() {
                        continue;
                    }
                    node.children.push(child_node);
                }
                node
            }
            morph_parser::JsxNode::Fragment { children, .. } => {
                let mut node = IRNode { node_id: node_id.to_string(), node_type: "__fragment__".into(), ..Default::default() };
                for (i, child) in children.iter().enumerate() {
                    node.children.push(self.build_node(child, &format!("{node_id}_{i}"), css_rules, depth));
                }
                node
            }
            morph_parser::JsxNode::Text(t) => {
                let mut node = IRNode { node_id: node_id.to_string(), node_type: "__text__".into(), ..Default::default() };
                node.text_content = t.clone();
                node
            }
            morph_parser::JsxNode::Expression(e) => {
                let mut node = IRNode { node_id: node_id.to_string(), node_type: "__expr__".into(), ..Default::default() };
                node.reactive_text = e.clone();
                node
            }
            morph_parser::JsxNode::Conditional { condition, then_branch, else_branch, .. } => {
                let mut node = IRNode { node_id: node_id.to_string(), node_type: "__conditional__".into(), ..Default::default() };
                node.condition_expr = condition.clone();
                for (i, c) in then_branch.iter().enumerate() {
                    node.then_nodes.push(self.build_node(c, &format!("{node_id}_then_{i}"), css_rules, depth));
                }
                for (i, c) in else_branch.iter().enumerate() {
                    node.else_nodes.push(self.build_node(c, &format!("{node_id}_else_{i}"), css_rules, depth));
                }
                node
            }
            morph_parser::JsxNode::List { array_expr, key_expr, item_template, .. } => {
                let mut node = IRNode { node_id: node_id.to_string(), node_type: "__list__".into(), ..Default::default() };
                node.list_expr = array_expr.clone();
                node.list_key_expr = key_expr.clone();
                node.item_template = Some(Box::new(self.build_node(item_template, &format!("{node_id}_item"), css_rules, depth)));
                node
            }
        }
    }
}

fn apply_ua_defaults(style: &mut IRStyle, tag: &str) {
    match tag {
        "h1" => { style.font_size = 32.0; style.font_weight = "bold".into(); }
        "h2" => { style.font_size = 24.0; style.font_weight = "bold".into(); }
        "button" => { style.display = "inline-block".into(); style.cursor = "pointer".into(); }
        "input" => { style.display = "inline-block".into(); style.border_width = 1.0; style.border_style = "solid".into(); }
        "img" => { style.display = "inline-block".into(); }
        _ => {}
    }
}

fn selector_matches(tag: &str, props: &std::collections::HashMap<String, morph_parser::JsxPropValue>, selector: &str) -> bool {
    let sel = selector.trim();
    if sel == tag { return true; }
    if sel.starts_with('.') {
        let cls = &sel[1..];
        if let Some(morph_parser::JsxPropValue::String(c)) = props.get("className").or_else(|| props.get("class")) {
            return c.split_whitespace().any(|c| c == cls);
        }
    }
    if sel.starts_with('#') {
        let id = &sel[1..];
        if let Some(morph_parser::JsxPropValue::String(v)) = props.get("id") {
            return v == id;
        }
    }
    false
}

fn apply_css_prop(style: &mut IRStyle, prop: &str, val: &str) {
    if !css_registry::is_known_property(prop) { return; }
    match prop {
        "background-color" | "background" => if let Some(c) = parse_color(val) { style.bg_color = c; },
        "color" => if let Some(c) = parse_color(val) { style.color = c; },
        "width" => if let Some(v) = parse_length(val) { style.width = Some(v); },
        "height" => if let Some(v) = parse_length(val) { style.height = Some(v); },
        "min-width" => if let Some(v) = parse_length(val) { style.min_width = Some(v); },
        "max-width" => if let Some(v) = parse_length(val) { style.max_width = Some(v); },
        "min-height" => if let Some(v) = parse_length(val) { style.min_height = Some(v); },
        "max-height" => if let Some(v) = parse_length(val) { style.max_height = Some(v); },
        "padding" => if let Some(v) = parse_length(val) { style.padding = [v,v,v,v]; },
        "margin" => if let Some(v) = parse_length(val) { style.margin = [v,v,v,v]; },
        "border-radius" => if let Some(v) = parse_length(val) { style.border_radius = v; },
        "font-size" => if let Some(v) = parse_length(val) { style.font_size = v; },
        "font-weight" => style.font_weight = val.to_string(),
        "text-align" => style.text_align = val.to_string(),
        "display" => style.display = val.to_string(),
        "flex-direction" => style.flex_dir = val.to_string(),
        "gap" => if let Some(v) = parse_length(val) { style.gap = v; },
        "position" => style.position = val.to_string(),
        "justify-content" => style.justify_content = val.to_string(),
        "align-items" => style.align_items = val.to_string(),
        "flex-wrap" => style.flex_wrap = val.to_string(),
        "cursor" => style.cursor = val.to_string(),
        "overflow" => style.overflow = val.to_string(),
        "opacity" => if let Ok(v) = val.parse::<f32>() { style.opacity = v; },
        "z-index" => if let Ok(v) = val.parse::<i32>() { style.z_index = Some(v); },
        "border-width" => if let Some(v) = parse_length(val) { style.border_width = v; },
        "border-color" => if let Some(c) = parse_color(val) { style.border_color = c; },
        "border-style" => style.border_style = val.to_string(),
        "box-sizing" => style.box_sizing = val.to_string(),
        _ => {}
    }
}

fn parse_length(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(num) = s.strip_suffix("px") { return num.trim().parse().ok(); }
    if let Some(num) = s.strip_suffix("rem") { return num.trim().parse::<f32>().ok().map(|v| v*16.0); }
    if let Some(num) = s.strip_suffix("em") { return num.trim().parse::<f32>().ok().map(|v| v*16.0); }
    if s.ends_with('%') { return None; }
    s.parse().ok()
}

fn parse_color(s: &str) -> Option<[f32;4]> {
    let s = s.trim().to_lowercase();
    if s.starts_with('#') {
        let hex = s.trim_start_matches('#');
        let (r,g,b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                (r,g,b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r,g,b)
            }
            _ => return None,
        };
        return Some([r as f32/255.0, g as f32/255.0, b as f32/255.0, 1.0]);
    }
    match s.as_str() {
        "transparent" => Some([0.0,0.0,0.0,0.0]),
        "white" => Some([1.0,1.0,1.0,1.0]),
        "black" => Some([0.0,0.0,0.0,1.0]),
        "red" => Some([1.0,0.0,0.0,1.0]),
        "blue" => Some([0.0,0.0,1.0,1.0]),
        _ => None,
    }
}

impl Default for IRBuilder {
    fn default() -> Self { Self::new() }
}
