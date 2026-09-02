use std::collections::HashMap;

use crate::css_registry;
use crate::node::{IRWindow, IRNode, IREvent};
use crate::style::IRStyle;
use crate::tailwind::TailwindResolver;

pub struct IRBuilder {
    tailwind: TailwindResolver,
    counter: std::cell::Cell<usize>,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self { tailwind: TailwindResolver::new(), counter: std::cell::Cell::new(0) }
    }

    /// Assign the next flat `node_NNNN` id (Python-style global counter).
    fn next_id(&self) -> String {
        let n = self.counter.get();
        self.counter.set(n + 1);
        format!("node_{n:04}")
    }

    pub fn build(
        &self,
        source: &morph_parser::MxSource,
        css_rules: &[(String, morph_parser::CssRule)],
        css_keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
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
        // Collect premain functions (inner functions like doLogin, logout) from all components
        let premain: Vec<String> = source.components.iter().flat_map(|c| {
            c.inner_functions.iter().map(|f| f.source.clone()).chain(
                c.consts.iter().map(|cst| format!("auto {} = {};", cst.name, cst.rhs))
            )
        }).collect();
        let mut window = IRWindow {
            window_id: self.next_id(),
            title: wc.map(|w| w.title.clone()).unwrap_or_else(|| "Morph App".into()),
            width: wc.map(|w| w.width).unwrap_or(800),
            height: wc.map(|w| w.height).unwrap_or(600),
            visible: true,
            min_width: wc.and_then(|w| w.min_width),
            max_width: wc.and_then(|w| w.max_width),
            min_height: wc.and_then(|w| w.min_height),
            max_height: wc.and_then(|w| w.max_height),
            modal: wc.map(|w| w.modal).unwrap_or(false),
            renderer: "flash".into(),
            nodes: vec![],
            startup_logs: source.console_logs.clone(),
            premain_functions: premain,
            extra_headers: vec![],
            state_vars: all_state,
            effect_decls: all_effects,
            cpp_imports: source.cpp_imports.iter().map(|ci| {
                let mut m = HashMap::new();
                m.insert("path".into(), ci.path.clone());
                m.insert("specifiers".into(), ci.specifiers.join(", "));
                m
            }).collect(),
            keyframes: self.convert_keyframes(css_keyframes),
        };
        for comp in &source.components {
            let node = self.build_node(&comp.jsx, css_rules, 0);
            window.nodes.push(node);
        }
        windows.push(window);
        windows
    }

    fn build_node(
        &self,
        jsx: &morph_parser::JsxNode,
        css_rules: &[(String, morph_parser::CssRule)],
        depth: usize,
    ) -> IRNode {
        match jsx {
            morph_parser::JsxNode::Element { tag, props, children, line, col, .. } => {
                let node_id = self.next_id();
                let mut node = IRNode {
                    node_id: node_id.clone(),
                    node_type: tag.clone(),
                    ..Default::default()
                };
                let mut style = IRStyle::new();
                apply_ua_defaults(&mut style, tag);
                let mut hover_style = IRStyle::new();
                let mut active_style = IRStyle::new();
                for (selector, rule) in css_rules {
                    match selector_matches(tag, props, selector) {
                        Some(PseudoKind::Base) => {
                            for (prop, val) in &rule.properties {
                                apply_css_prop(&mut style, prop, val);
                            }
                        }
                        Some(PseudoKind::Hover) => {
                            for (prop, val) in &rule.properties {
                                apply_css_prop(&mut hover_style, prop, val);
                            }
                        }
                        Some(PseudoKind::Active) => {
                            for (prop, val) in &rule.properties {
                                apply_css_prop(&mut active_style, prop, val);
                            }
                        }
                        None => {}
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
                            morph_parser::StyleValue::Static(s) => { apply_css_prop(&mut style, prop, s); }
                            morph_parser::StyleValue::Expr(e) => { node.reactive_style.insert(prop.clone(), e.clone()); }
                        }
                    }
                }
                node.style = style;
                if !hover_style.is_empty_style() { node.hover_style = Some(hover_style); }
                if !active_style.is_empty_style() { node.active_style = Some(active_style); }
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
                    let child_node = self.build_node(child, css_rules, depth + 1);
                    if child_node.node_type == "__text__" && child_node.text_content.trim().is_empty() {
                        continue;
                    }
                    node.children.push(child_node);
                }
                node
            }
            morph_parser::JsxNode::Fragment { children, .. } => {
                let mut node = IRNode { node_id: self.next_id(), node_type: "__fragment__".into(), ..Default::default() };
                for child in children.iter() {
                    node.children.push(self.build_node(child, css_rules, depth));
                }
                node
            }
            morph_parser::JsxNode::Text(t) => {
                let mut node = IRNode { node_id: self.next_id(), node_type: "__text__".into(), ..Default::default() };
                node.text_content = t.clone();
                node
            }
            morph_parser::JsxNode::Expression(e) => {
                let mut node = IRNode { node_id: self.next_id(), node_type: "__expr__".into(), ..Default::default() };
                node.reactive_text = e.clone();
                node
            }
            morph_parser::JsxNode::Conditional { condition, then_branch, else_branch, .. } => {
                let mut node = IRNode { node_id: self.next_id(), node_type: "__conditional__".into(), ..Default::default() };
                node.condition_expr = condition.clone();
                for c in then_branch.iter() {
                    node.then_nodes.push(self.build_node(c, css_rules, depth));
                }
                for c in else_branch.iter() {
                    node.else_nodes.push(self.build_node(c, css_rules, depth));
                }
                node
            }
            morph_parser::JsxNode::List { array_expr, key_expr, item_template, .. } => {
                let mut node = IRNode { node_id: self.next_id(), node_type: "__list__".into(), ..Default::default() };
                node.list_expr = array_expr.clone();
                node.list_key_expr = key_expr.clone();
                node.item_template = Some(Box::new(self.build_node(item_template, css_rules, depth)));
                node
            }
        }
    }

    fn convert_keyframes(
        &self,
        css_keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> HashMap<String, Vec<crate::node::IRKeyframe>> {
        let mut result: HashMap<String, Vec<crate::node::IRKeyframe>> = HashMap::new();
        for (name, kfs) in css_keyframes {
            let mut converted = Vec::new();
            for kf in kfs {
                let mut raw: HashMap<String, String> = HashMap::new();
                let mut style = IRStyle::new();
                let mut declared: Vec<String> = Vec::new();
                for (prop, val) in &kf.properties {
                    if !is_animatable(prop) { continue; }
                    if prop == "transform" || needs_layout(val) {
                        raw.insert(prop.clone(), val.clone());
                        continue;
                    }
                    if let Some(field) = apply_css_prop(&mut style, prop, val) {
                        declared.push(field.to_string());
                    }
                }
                converted.push(crate::node::IRKeyframe {
                    offset: kf.offset,
                    style,
                    declared,
                    raw,
                });
            }
            result.insert(name.clone(), converted);
        }
        result
    }
}

/// Which style bucket a matched CSS rule targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PseudoKind {
    Base,
    Hover,
    Active,
}

fn is_animatable(prop: &str) -> bool {
    matches!(
        prop,
        "opacity" | "background-color" | "color" | "border-radius" | "font-size"
            | "width" | "height" | "left" | "top" | "transform"
    )
}

fn needs_layout(val: &str) -> bool {
    let v = val.trim();
    if v.is_empty() || v == "auto" { return true; }
    v.ends_with('%') || v.ends_with("vh") || v.ends_with("vw")
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

/// Match a CSS selector against an element. Returns the style bucket to apply.
///
/// Supports compound selectors (`.btn.ghost`, `button.btn`, `#main.card`) as well
/// as the `:hover` / `:active` pseudo-classes (`button.btn.ghost:hover`).
/// A comma-separated selector list matches if any alternative matches.
fn selector_matches(tag: &str, props: &std::collections::HashMap<String, morph_parser::JsxPropValue>, selector: &str) -> Option<PseudoKind> {
    let classes: Vec<String> = {
        match props.get("className").or_else(|| props.get("class")) {
            Some(morph_parser::JsxPropValue::String(c)) => {
                c.split_whitespace().map(|s| s.to_string()).collect()
            }
            _ => Vec::new(),
        }
    };
    let id = props.get("id").and_then(|v| match v {
        morph_parser::JsxPropValue::String(s) => Some(s.clone()),
        _ => None,
    });

    for alternative in selector.trim().split(',') {
        let alternative = alternative.trim();
        if alternative.is_empty() { continue; }
        // A compound selector may carry a trailing pseudo-class on the last component.
        let (structural, pseudo) = match split_trailing_pseudo(alternative) {
            Some(p) => p,
            None => (alternative, None),
        };
        if !match_selector_compound(tag, &classes, id.as_deref(), structural) {
            continue;
        }
        return Some(pseudo.unwrap_or(PseudoKind::Base));
    }
    None
}

/// Split a leading/trailing `:hover` / `:active` pseudo-class off a simple or
/// compound selector. Only the *last* component's pseudo applies to the element
/// itself; earlier-ancestor pseudos are not handled by this builder.
fn split_trailing_pseudo(sel: &str) -> Option<(&str, Option<PseudoKind>)> {
    let mut pseudo: Option<PseudoKind> = None;
    let mut s = sel;
    loop {
        let t = s.trim_end();
        if let Some(rest) = t.strip_suffix(":hover") {
            pseudo = Some(PseudoKind::Hover);
            s = rest;
        } else if let Some(rest) = t.strip_suffix(":active") {
            pseudo = Some(PseudoKind::Active);
            s = rest;
        } else {
            break;
        }
    }
    Some((s.trim_end(), pseudo))
}

/// Match a single (possibly compound, pseudo-stripped) selector against the
/// element. No combinators/descendant selectors are supported here.
fn match_selector_compound(tag: &str, classes: &[String], id: Option<&str>, sel: &str) -> bool {
    let sel = sel.trim();
    if sel.is_empty() { return false; }
    if sel == "*" { return true; }

    let mut matched_tag = false;
    let mut tag_found = false;
    let mut required_classes: Vec<&str> = Vec::new();
    let mut has_id = false;
    let mut id_ok = true;

    let bytes = sel.as_bytes();
    let mut i = 0;
    let mut buf = String::new();

    while i < bytes.len() {
        let ch = sel[i..].chars().next().unwrap();
        match ch {
            '.' => {
                flush_tag(&mut buf, &mut tag_found, &mut matched_tag, tag);
                i += 1;
                let start = i;
                while i < bytes.len() && !" .#:[]>~+*".contains(sel[i..].chars().next().unwrap()) {
                    i += sel[i..].chars().next().unwrap().len_utf8();
                }
                if i > start { required_classes.push(&sel[start..i]); }
            }
            '#' => {
                flush_tag(&mut buf, &mut tag_found, &mut matched_tag, tag);
                i += 1;
                let start = i;
                while i < bytes.len() && !" .#:[]>~+*".contains(sel[i..].chars().next().unwrap()) {
                    i += sel[i..].chars().next().unwrap().len_utf8();
                }
                has_id = true;
                if id.map(|v| v == &sel[start..i]).unwrap_or(false) {
                    // id matches
                } else {
                    id_ok = false;
                }
            }
            ':' | '[' | '>' | '~' | '+' | ' ' => {
                // Unsupported pseudo/attribute/descendant — remaining structural
                // tail is not a valid element matcher for this builder.
                break;
            }
            _ => {
                buf.push(ch);
                i += 1;
            }
        }
    }
    // Trailing tag text after the last class/id token.
    flush_tag(&mut buf, &mut tag_found, &mut matched_tag, tag);

    if has_id && !id_ok { return false; }
    if tag_found && !matched_tag { return false; }
    for c in required_classes {
        if !classes.iter().any(|cl| cl.as_str() == c) {
            return false;
        }
    }
    true
}

/// Flush a buffered bare tag token (e.g. `button` in `button.btn.ghost`).
fn flush_tag(buf: &mut String, tag_found: &mut bool, matched_tag: &mut bool, tag: &str) {
    let s = buf.trim();
    if !s.is_empty() && s != "*" {
        *tag_found = true;
        if s == tag { *matched_tag = true; }
    }
    buf.clear();
}

/// Apply a CSS property to a style, returning the IR field name that was set
/// (used for `@keyframes` declared-field tracking), or None if unsupported.
fn apply_css_prop(style: &mut IRStyle, prop: &str, val: &str) -> Option<&'static str> {
    if !css_registry::is_known_property(prop) { return None; }
    match prop {
        "background-color" | "background" => if let Some(c) = parse_color(val) { style.bg_color = c; Some("bg_color") } else { None },
        "color" => if let Some(c) = parse_color(val) { style.color = c; Some("color") } else { None },
        "width" => if let Some(v) = parse_length(val) { style.width = Some(v); Some("width") } else { None },
        "height" => if let Some(v) = parse_length(val) { style.height = Some(v); Some("height") } else { None },
        "min-width" => if let Some(v) = parse_length(val) { style.min_width = Some(v); Some("min_width") } else { None },
        "max-width" => if let Some(v) = parse_length(val) { style.max_width = Some(v); Some("max_width") } else { None },
        "min-height" => if let Some(v) = parse_length(val) { style.min_height = Some(v); Some("min_height") } else { None },
        "max-height" => if let Some(v) = parse_length(val) { style.max_height = Some(v); Some("max_height") } else { None },
        "padding" => if let Some(v) = parse_length(val) { style.padding = [v,v,v,v]; Some("padding") } else { None },
        "margin" => if let Some(v) = parse_length(val) { style.margin = [v,v,v,v]; Some("margin") } else { None },
        "border-radius" => if let Some(v) = parse_length(val) { style.border_radius = v; Some("border_radius") } else { None },
        "font-size" => if let Some(v) = parse_length(val) { style.font_size = v; Some("font_size") } else { None },
        "font-weight" => { style.font_weight = val.to_string(); Some("font_weight") }
        "text-align" => { style.text_align = val.to_string(); Some("text_align") }
        "display" => { style.display = val.to_string(); Some("display") }
        "flex-direction" => { style.flex_dir = val.to_string(); Some("flex_dir") }
        "gap" => if let Some(v) = parse_length(val) { style.gap = v; Some("gap") } else { None },
        "position" => { style.position = val.to_string(); Some("position") }
        "justify-content" => { style.justify_content = val.to_string(); Some("justify_content") }
        "align-items" => { style.align_items = val.to_string(); Some("align_items") }
        "flex-wrap" => { style.flex_wrap = val.to_string(); Some("flex_wrap") }
        "cursor" => { style.cursor = val.to_string(); Some("cursor") }
        "overflow" => { style.overflow = val.to_string(); Some("overflow") }
        "opacity" => if let Ok(v) = val.trim().parse::<f32>() { style.opacity = v; Some("opacity") } else { None },
        "z-index" => if let Ok(v) = val.parse::<i32>() { style.z_index = Some(v); Some("z_index") } else { None },
        "border-width" => if let Some(v) = parse_length(val) { style.border_width = v; Some("border_width") } else { None },
        "border-color" => if let Some(c) = parse_color(val) { style.border_color = c; Some("border_color") } else { None },
        "border-style" => { style.border_style = val.to_string(); Some("border_style") }
        "box-sizing" => { style.box_sizing = val.to_string(); Some("box_sizing") }
        _ => None,
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
        let (r,g,b,a) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                (r,g,b,255)
            }
            4 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
                (r,g,b,a)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r,g,b,255)
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                (r,g,b,a)
            }
            _ => return None,
        };
        return Some([r as f32/255.0, g as f32/255.0, b as f32/255.0, a as f32/255.0]);
    }
    if s.starts_with("rgb") {
        return parse_rgb(&s);
    }
    match s.as_str() {
        "transparent" => Some([0.0,0.0,0.0,0.0]),
        "white" => Some([1.0,1.0,1.0,1.0]),
        "black" => Some([0.0,0.0,0.0,1.0]),
        "red" => Some([1.0,0.0,0.0,1.0]),
        "green" => Some([0.0,0.5,0.0,1.0]),
        "blue" => Some([0.0,0.0,1.0,1.0]),
        "gray" | "grey" => Some([0.5,0.5,0.5,1.0]),
        _ => None,
    }
}

/// Parse `rgb(r,g,b)` / `rgba(r,g,b,a)` — components may be ints (0-255) or
/// percentages, and alpha may be a 0..1 float or percentage.
fn parse_rgb(s: &str) -> Option<[f32;4]> {
    let inner = s.find('(')?;
    let end = s.rfind(')')?;
    let args = &s[inner+1..end];
    let parts: Vec<&str> = args.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.len() < 3 { return None; }

    let comp = |p: &str| -> Option<f32> {
        let p = p.trim();
        if let Some(v) = p.strip_suffix('%') {
            Some(v.trim().parse::<f32>().ok()? / 100.0)
        } else {
            Some(p.parse::<f32>().ok()? / 255.0)
        }
    };

    let r = comp(parts[0])?;
    let g = comp(parts[1])?;
    let b = comp(parts[2])?;
    let a = if parts.len() >= 4 {
        let p = parts[3].trim();
        if let Some(v) = p.strip_suffix('%') {
            v.trim().parse::<f32>().ok()? / 100.0
        } else {
            p.parse::<f32>().ok()?
        }
    } else {
        1.0
    };
    Some([r, g, b, a])
}

impl Default for IRBuilder {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morph_parser::JsxPropValue;

    fn match_sel(sel: &str, cls: &[&str]) -> bool {
        let mut props = std::collections::HashMap::new();
        props.insert("className".to_string(), JsxPropValue::String(cls.join(" ")));
        selector_matches("button", &props, sel).is_some()
    }

    #[test]
    fn compound_ghost() {
        assert!(match_sel(".btn.ghost", &["btn", "ghost"]), ".btn.ghost should match btn ghost");
        assert!(!match_sel(".btn.ghost", &["btn"]), ".btn.ghost should NOT match btn only");
        assert!(match_sel(".btn", &["btn", "ghost"]), ".btn should match btn ghost");
        assert!(match_sel(".btn.ghost:hover", &["btn", "ghost"]), "hover compound should match");
    }

    #[test]
    fn transparent_parsed() {
        // lightningcss serializes `background-color: transparent` as #0000 (4-digit)
        // and `rgba(79,123,255,0)` as #4f7cff00 (8-digit). Both must parse to alpha 0.
        let close = |a: Option<[f32; 4]>, b: [f32; 4]| -> bool {
            match a {
                Some(v) => (0..4).all(|i| (v[i] - b[i]).abs() < 0.001),
                None => false,
            }
        };
        assert!(close(parse_color("transparent"), [0.0, 0.0, 0.0, 0.0]));
        assert!(close(parse_color("#0000"), [0.0, 0.0, 0.0, 0.0]));
        assert!(close(parse_color("#4f7cff00"), [0.3098, 0.4863, 1.0, 0.0]));
        assert!(close(parse_color("rgba(79, 124, 255, 0)"), [0.3098, 0.4863, 1.0, 0.0]));
        assert!(close(parse_color("rgba(255,255,255,0.5)"), [1.0, 1.0, 1.0, 0.5]));
        assert!(close(parse_color("rgb(255,0,0)"), [1.0, 0.0, 0.0, 1.0]));
    }
}
