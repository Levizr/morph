use std::collections::HashMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::css_registry;
use crate::node::{IRWindow, IRNode, IREvent};
use crate::style::IRStyle;
use crate::tailwind::TailwindResolver;
use crate::transforms;

pub struct IRBuilder {
    tailwind: TailwindResolver,
    counter: std::cell::Cell<usize>,
    type_mode: morpher::TypeMode,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self {
            tailwind: TailwindResolver::new(),
            counter: std::cell::Cell::new(0),
            type_mode: morpher::TypeMode::default(),
        }
    }

    /// Type mode for translating embedded logic (handlers, effects, globals).
    /// Defaults to inference; `morph build --types` overrides it.
    pub fn with_type_mode(mut self, mode: morpher::TypeMode) -> Self {
        self.type_mode = mode;
        self
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
        // Ambient state map shared by every embedded-logic translation:
        // getters read signals, setters write them, reactive const lambdas
        // re-evaluate. Extended with component consts below, mirroring the
        // order Python builds its per-component translator state.
        let mut ambient_vars: HashMap<String, String> = HashMap::new();
        let mut ambient_types: HashMap<String, String> = HashMap::new();
        let mut all_state: Vec<HashMap<String, String>> = Vec::new();
        for sv in source
            .state_vars
            .iter()
            .chain(source.components.iter().flat_map(|c| c.state_vars.iter()))
        {
            let mut m = HashMap::new();
            m.insert("getter".into(), sv.getter.clone());
            m.insert("setter".into(), sv.setter.clone());
            m.insert("init".into(), sv.init.clone());
            all_state.push(m);
            if !sv.getter.is_empty() {
                ambient_vars.insert(sv.getter.clone(), format!("__st_{}.get()", sv.getter));
                if let Some(ty) = infer_state_type(&sv.init) {
                    ambient_types.insert(sv.getter.clone(), ty);
                }
            }
            if !sv.setter.is_empty() {
                ambient_vars.insert(sv.setter.clone(), format!("__st_{}.set", sv.getter));
            }
        }
        let wc = source.window_config.as_ref();
        let mut extra_headers: Vec<String> = source.extra_headers.clone();
        // ── Module-level logic first (Python: function_declarations, global_vars) ──
        let mut premain: Vec<String> = Vec::new();
        for fd in &source.function_declarations {
            self.push_snippet(
                &mut premain,
                &mut extra_headers,
                &fd.source,
                &ambient_vars,
                &ambient_types,
            );
        }
        for gv in &source.global_vars {
            self.push_snippet(&mut premain, &mut extra_headers, gv, &ambient_vars, &ambient_types);
        }
        // ── Component consts become reactive lambdas; later translations ──
        // see them as `name()` calls (Python: `auto x = []() { return …; };`).
        let mut reactive_consts: Vec<String> = Vec::new();
        for c in &source.components {
            for cst in &c.consts {
                if cst.name.is_empty() || cst.rhs.is_empty() {
                    continue;
                }
                if let Some(out) =
                    self.translate_logic(&cst.rhs, &ambient_vars, &ambient_types)
                {
                    let expr = out.body.trim().trim_end_matches(';').trim();
                    if expr.is_empty() {
                        continue;
                    }
                    extra_headers.extend(include_lines(&out.includes));
                    premain.push(format!(
                        "auto {} = []() {{ return ({}); }};",
                        cst.name, expr
                    ));
                    ambient_vars.insert(cst.name.clone(), format!("{}()", cst.name));
                    reactive_consts.push(cst.name.clone());
                }
            }
        }
        // ── Inner functions (module + component) with the extended map ──
        for f in source
            .inner_functions
            .iter()
            .chain(source.components.iter().flat_map(|c| c.inner_functions.iter()))
        {
            self.push_snippet(
                &mut premain,
                &mut extra_headers,
                &f.source,
                &ambient_vars,
                &ambient_types,
            );
        }
        // ── Effects: transpile callbacks now, emit `create_effect` later ──
        let mut all_effects: Vec<HashMap<String, String>> = Vec::new();
        for e in source
            .effects
            .iter()
            .chain(source.components.iter().flat_map(|c| c.effects.iter()))
        {
            if let Some(out) = self.translate_logic(&e.callback, &ambient_vars, &ambient_types)
            {
                let lambda = out.body.trim().trim_end_matches(';').trim().to_string();
                if lambda.is_empty() {
                    continue;
                }
                extra_headers.extend(include_lines(&out.includes));
                let mut m = HashMap::new();
                m.insert("lambda".into(), lambda);
                m.insert("deps".into(), e.deps.clone());
                all_effects.push(m);
            }
        }
        extra_headers.sort();
        extra_headers.dedup();
        // Startup logs: module logs, then each component's body logs
        // (Python: per-component `body_logs` → window `startup_logs`).
        let mut startup_logs = source.console_logs.clone();
        for c in &source.components {
            for log in &c.console_logs {
                if !startup_logs.contains(log) {
                    startup_logs.push(log.clone());
                }
            }
        }
        let mut nodes = Vec::new();
        for comp in &source.components {
            nodes.push(self.build_node(
                &comp.jsx,
                css_rules,
                0,
                &[],
                &ambient_vars,
                &ambient_types,
                &mut extra_headers,
                css_keyframes,
            ));
        }
        extra_headers.sort();
        extra_headers.dedup();
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
            startup_logs,
            premain_functions: premain,
            extra_headers,
            state_vars: all_state,
            reactive_consts,
            effect_decls: all_effects,
            cpp_imports: source.cpp_imports.iter().map(|ci| {
                let mut m = HashMap::new();
                m.insert("path".into(), ci.path.clone());
                m.insert("specifiers".into(), ci.specifiers.join(", "));
                m
            }).collect(),
            keyframes: self.convert_keyframes(css_keyframes),
        };
        window.nodes = nodes;
        windows.push(window);
        windows
    }

    /// Translate embedded JS/TS against ambient app state. `None` when the
    /// snippet does not parse or has no translatable content (the caller
    /// skips it, mirroring Python's per-block try/except).
    fn translate_logic(
        &self,
        source: &str,
        ambient_vars: &HashMap<String, String>,
        ambient_types: &HashMap<String, String>,
    ) -> Option<morpher::SnippetOutput> {
        let mut options = morpher::TranslateOptions::default();
        options.type_mode = self.type_mode;
        options.state_vars = ambient_vars.clone();
        options.state_types = ambient_types.clone();
        morpher::translate_snippet(source, "snippet.ts", options).ok().filter(|out| {
            !out.body.trim().is_empty()
        })
    }

    /// Translate a statement-level snippet and splice its body into `premain`
    /// with external linkage (mirrors Python's `strip_static_function`).
    fn push_snippet(
        &self,
        premain: &mut Vec<String>,
        extra_headers: &mut Vec<String>,
        source: &str,
        ambient_vars: &HashMap<String, String>,
        ambient_types: &HashMap<String, String>,
    ) {
        if let Some(out) = self.translate_logic(source, ambient_vars, ambient_types) {
            extra_headers.extend(include_lines(&out.includes));
            let body = strip_static_linkage(&out.body);
            if !body.is_empty() {
                premain.push(body);
            }
        }
    }

    fn build_node(
        &self,
        jsx: &morph_parser::JsxNode,
        css_rules: &[(String, morph_parser::CssRule)],
        depth: usize,
        ancestors: &[AncestorHint],
        ambient_vars: &HashMap<String, String>,
        ambient_types: &HashMap<String, String>,
        extra_headers: &mut Vec<String>,
        keyframes: &HashMap<String, Vec<morph_parser::CssKeyframe>>,
    ) -> IRNode {
        match jsx {
            morph_parser::JsxNode::Element { tag, props, children, line: _, col: _, .. } => {
                let node_id = self.next_id();
                let mut node = IRNode {
                    node_id: node_id.clone(),
                    node_type: tag.clone(),
                    ..Default::default()
                };
                let mut style = IRStyle::new();
                apply_ua_defaults(&mut style, tag);
                let mut hover_style = IRStyle::new();
                for (prop, val) in ua_hover_defaults(tag) { apply_css_prop(&mut hover_style, prop, val); }
                let mut active_style = IRStyle::new();
                for (prop, val) in ua_active_defaults(tag) { apply_css_prop(&mut active_style, prop, val); }
                let (classes, id) = element_classes_id(props);
                // Cascade: collect every matching declaration with its
                // specificity and source order, then apply weakest-first so
                // the winner writes last. Stable sort keeps source order
                // among equal specificities (later sheets win ties).
                let mut declarations: Vec<(Specificity, usize, String, String, PseudoKind)> =
                    Vec::new();
                for (order, (selector, rule)) in css_rules.iter().enumerate() {
                    if let Some((pseudo, specificity)) =
                        match_selector_detailed(tag, &classes, id.as_deref(), ancestors, selector)
                    {
                        for (prop, val) in &rule.properties {
                            declarations.push((
                                specificity,
                                order,
                                prop.clone(),
                                val.clone(),
                                pseudo,
                            ));
                        }
                    }
                }
                declarations.sort();
                // Winning declarations per bucket, for animation parsing
                // (Python: matched CSS merged before tailwind/attrs/inline).
                let mut base_props: HashMap<String, String> = HashMap::new();
                let mut hover_props: HashMap<String, String> = HashMap::new();
                for (_, _, prop, val, pseudo) in &declarations {
                    let target = match pseudo {
                        PseudoKind::Base => &mut style,
                        PseudoKind::Hover => &mut hover_style,
                        PseudoKind::Active => &mut active_style,
                    };
                    apply_css_prop(target, prop, val);
                    match pseudo {
                        PseudoKind::Base => {
                            base_props.insert(prop.clone(), val.clone());
                        }
                        PseudoKind::Hover => {
                            hover_props.insert(prop.clone(), val.clone());
                        }
                        PseudoKind::Active => {}
                    }
                }
                if let Some(morph_parser::JsxPropValue::String(cls)) = props.get("className").or_else(|| props.get("class")) {
                    for (prop, val) in self.tailwind.resolve_many(cls) {
                        apply_css_prop(&mut style, &prop, &val);
                        base_props.insert(prop, val);
                    }
                }
                // Presentational hints lose to every stylesheet rule: HTML
                // width/height attributes apply only when the cascade left
                // the property unset (browsers treat them as weakest).
                if style.width.is_none() {
                    if let Some(morph_parser::JsxPropValue::String(raw)) = props.get("width") {
                        if let Some(px) = parse_length(raw) {
                            style.width = Some(px);
                            base_props.insert("width".to_string(), raw.clone());
                        }
                    }
                }
                if style.height.is_none() {
                    if let Some(morph_parser::JsxPropValue::String(raw)) = props.get("height") {
                        if let Some(px) = parse_length(raw) {
                            style.height = Some(px);
                            base_props.insert("height".to_string(), raw.clone());
                        }
                    }
                }
                if let Some(morph_parser::JsxPropValue::Style(map)) = props.get("style") {
                    for (prop, val) in map {
                        match val {
                            morph_parser::StyleValue::Static(s) => {
                                apply_css_prop(&mut style, prop, s);
                                base_props.insert(prop.clone(), s.clone());
                            }
                            morph_parser::StyleValue::Expr(e) => { node.reactive_style.insert(prop.clone(), e.clone()); }
                        }
                    }
                }
                node.style = style;
                if !hover_style.is_empty_style() { node.hover_style = Some(hover_style); }
                if !active_style.is_empty_style() { node.active_style = Some(active_style); }
                // CSS animations from merged declarations; keyframe names
                // unknown to the build are dropped like browsers do.
                node.animations = parse_animations(&base_props)
                    .into_iter()
                    .filter(|anim| keyframes.contains_key(&anim.name))
                    .collect();
                node.hover_animations = parse_animations(&hover_props)
                    .into_iter()
                    .filter(|anim| keyframes.contains_key(&anim.name))
                    .collect();
                for (k, v) in props {
                    if let morph_parser::JsxPropValue::Fn(f) = v {
                        if let Some(trigger) = event_trigger(k) {
                            node.events.push(IREvent {
                                trigger: trigger.into(),
                                action: "call".into(),
                                target: f.clone(),
                            });
                            continue;
                        }
                    }
                    match (k.as_str(), v) {
                        ("id", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("id".into(), s.clone()); }
                        ("src", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("src".into(), s.clone()); }
                        ("placeholder", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("placeholder".into(), s.clone()); }
                        ("type", morph_parser::JsxPropValue::String(s)) => { node.attrs.insert("type".into(), s.clone()); }
                        // Static class strings only drive build-time matching;
                        // only dynamic className={...} becomes a reactive
                        // expression (translated with state at emit time).
                        // Stuffing static strings through JS translation
                        // mangles any word colliding with state (`key op`
                        // with an `op` signal became `key __st_op.get()`).
                        ("className", morph_parser::JsxPropValue::Expr(s))
                        | ("class", morph_parser::JsxPropValue::Expr(s))
                        | ("className", morph_parser::JsxPropValue::Template(s))
                        | ("class", morph_parser::JsxPropValue::Template(s))
                        | ("className", morph_parser::JsxPropValue::Ref(s))
                        | ("class", morph_parser::JsxPropValue::Ref(s)) => {
                            // Runtime class string via the full translator.
                            if let Some(out) =
                                self.translate_logic(s, ambient_vars, ambient_types)
                            {
                                extra_headers.extend(include_lines(&out.includes));
                                let body = out
                                    .body
                                    .trim()
                                    .trim_end_matches(';')
                                    .trim()
                                    .to_string();
                                if !body.is_empty() {
                                    node.reactive_class = body;
                                }
                            }
                            // Build-time branch resolution for ternary arms.
                            let mut fx = analyze_dynamic_class(
                                s,
                                tag,
                                css_rules,
                                &self.tailwind,
                                ambient_vars,
                                ambient_types,
                                self.type_mode,
                                extra_headers,
                            );
                            node.class_conditional_effects.append(&mut fx);
                        }
                        _ => {}
                    }
                }
                let text_parts: Vec<String> = children.iter().filter_map(|c| if let morph_parser::JsxNode::Text(t) = c { Some(t.clone()) } else { None }).collect();
                if !text_parts.is_empty() { node.text_content = text_parts.join(""); }
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.insert(0, AncestorHint {
                    tag: tag.clone(),
                    classes: classes.clone(),
                    id: id.clone(),
                });
                for child in children.iter() {
                    let child_node = self.build_node(
                        child,
                        css_rules,
                        depth + 1,
                        &child_ancestors,
                        ambient_vars,
                        ambient_types,
                        extra_headers,
                        keyframes,
                    );
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
                    node.children.push(self.build_node(
                        child,
                        css_rules,
                        depth,
                        ancestors,
                        ambient_vars,
                        ambient_types,
                        extra_headers,
                        keyframes,
                    ));
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
                    node.then_nodes.push(self.build_node(
                        c,
                        css_rules,
                        depth,
                        ancestors,
                        ambient_vars,
                        ambient_types,
                        extra_headers,
                        keyframes,
                    ));
                }
                for c in else_branch.iter() {
                    node.else_nodes.push(self.build_node(
                        c,
                        css_rules,
                        depth,
                        ancestors,
                        ambient_vars,
                        ambient_types,
                        extra_headers,
                        keyframes,
                    ));
                }
                node
            }
            morph_parser::JsxNode::List { array_expr, key_expr, item_template, .. } => {
                let mut node = IRNode { node_id: self.next_id(), node_type: "__list__".into(), ..Default::default() };
                node.list_expr = array_expr.clone();
                node.list_key_expr = key_expr.clone();
                node.item_template = Some(Box::new(self.build_node(
                    item_template,
                    css_rules,
                    depth,
                    ancestors,
                    ambient_vars,
                    ambient_types,
                    extra_headers,
                    keyframes,
                )));
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

/// User-agent default styles for HTML tags (lowest priority — overridden by
/// every later cascade stage). Mirrors Python's `_UA_DEFAULTS` verbatim; the
/// `color: #010101` near-black sentinel on form controls signals "explicit
/// value, never inherit the parent's color".
fn ua_defaults(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        // ── Document ────────────────────────────────────────────
        "html" => &[("display", "block")],
        "body" => &[("display", "block"), ("padding", "8px")],

        // ── Headings ────────────────────────────────────────────
        "h1" => &[("display", "block"), ("font-size", "32px"), ("font-weight", "bold"), ("margin", "21.44px 0")],
        "h2" => &[("display", "block"), ("font-size", "24px"), ("font-weight", "bold"), ("margin", "19.92px 0")],
        "h3" => &[("display", "block"), ("font-size", "18.72px"), ("font-weight", "bold"), ("margin", "18.72px 0")],
        "h4" => &[("display", "block"), ("font-size", "16px"), ("font-weight", "bold"), ("margin", "21.28px 0")],
        "h5" => &[("display", "block"), ("font-size", "13.28px"), ("font-weight", "bold"), ("margin", "22.18px 0")],
        "h6" => &[("display", "block"), ("font-size", "10.72px"), ("font-weight", "bold"), ("margin", "24.97px 0")],

        // ── Grouping ────────────────────────────────────────────
        "div" => &[("display", "block")],
        "p" => &[("display", "block"), ("margin", "16px 0")],
        "pre" => &[("display", "block"), ("margin", "16px 0")],
        "blockquote" => &[("display", "block"), ("margin", "16px 40px")],
        "hr" => &[("display", "block")],
        "figure" => &[("display", "block"), ("margin", "16px 40px")],
        "figcaption" => &[("display", "block")],
        "main" => &[("display", "block")],
        "header" => &[("display", "block")],
        "footer" => &[("display", "block")],
        "nav" => &[("display", "block")],
        "section" => &[("display", "block")],
        "article" => &[("display", "block")],
        "aside" => &[("display", "block")],

        // ── Lists ───────────────────────────────────────────────
        "ul" => &[("display", "block"), ("margin", "16px 0")],
        "ol" => &[("display", "block"), ("margin", "16px 0")],
        "li" => &[("display", "block")],
        "dl" => &[("display", "block"), ("margin", "16px 0")],
        "dt" => &[("display", "block")],
        "dd" => &[("display", "block"), ("margin-left", "40px")],

        // ── Text-level ──────────────────────────────────────────
        "span" => &[("display", "inline")],
        "a" => &[("display", "inline"), ("color", "#0000ee"), ("cursor", "pointer")],
        "strong" => &[("font-weight", "bold")],
        "b" => &[("font-weight", "bold")],
        "small" => &[("font-size", "13.28px")],
        "mark" => &[("background-color", "#ffff00"), ("color", "#000000")],
        "sub" => &[("font-size", "13.28px")],
        "sup" => &[("font-size", "13.28px")],
        "code" => &[("display", "inline")],
        "kbd" => &[("display", "inline")],
        "samp" => &[("display", "inline")],
        "em" => &[("display", "inline")],
        "i" => &[("display", "inline")],
        "ins" => &[("display", "inline")],
        "u" => &[("display", "inline")],
        "del" => &[("display", "inline")],
        "s" => &[("display", "inline")],
        "q" => &[("display", "inline")],

        // ── Embedded ────────────────────────────────────────────
        "img" => &[("display", "inline-block")],

        // ── Forms ───────────────────────────────────────────────
        "button" => &[
            ("display", "inline-block"),
            ("background-color", "#efefef"),
            ("color", "#010101"),
            ("border-width", "1px"),
            ("border-style", "solid"),
            ("border-color", "#767676"),
            ("border-radius", "4px"),
            ("padding", "1px 6px"),
            ("font-size", "13.33px"),
            ("text-align", "center"),
        ],
        "input" => &[
            ("display", "inline-block"),
            ("background-color", "#ffffff"),
            ("color", "#010101"),
            ("border-width", "1px"),
            ("border-style", "solid"),
            ("border-color", "#767676"),
            ("border-radius", "4px"),
            ("padding", "1px 6px"),
            ("font-size", "13.33px"),
            ("cursor", "text"),
        ],
        "select" => &[("display", "inline-block")],
        "textarea" => &[("display", "inline-block")],
        "label" => &[("display", "inline")],
        "fieldset" => &[("display", "block"), ("border-width", "2px"), ("border-style", "groove"), ("margin", "0 2px"), ("padding", "5px 12px 10px")],
        "legend" => &[("display", "block"), ("padding", "0 2px")],
        "form" => &[("display", "block")],

        // ── Tables ──────────────────────────────────────────────
        "table" => &[("display", "block")],
        "caption" => &[("display", "block")],
        "thead" => &[("display", "block")],
        "tbody" => &[("display", "block")],
        "tfoot" => &[("display", "block")],
        "tr" => &[("display", "block")],
        "td" => &[("display", "block")],
        "th" => &[("display", "block"), ("font-weight", "bold"), ("text-align", "center")],

        // ── Interactive ─────────────────────────────────────────
        "details" => &[("display", "block")],
        "summary" => &[("display", "block")],
        "dialog" => &[("display", "block")],

        _ => &[],
    }
}

/// User-agent default :hover styles (lowest priority — merged FIRST, any
/// matching user `:hover` rule overrides per-property, like browsers).
fn ua_hover_defaults(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        "button" => &[("background-color", "#e6e6e6")],
        _ => &[],
    }
}

/// User-agent default :active styles (pressed state — darker face + border,
/// mirroring the browser's buttonface → buttonhighlight/buttonshadow shift).
fn ua_active_defaults(tag: &str) -> &'static [(&'static str, &'static str)] {
    match tag {
        "button" => &[("background-color", "#d4d4d4"), ("border-color", "#5a5a5a")],
        _ => &[],
    }
}

fn apply_ua_defaults(style: &mut IRStyle, tag: &str) {
    for (prop, val) in ua_defaults(tag) {
        apply_css_prop(style, prop, val);
    }
}

/// Infer a C++ type for a state init literal so snippet translations get
/// operand classes and member-access style for ambient reads. Mirrors the
/// init-based inference in `morph-codegen`'s emitter.
fn infer_state_type(init: &str) -> Option<String> {
    let s = init.trim();
    if s == "true" || s == "false" {
        return Some("bool".to_string());
    }
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        return Some("std::string".to_string());
    }
    if s.starts_with('[') && s.ends_with(']') {
        return Some("JsArray".to_string());
    }
    if s.parse::<i64>().is_ok() {
        return Some("int".to_string());
    }
    if s.parse::<f64>().is_ok() {
        return Some("double".to_string());
    }
    None
}

/// Remove `static`/`static inline` linkage from a translated top-level
/// function so it gets external linkage in the app TU (mirrors Python's
/// `strip_static_function`, visible to user .cpp code).
fn strip_static_linkage(cpp: &str) -> String {
    let mut text = cpp.trim().to_string();
    for prefix in ["static inline", "static"] {
        if text.starts_with(prefix)
            && text[prefix.len()..].chars().next().map(|c| c.is_whitespace()).unwrap_or(false)
        {
            text = text[prefix.len()..].trim_start().to_string();
            break;
        }
    }
    text
}

/// Analyze a dynamic `className` template/expression for ternary branches
/// with string-literal arms (`... ${cond ? "a" : "b"}`). Each branch's
/// classes resolve to CSS declarations at build time (Tailwind + matching
/// stylesheet rules, pseudo rules skipped), and the condition becomes a
/// translated C++ bool expression. Mirrors Python's
/// `_analyze_class_template` / `_analyze_class_expression`.
/// Returns the conditional effects; best-effort (unparseable input yields none).
fn analyze_dynamic_class(
    source: &str,
    tag: &str,
    css_rules: &[(String, morph_parser::CssRule)],
    tailwind: &TailwindResolver,
    ambient_vars: &HashMap<String, String>,
    ambient_types: &HashMap<String, String>,
    type_mode: morpher::TypeMode,
    extra_headers: &mut Vec<String>,
) -> Vec<crate::node::IRConditionalClassEffect> {
    let mut effects = Vec::new();
    let allocator = Allocator::default();
    let source_type = SourceType::from_path("snippet.ts")
        .unwrap_or_default()
        .with_typescript(true);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return effects;
    }
    let first = parsed.program.body.first();
    let expr = match first {
        Some(Statement::ExpressionStatement(stmt)) => &stmt.expression,
        _ => return effects,
    };
    // Collect top-level ternary expressions: the template's ${...} parts
    // plus a bare ternary expression.
    let mut ternaries: Vec<&ConditionalExpression> = Vec::new();
    match expr {
        Expression::TemplateLiteral(tpl) => {
            for part in tpl.expressions.iter() {
                if let Expression::ConditionalExpression(cond) = part {
                    ternaries.push(cond);
                }
            }
        }
        Expression::ConditionalExpression(cond) => ternaries.push(cond),
        _ => {}
    }
    for ternary in ternaries {
        let on_str = string_branch(&ternary.consequent);
        let off_str = string_branch(&ternary.alternate);
        let on_styles = resolve_branch_classes(&on_str, tag, css_rules, tailwind);
        let off_styles = resolve_branch_classes(&off_str, tag, css_rules, tailwind);
        if on_styles.is_empty() && off_styles.is_empty() {
            continue;
        }
        let cond_src = &source[ternary.test.span().start as usize..ternary.test.span().end as usize];
        let mut options = morpher::TranslateOptions::default();
        options.type_mode = type_mode;
        options.state_vars = ambient_vars.clone();
        options.state_types = ambient_types.clone();
        let cond_cpp = morpher::translate_snippet(cond_src, "snippet.ts", options)
            .ok()
            .map(|out| {
                extra_headers.extend(include_lines(&out.includes));
                out.body.trim().trim_end_matches(';').trim().to_string()
            })
            .unwrap_or_default();
        if cond_cpp.is_empty() {
            continue;
        }
        effects.push(crate::node::IRConditionalClassEffect {
            condition: cond_cpp,
            on_styles,
            off_styles,
        });
    }
    effects
}

/// The class string of a ternary branch when it is a plain string literal.
fn string_branch(expr: &Expression) -> String {
    match expr {
        Expression::StringLiteral(lit) => lit.value.to_string(),
        _ => String::new(),
    }
}

/// Resolve a branch's class tokens to CSS declarations: Tailwind first,
/// then matching non-pseudo stylesheet rules (later winners overwrite).
fn resolve_branch_classes(
    class_str: &str,
    tag: &str,
    css_rules: &[(String, morph_parser::CssRule)],
    tailwind: &TailwindResolver,
) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let tokens: Vec<String> = class_str
        .split_whitespace()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    for token in &tokens {
        for (prop, val) in tailwind.resolve(token) {
            out.insert(prop, val);
        }
    }
    for (selector, rule) in css_rules {
        if selector.contains(":hover") || selector.contains(":active") {
            continue;
        }
        if match_selector_detailed(tag, &tokens, None, &[], selector).is_some() {
            for (prop, val) in &rule.properties {
                out.insert(prop.clone(), val.clone());
            }
        }
    }
    out
}

/// Collect bare `#include` specs (`<string>`, `"x.h"`) from a snippet's
/// split-off header block. The app template adds the `#include` keyword
/// itself, mirroring Python's translator `_needed` set.
fn include_lines(header: &str) -> Vec<String> {
    header
        .lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("#include"))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
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

/// Map a JSX event prop to its trigger name. Anything unlisted is not
/// an event the runtime wires, so it falls through to attribute handling.
fn event_trigger(prop: &str) -> Option<&'static str> {
    match prop {
        "onClick" => Some("click"),
        "onInput" => Some("input"),
        "onChange" => Some("change"),
        "onFocus" => Some("focus"),
        "onBlur" => Some("blur"),
        "onKeyUp" => Some("keyup"),
        "onKeyDown" => Some("keydown"),
        "onMouseEnter" => Some("mouseenter"),
        "onMouseLeave" => Some("mouseleave"),
        "onMouseDown" => Some("mousedown"),
        "onMouseUp" => Some("mouseup"),
        _ => None,
    }
}

/// Class list and id of an element, shared by matching and ancestry.
fn element_classes_id(
    props: &std::collections::HashMap<String, morph_parser::JsxPropValue>,
) -> (Vec<String>, Option<String>) {
    let classes = match props.get("className").or_else(|| props.get("class")) {
        Some(morph_parser::JsxPropValue::String(c)) => {
            c.split_whitespace().map(|s| s.to_string()).collect()
        }
        _ => Vec::new(),
    };
    let id = props.get("id").and_then(|v| match v {
        morph_parser::JsxPropValue::String(s) => Some(s.clone()),
        _ => None,
    });
    (classes, id)
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

/// One ancestor step for descendant-selector matching.
#[derive(Clone, Default)]
struct AncestorHint {
    tag: String,
    classes: Vec<String>,
    id: Option<String>,
}

/// Selector specificity as (ids, classes, tags): higher wins regardless
/// of source order, matching browser cascade. Pseudo-classes count as
/// classes. `!important` is not tracked (the parser merges it away), and
/// sibling combinators are unsupported (see below).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
struct Specificity(u32, u32, u32);

/// Count specificity of one comma-free alternative: `#` per id, `.` and
/// `:` runs per class/pseudo-class, bare leading words per tag.
fn selector_specificity(alternative: &str) -> Specificity {
    let mut ids = 0;
    let mut classes = 0;
    let mut tags = 0;
    let mut word_start = true;
    let mut chars = alternative.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '#' => {
                ids += 1;
                word_start = false;
            }
            '.' => {
                classes += 1;
                word_start = false;
            }
            ':' => {
                if chars.peek() == Some(&':') {
                    chars.next();
                }
                classes += 1;
                word_start = false;
            }
            '[' => {
                classes += 1;
                word_start = false;
            }
            ' ' | '>' | '+' | '~' | ',' | '*' => {
                word_start = true;
            }
            _ => {
                if word_start && ch.is_alphabetic() {
                    tags += 1;
                }
                word_start = false;
            }
        }
    }
    Specificity(ids, classes, tags)
}

/// How one compound attaches to the previous one. Sibling combinators
/// (`+`, `~`) have no meaning in a flat single-node walk.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Combinator {
    Descendant,
    Child,
}

/// Split `div > p` into per-compound steps with their combinators.
/// `None` for sibling combinators and attribute selectors: ignoring the
/// rule beats applying it to the wrong element.
fn split_selector_sequence(selector: &str) -> Option<Vec<(Option<Combinator>, String)>> {
    if selector.contains('[') {
        return None;
    }
    let mut steps: Vec<(Option<Combinator>, String)> = Vec::new();
    let mut current = String::new();
    let mut pending = None;
    let mut chars = selector.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' => {
                if !current.trim().is_empty() {
                    steps.push((pending.take(), current.trim().to_string()));
                    current = String::new();
                }
                if pending.is_none() {
                    pending = Some(Combinator::Descendant);
                }
            }
            '>' => {
                if !current.trim().is_empty() {
                    steps.push((pending.take(), current.trim().to_string()));
                    current = String::new();
                }
                pending = Some(Combinator::Child);
            }
            '+' | '~' => {
                return None;
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.trim().is_empty() {
        steps.push((pending.take(), current.trim().to_string()));
    }
    if steps.is_empty() {
        return None;
    }
    Some(steps)
}

/// Match full steps right-to-left: the last compound hits the element,
/// the rest walk the ancestor chain (`ancestors[0]` is the parent).
fn match_sequence(
    tag: &str,
    classes: &[String],
    id: Option<&str>,
    ancestors: &[AncestorHint],
    steps: &[(Option<Combinator>, String)],
) -> bool {
    let Some((_, last)) = steps.last() else {
        return false;
    };
    if !match_selector_compound(tag, classes, id, last) {
        return false;
    }
    let mut ancestor_at = 0;
    for index in (1..steps.len()).rev() {
        let compound = &steps[index - 1].1;
        match steps[index].0 {
            None | Some(Combinator::Child) => {
                let Some(ancestor) = ancestors.get(ancestor_at) else {
                    return false;
                };
                if !match_selector_compound(
                    &ancestor.tag,
                    &ancestor.classes,
                    ancestor.id.as_deref(),
                    compound,
                ) {
                    return false;
                }
                ancestor_at += 1;
            }
            Some(Combinator::Descendant) => {
                let mut found = false;
                while let Some(ancestor) = ancestors.get(ancestor_at) {
                    ancestor_at += 1;
                    if match_selector_compound(
                        &ancestor.tag,
                        &ancestor.classes,
                        ancestor.id.as_deref(),
                        compound,
                    ) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
        }
    }
    true
}

/// Match with specificity of the winning alternative. Unknown pseudos
/// and attribute selectors never match (unsupported, ignored); among
/// matching alternatives the most specific one counts, not the first.
fn match_selector_detailed(
    tag: &str,
    classes: &[String],
    id: Option<&str>,
    ancestors: &[AncestorHint],
    selector: &str,
) -> Option<(PseudoKind, Specificity)> {
    let mut best: Option<(PseudoKind, Specificity)> = None;
    for alternative in selector.trim().split(',') {
        let alternative = alternative.trim();
        if alternative.is_empty() {
            continue;
        }
        let (structural, pseudo) = match split_trailing_pseudo(alternative) {
            Some(split) => split,
            None => (alternative, None),
        };
        if structural.contains(':') || structural.contains('[') {
            continue;
        }
        let Some(steps) = split_selector_sequence(structural) else {
            continue;
        };
        if match_sequence(tag, classes, id, ancestors, &steps) {
            let specificity = selector_specificity(alternative);
            let better = best.map(|(_, held)| specificity > held).unwrap_or(true);
            if better {
                best = Some((pseudo.unwrap_or(PseudoKind::Base), specificity));
            }
        }
    }
    best
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
        "padding" => if let Some(v) = parse_box_sides(val) { style.padding = v; Some("padding") } else { None },
        "margin" => if let Some(v) = parse_box_sides(val) { style.margin = v; Some("margin") } else { None },
        "border-radius" => if let Some(v) = parse_length(val) { style.border_radius = v; Some("border_radius") } else { None },
        "font-size" => if let Some(v) = parse_length(val) { style.font_size = v; Some("font_size") } else { None },
        "font-weight" => { style.font_weight = val.to_string(); Some("font_weight") }
        "text-align" => { style.text_align = val.to_string(); Some("text_align") }
        "display" => { style.display = val.to_string(); Some("display") }
        "flex-direction" => { style.flex_dir = val.to_string(); Some("flex_dir") }
        "gap" => if let Some(v) = parse_length(val) { style.gap = v; Some("gap") } else { None },
        "position" => { style.position = val.to_string(); Some("position") }
        "left" => {
            style.left = parse_length(val);
            if style.left.is_some() { Some("left") } else { None }
        }
        "right" => {
            style.right = parse_length(val);
            if style.right.is_some() { Some("right") } else { None }
        }
        "top" => {
            style.top = parse_length(val);
            if style.top.is_some() { Some("top") } else { None }
        }
        "bottom" => {
            style.bottom = parse_length(val);
            if style.bottom.is_some() { Some("bottom") } else { None }
        }
        "justify-content" => { style.justify_content = val.to_string(); Some("justify_content") }
        "align-items" => { style.align_items = val.to_string(); Some("align_items") }
        "flex-wrap" => { style.flex_wrap = val.to_string(); Some("flex_wrap") }
        "flex-grow" => if let Ok(v) = val.trim().parse::<f32>() {
            style.flex_grow = v; Some("flex_grow")
        } else { None },
        "flex-shrink" => if let Ok(v) = val.trim().parse::<f32>() {
            style.flex_shrink = v; Some("flex_shrink")
        } else { None },
        "flex-basis" => {
            let v = val.trim();
            style.flex_basis = if v == "auto" {
                "auto".to_string()
            } else if let Some(px) = parse_length(v) {
                format!("{}px", px)
            } else {
                v.to_string()
            };
            Some("flex_basis")
        }
        "flex" => { parse_flex_shorthand(&mut *style, val); Some("flex") }
        "border" => { parse_border_shorthand(&mut *style, val); Some("border") }
        "margin-top" => if let Some(v) = parse_length(val) {
            style.margin[0] = v; Some("margin")
        } else { None },
        "margin-right" => if let Some(v) = parse_length(val) {
            style.margin[1] = v; Some("margin")
        } else { None },
        "margin-bottom" => if let Some(v) = parse_length(val) {
            style.margin[2] = v; Some("margin")
        } else { None },
        "margin-left" => if let Some(v) = parse_length(val) {
            style.margin[3] = v; Some("margin")
        } else { None },
        "padding-top" => if let Some(v) = parse_length(val) {
            style.padding[0] = v; Some("padding")
        } else { None },
        "padding-right" => if let Some(v) = parse_length(val) {
            style.padding[1] = v; Some("padding")
        } else { None },
        "padding-bottom" => if let Some(v) = parse_length(val) {
            style.padding[2] = v; Some("padding")
        } else { None },
        "padding-left" => if let Some(v) = parse_length(val) {
            style.padding[3] = v; Some("padding")
        } else { None },
        "cursor" => { style.cursor = val.to_string(); Some("cursor") }
        "overflow" => { style.overflow = val.to_string(); Some("overflow") }
        "opacity" => if let Ok(v) = val.trim().parse::<f32>() { style.opacity = v; Some("opacity") } else { None },
        "transform" => {
            // Resolve at build time so the emitted style carries a concrete
            // matrix (Python resolves via its layout engine in dev; prod leaves
            // it unresolved, so Rust is strictly a superset here).
            match transforms::parse_transform(val) {
                Some(ops) => {
                    style.transform_ops = Some(ops.clone());
                    if !ops.is_empty() {
                        style.transform_matrix =
                            Some(transforms::compose_transform(&ops, 0.0, 0.0));
                    }
                    Some("transform")
                }
                None => None,
            }
        }
        "transform-origin" => {
            match transforms::parse_transform_origin(val) {
                Some((raw, resolved)) => {
                    style.transform_origin = Some(raw);
                    style.transform_origin_resolved = resolved;
                    Some("transform_origin")
                }
                None => None,
            }
        }
        "z-index" => if let Ok(v) = val.parse::<i32>() { style.z_index = Some(v); Some("z_index") } else { None },
        "border-width" => if let Some(v) = parse_length(val) { style.border_width = v; Some("border_width") } else { None },
        "border-color" => if let Some(c) = parse_color(val) { style.border_color = c; Some("border_color") } else { None },
        "border-style" => { style.border_style = val.to_string(); Some("border_style") }
        "box-sizing" => { style.box_sizing = val.to_string(); Some("box_sizing") }
        _ => None,
    }
}

/// Parse the CSS `flex` shorthand into grow/shrink/basis.
/// Mirrors Python `_parse_flex_shorthand`.
fn parse_flex_shorthand(style: &mut IRStyle, val: &str) {
    let kw = val.trim();
    let parts: Vec<&str> = kw.split_whitespace().collect();
    match kw {
        "none" => {
            style.flex_grow = 0.0;
            style.flex_shrink = 0.0;
            style.flex_basis = "auto".to_string();
        }
        "auto" => {
            style.flex_grow = 1.0;
            style.flex_shrink = 1.0;
            style.flex_basis = "auto".to_string();
        }
        "initial" => {
            style.flex_grow = 0.0;
            style.flex_shrink = 1.0;
            style.flex_basis = "auto".to_string();
        }
        _ => match parts.len() {
            1 => {
                if let Ok(v) = parts[0].parse::<f32>() {
                    style.flex_grow = v;
                    style.flex_shrink = 1.0;
                    style.flex_basis = "0%".to_string();
                }
            }
            2 => {
                if let (Ok(g), Ok(s)) =
                    (parts[0].parse::<f32>(), parts[1].parse::<f32>())
                {
                    style.flex_grow = g;
                    style.flex_shrink = s;
                    style.flex_basis = "0%".to_string();
                }
            }
            _ => {
                if parts.len() >= 3 {
                    if let (Ok(g), Ok(s)) =
                        (parts[0].parse::<f32>(), parts[1].parse::<f32>())
                    {
                        style.flex_grow = g;
                        style.flex_shrink = s;
                        style.flex_basis = parts[2].to_string();
                    }
                }
            }
        },
    }
}

/// Split the CSS `border` shorthand (`1px solid #232b3d`) into
/// width/style/color. Mirrors Python's `_css_to_ir_kw` branch.
fn parse_border_shorthand(style: &mut IRStyle, val: &str) {
    for part in val.split_whitespace() {
        if matches!(part, "solid" | "dashed" | "dotted" | "none") {
            style.border_style = part.to_string();
        } else if part.starts_with('#')
            || part.starts_with("rgb")
            || part == "transparent"
        {
            if let Some(c) = parse_color(part) {
                style.border_color = c;
            }
        } else if let Some(w) = parse_length(part) {
            style.border_width = w;
        }
    }
}

/// Parse CSS 1-4 value box shorthand (`10px`, `6px 12px`, ...) into
/// [top, right, bottom, left], mirroring Python's per-side conversion.
fn parse_box_sides(s: &str) -> Option<[f32; 4]> {
    let parts: Vec<Option<f32>> = s.split_whitespace().map(parse_length).collect();
    if parts.is_empty() || parts.len() > 4 || parts.iter().any(|p| p.is_none()) {
        return None;
    }
    let v: Vec<f32> = parts.into_iter().map(|p| p.unwrap_or(0.0)).collect();
    Some(match v.len() {
        1 => [v[0], v[0], v[0], v[0]],
        2 => [v[0], v[1], v[0], v[1]],
        3 => [v[0], v[1], v[2], v[1]],
        _ => [v[0], v[1], v[2], v[3]],
    })
}

/// Parse a CSS time (`0.3s` / `500ms` / unitless seconds) to seconds.
fn parse_css_time(raw: &str) -> Option<f32> {
    let s = raw.trim().to_lowercase();
    if let Some(ms) = s.strip_suffix("ms") {
        return ms.trim().parse::<f32>().ok().map(|v| v / 1000.0);
    }
    if let Some(sec) = s.strip_suffix('s') {
        return sec.trim().parse::<f32>().ok();
    }
    s.parse::<f32>().ok()
}

fn map_easing(low: &str) -> Option<&'static str> {
    // The runtime has no separate `ease` curve: it is ease-in-out,
    // mirroring Python's _EASING_KEYWORDS.
    Some(match low {
        "linear" => "linear",
        "ease" | "ease-in-out" => "ease-in-out",
        "ease-in" => "ease-in",
        "ease-out" => "ease-out",
        _ => return None,
    })
}

/// Split `animation: a, b` on top-level commas (ignores parens).
fn split_animation_list(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in raw.chars() {
        match ch {
            '(' => {
                depth += 1;
                cur.push(ch);
            }
            ')' => {
                depth -= 1;
                cur.push(ch);
            }
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    parts.push(cur.trim().to_string());
                }
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

/// Parse one comma-separated `animation` shorthand value. Mirrors Python's
/// `_parse_animation_component` (first time = duration, second = delay,
/// bare numbers = iteration count, first unclassified token = name).
fn parse_animation_component(raw: &str) -> crate::node::IRAnimation {
    let mut anim = crate::node::IRAnimation::default();
    let mut unclassified: Vec<String> = Vec::new();
    for tok in raw.split_whitespace() {
        let low = tok.to_lowercase();
        if let Some(easing) = map_easing(&low) {
            anim.easing = easing.to_string();
        } else if matches!(low.as_str(), "normal" | "reverse" | "alternate" | "alternate-reverse") {
            anim.direction = low;
        } else if matches!(low.as_str(), "none" | "forwards" | "backwards" | "both") {
            anim.fill_mode = low;
        } else if matches!(low.as_str(), "running" | "paused") {
            anim.play_state = low;
        } else if low == "infinite" {
            anim.iterations = -1.0;
        } else if low.parse::<f32>().is_ok() {
            // Fractional counts (2.5) must not misparse as times.
            anim.iterations = low.parse::<f32>().unwrap_or(1.0);
        } else if let Some(t) = parse_css_time(&low) {
            if anim.duration == 0.0 {
                anim.duration = t;
            } else {
                anim.delay = t;
            }
        } else if !low.contains('(') {
            // Unsupported easing functions (cubic-bezier, steps) ignored.
            unclassified.push(tok.to_string());
        }
    }
    if !unclassified.is_empty() {
        anim.name = unclassified.into_iter().next().unwrap_or_default();
    }
    anim
}

fn parse_animation_shorthand(raw: &str) -> Vec<crate::node::IRAnimation> {
    split_animation_list(raw)
        .iter()
        .map(|part| parse_animation_component(part))
        .filter(|anim| !anim.name.is_empty())
        .collect()
}

const ANIMATION_LONGHANDS: &[&str] = &[
    "animation-name",
    "animation-duration",
    "animation-timing-function",
    "animation-delay",
    "animation-iteration-count",
    "animation-direction",
    "animation-fill-mode",
    "animation-play-state",
];

fn apply_animation_longhand(
    anim: &mut crate::node::IRAnimation,
    prop: &str,
    value: &str,
) -> bool {
    let val = value.trim().to_lowercase();
    match prop {
        "animation-name" => anim.name = val,
        "animation-duration" => {
            anim.duration = parse_css_time(&val).unwrap_or(anim.duration);
            if parse_css_time(&val).is_none() {
                return false;
            }
        }
        "animation-timing-function" => {
            if let Some(easing) = map_easing(&val) {
                anim.easing = easing.to_string();
            }
        }
        "animation-delay" => {
            if parse_css_time(&val).is_none() {
                return false;
            }
            anim.delay = parse_css_time(&val).unwrap_or(anim.delay);
        }
        "animation-iteration-count" => {
            if val == "infinite" {
                anim.iterations = -1.0;
            } else if let Ok(n) = val.parse::<f32>() {
                anim.iterations = n;
            } else {
                return false;
            }
        }
        "animation-direction" => {
            if matches!(val.as_str(), "normal" | "reverse" | "alternate" | "alternate-reverse") {
                anim.direction = val;
            }
        }
        "animation-fill-mode" => {
            if matches!(val.as_str(), "none" | "forwards" | "backwards" | "both") {
                anim.fill_mode = val;
            }
        }
        "animation-play-state" => {
            if matches!(val.as_str(), "running" | "paused") {
                anim.play_state = val;
            }
        }
        _ => {}
    }
    true
}

/// Build a node's animation list from merged CSS declarations: the
/// `animation` shorthand first, then longhands as per-index overrides
/// (CSS list semantics: the last value repeats). Animations without a
/// name are dropped; play-state alone never creates one.
fn parse_animations(merged: &HashMap<String, String>) -> Vec<crate::node::IRAnimation> {
    let mut anims: Vec<crate::node::IRAnimation> = merged
        .get("animation")
        .map(|raw| parse_animation_shorthand(raw))
        .unwrap_or_default();
    let mut longhands: Vec<(&str, Vec<String>)> = Vec::new();
    for prop in ANIMATION_LONGHANDS {
        if let Some(raw) = merged.get(*prop) {
            longhands.push((prop, split_animation_list(raw)));
        }
    }
    if longhands.is_empty() {
        return anims.into_iter().filter(|a| !a.name.is_empty()).collect();
    }
    let count = longhands
        .iter()
        .map(|(_, values)| values.len())
        .max()
        .unwrap_or(0)
        .max(anims.len());
    while anims.len() < count {
        anims.push(crate::node::IRAnimation::default());
    }
    for (prop, values) in &longhands {
        for (i, anim) in anims.iter_mut().enumerate().take(count) {
            let val = values.get(i).or_else(|| values.last());
            if let Some(val) = val {
                apply_animation_longhand(anim, prop, val);
            }
        }
    }
    anims.into_iter().filter(|a| !a.name.is_empty()).collect()
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
        let (classes, id) = element_classes_id(&props);
        match_selector_detailed("button", &classes, id.as_deref(), &[], sel).is_some()
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

    #[test]
    fn specificity_orders_id_over_class_over_tag() {
        assert!(selector_specificity("#a") > selector_specificity(".b.c.d"));
        assert!(selector_specificity(".b") > selector_specificity("div"));
        assert!(selector_specificity("div p") > selector_specificity("p"));
        assert_eq!(selector_specificity(".a"), selector_specificity(".b"));
    }

    fn detailed(
        tag: &str,
        classes: &[&str],
        ancestors: &[(&str, &[&str])],
        selector: &str,
    ) -> Option<(PseudoKind, Specificity)> {
        let owned_classes: Vec<String> = classes.iter().map(|s| s.to_string()).collect();
        let owned_ancestors: Vec<AncestorHint> = ancestors
            .iter()
            .map(|(tag, classes)| AncestorHint {
                tag: tag.to_string(),
                classes: classes.iter().map(|s| s.to_string()).collect(),
                id: None,
            })
            .collect();
        match_selector_detailed(tag, &owned_classes, None, &owned_ancestors, selector)
    }

    #[test]
    fn descendant_matches_through_ancestors() {
        let wrap: &[&str] = &["wrap"];
        let ancestors = [("div", wrap)];
        assert!(detailed("p", &[], &ancestors, "div p").is_some());
        assert!(detailed("p", &[], &[], "div p").is_none());
        assert!(detailed("span", &[], &ancestors, "div > span").is_some());
    }

    #[test]
    fn sibling_combinators_never_match() {
        let empty: &[&str] = &[];
        let ancestors = [("h2", empty)];
        assert!(detailed("p", &[], &ancestors, "h2 + p").is_none());
        assert!(detailed("p", &[], &ancestors, "h2 ~ p").is_none());
    }

    #[test]
    fn grouped_alternatives_use_matching_specificity() {
        let matched = detailed("p", &["note"], &[], ".note, #other").unwrap();
        assert_eq!(matched.1, selector_specificity(".note"));
    }

    #[test]
    fn width_attribute_loses_to_stylesheet() {
        let builder = IRBuilder::new();
        let mut props = std::collections::HashMap::new();
        props.insert("width".to_string(), JsxPropValue::String("400".to_string()));
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "img".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert_eq!(node.style.width, Some(400.0));

        let mut props = std::collections::HashMap::new();
        props.insert("width".to_string(), JsxPropValue::String("400".to_string()));
        let rules = vec![(
            ".wide".to_string(),
            morph_parser::CssRule {
                selector: ".wide".to_string(),
                properties: [("width".to_string(), "100px".to_string())]
                    .into_iter()
                    .collect(),
            },
        )];
        props.insert("className".to_string(), JsxPropValue::String("wide".to_string()));
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "img".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &rules,
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &HashMap::new(),
        );
        assert_eq!(node.style.width, Some(100.0));
    }

    #[test]
    fn embedded_logic_transpiles_to_cpp_premain() {
        use morph_parser::{
            ComponentConst, InnerFunction, MxComponent, MxEffect, MxSource, StateVar,
        };
        let source = MxSource {
            filename: "app.mx".to_string(),
            imports: Vec::new(),
            window_config: None,
            components: vec![MxComponent {
                name: "App".to_string(),
                exported: true,
                params: Vec::new(),
                jsx: morph_parser::JsxNode::Text("hi".to_string()),
                state_vars: vec![StateVar {
                    getter: "count".to_string(),
                    setter: "setCount".to_string(),
                    init: "0".to_string(),
                }],
                effects: vec![
                    MxEffect {
                        callback: "() => { console.log(count); }".to_string(),
                        deps: "[]".to_string(),
                    },
                    MxEffect {
                        callback: "() => { console.log(count); }".to_string(),
                        deps: "[count]".to_string(),
                    },
                ],
                inner_functions: vec![InnerFunction {
                    name: "doLogin".to_string(),
                    source: "function doLogin() { setCount(count + 1); }".to_string(),
                }],
                consts: vec![ComponentConst {
                    name: "doubled".to_string(),
                    rhs: "count * 2".to_string(),
                }],
                console_logs: vec!["body log".to_string()],
            }],
            state_vars: Vec::new(),
            effects: Vec::new(),
            inner_functions: Vec::new(),
            function_declarations: vec![InnerFunction {
                name: "helper".to_string(),
                source: "function helper() { return 1; }".to_string(),
            }],
            global_vars: vec!["const API_URL = \"https://api.test\";".to_string()],
            console_logs: vec!["module log".to_string()],
            extra_headers: Vec::new(),
            cpp_imports: Vec::new(),
        };
        let windows = IRBuilder::new().build(&source, &[], &HashMap::new());
        assert_eq!(windows.len(), 1);
        let win = &windows[0];
        let premain = win.premain_functions.join("\n");
        // Raw JS must never reach the app TU: functions are transpiled and
        // stripped of internal linkage, consts become reactive lambdas.
        assert!(premain.contains("void doLogin()"), "handler transpiled: {}", premain);
        assert!(premain.contains("auto helper"), "module fn transpiled: {}", premain);
        assert!(premain.contains("API_URL"), "global transpiled: {}", premain);
        assert!(!premain.contains("function "), "no raw JS: {}", premain);
        assert!(
            premain.contains("auto doubled = []() { return ("),
            "const is reactive lambda: {}",
            premain
        );
        assert!(
            premain.contains("__st_count.get()"),
            "ambient state mapped: {}",
            premain
        );
        assert!(!premain.contains("static "), "external linkage: {}", premain);
        assert_eq!(win.reactive_consts, vec!["doubled".to_string()]);
        // Effects carry transpiled lambdas, not JS callbacks.
        assert_eq!(win.effect_decls.len(), 2);
        assert!(win.effect_decls[0].get("lambda").unwrap().starts_with('['));
        assert!(!win.effect_decls[0].get("lambda").unwrap().contains("=>"));
        assert_eq!(win.effect_decls[0].get("deps").unwrap(), "[]");
        assert_eq!(win.effect_decls[1].get("deps").unwrap(), "[count]");
        // Snippet headers (e.g. <print> for console.log) merge upward.
        assert!(win.extra_headers.iter().any(|h| h.contains("print")), "{:?}", win.extra_headers);
        // Logs merge: module first, then component body.
        assert_eq!(win.startup_logs, vec!["module log".to_string(), "body log".to_string()]);
    }

    #[test]
    fn box_shorthands_expand_per_side() {
        assert_eq!(parse_box_sides("18px"), Some([18.0, 18.0, 18.0, 18.0]));
        assert_eq!(parse_box_sides("10px 6px"), Some([10.0, 6.0, 10.0, 6.0]));
        assert_eq!(
            parse_box_sides("36px 32px 28px 32px"),
            Some([36.0, 32.0, 28.0, 32.0])
        );
        assert_eq!(parse_box_sides("1px 2px 3px"), Some([1.0, 2.0, 3.0, 2.0]));
        assert_eq!(parse_box_sides("10px auto"), None);
        assert_eq!(parse_box_sides(""), None);
    }

    #[test]
    fn flex_and_border_shorthands_map() {
        let builder = IRBuilder::new();
        let mut style = IRStyle::default();
        apply_css_prop(&mut style, "flex-grow", "1");
        apply_css_prop(&mut style, "flex-shrink", "0");
        assert_eq!(style.flex_grow, 1.0);
        assert_eq!(style.flex_shrink, 0.0);
        apply_css_prop(&mut style, "flex", "2");
        assert_eq!(style.flex_grow, 2.0);
        assert_eq!(style.flex_shrink, 1.0);
        assert_eq!(style.flex_basis, "0%");
        apply_css_prop(&mut style, "border", "1px solid #232b3d");
        assert_eq!(style.border_width, 1.0);
        assert_eq!(style.border_style, "solid");
        assert!(style.border_color[0] > 0.1 && style.border_color[0] < 0.2);
        apply_css_prop(&mut style, "margin-top", "50px");
        apply_css_prop(&mut style, "padding-left", "6px");
        assert_eq!(style.margin[0], 50.0);
        assert_eq!(style.padding[3], 6.0);
        let _ = builder;
    }

    #[test]
    fn static_class_names_stay_out_of_reactive_class() {
        // `key op` with an `op` signal must survive verbatim; only
        // className={...} becomes a reactive expression.
        let builder = IRBuilder::new();
        let mut props = std::collections::HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::String("key op".to_string()),
        );
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "button".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
                &HashMap::new(),
                &HashMap::new(),
                &mut Vec::new(),
            &HashMap::new(),
        );
        assert!(node.reactive_class.is_empty());
        let mut props = std::collections::HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::Expr("op === 1 ? \"a\" : \"b\"".to_string()),
        );
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "button".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
                &HashMap::new(),
                &HashMap::new(),
                &mut Vec::new(),
            &HashMap::new(),
        );
        assert!(!node.reactive_class.is_empty());
    }

    #[test]
    fn template_className_analyzes_ternary_branches() {
        use morph_parser::{MxComponent, MxSource, StateVar};
        let mut props = std::collections::HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::Template(
                "`header ${theme == \"light\" ? \"bg-white\" : \"bg-gray-900\"}`".to_string(),
            ),
        );
        let source = MxSource {
            filename: "app.mx".to_string(),
            imports: Vec::new(),
            window_config: None,
            components: vec![MxComponent {
                name: "App".to_string(),
                exported: true,
                params: Vec::new(),
                jsx: morph_parser::JsxNode::Element {
                    tag: "div".to_string(),
                    props,
                    children: Vec::new(),
                    self_closing: true,
                    line: 0,
                    col: 0,
                },
                state_vars: vec![StateVar {
                    getter: "theme".to_string(),
                    setter: "setTheme".to_string(),
                    init: "\"light\"".to_string(),
                }],
                effects: Vec::new(),
                inner_functions: Vec::new(),
                consts: Vec::new(),
                console_logs: Vec::new(),
            }],
            state_vars: Vec::new(),
            effects: Vec::new(),
            inner_functions: Vec::new(),
            function_declarations: Vec::new(),
            global_vars: Vec::new(),
            console_logs: Vec::new(),
            extra_headers: Vec::new(),
            cpp_imports: Vec::new(),
        };
        let windows = IRBuilder::new().build(&source, &[], &HashMap::new());
        let node = &windows[0].nodes[0];
        assert_eq!(node.class_conditional_effects.len(), 1);
        let fx = &node.class_conditional_effects[0];
        assert!(fx.condition.contains("__st_theme"), "cond mapped: {}", fx.condition);
        assert_eq!(
            fx.on_styles.get("background-color").map(String::as_str),
            Some("#ffffff")
        );
        assert_eq!(
            fx.off_styles.get("background-color").map(String::as_str),
            Some("#111827")
        );
        assert!(!node.reactive_class.is_empty());
    }

    #[test]
    fn animation_shorthand_parses_and_filters_unknown_keyframes() {
        use morph_parser::{CssKeyframe, CssRule};
        let rules = vec![(
            ".pulse".to_string(),
            CssRule {
                selector: ".pulse".to_string(),
                properties: [(
                    "animation".to_string(),
                    "pulse 2s ease-in-out infinite".to_string(),
                )]
                .into_iter()
                .collect(),
            },
        )];
        let mut keyframes = HashMap::new();
        keyframes.insert("pulse".to_string(), Vec::<CssKeyframe>::new());
        let mut props = std::collections::HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::String("pulse".to_string()),
        );
        let jsx = morph_parser::JsxNode::Element {
            tag: "div".to_string(),
            props,
            children: Vec::new(),
            self_closing: true,
            line: 0,
            col: 0,
        };
        let builder = IRBuilder::new();
        let node = builder.build_node(
            &jsx,
            &rules,
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &keyframes,
        );
        assert_eq!(node.animations.len(), 1);
        let anim = &node.animations[0];
        assert_eq!(anim.name, "pulse");
        assert_eq!(anim.duration, 2.0);
        assert_eq!(anim.easing, "ease-in-out");
        assert_eq!(anim.iterations, -1.0);
        // Unknown keyframe names are dropped like browsers do.
        let rules = vec![(
            ".ghost".to_string(),
            CssRule {
                selector: ".ghost".to_string(),
                properties: [("animation".to_string(), "nope 1s linear infinite".to_string())]
                    .into_iter()
                    .collect(),
            },
        )];
        let mut props = std::collections::HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::String("ghost".to_string()),
        );
        let jsx = morph_parser::JsxNode::Element {
            tag: "div".to_string(),
            props,
            children: Vec::new(),
            self_closing: true,
            line: 0,
            col: 0,
        };
        let node = builder.build_node(
            &jsx,
            &rules,
            0,
            &[],
            &HashMap::new(),
            &HashMap::new(),
            &mut Vec::new(),
            &keyframes,
        );
        assert!(node.animations.is_empty());
    }

    #[test]
    fn tailwind_class_names_flow_into_style() {
        let builder = IRBuilder::new();
        let mut props = std::collections::HashMap::new();
        props.insert(
            "className".to_string(),
            JsxPropValue::String("bg-red-500 text-lg".to_string()),
        );
        let node = builder.build_node(
            &morph_parser::JsxNode::Element {
                tag: "div".to_string(),
                props,
                children: Vec::new(),
                self_closing: true,
                line: 0,
                col: 0,
            },
            &[],
            0,
            &[],
                &HashMap::new(),
                &HashMap::new(),
                &mut Vec::new(),
            &HashMap::new(),
        );
        assert!((node.style.bg_color[0] - 0xef as f32 / 255.0).abs() < 0.001);
        assert!((node.style.bg_color[1] - 0x44 as f32 / 255.0).abs() < 0.001);
        assert_eq!(node.style.bg_color[3], 1.0);
        assert_eq!(node.style.font_size, 18.0);
    }
}
