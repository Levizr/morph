use anyhow::Result;
use morph_ir::IRWindow;
use std::path::Path;

use crate::feature_set::FeatureSet;
use crate::node_emitter;
use crate::logic_emitter;

const TEMPLATE: &str = include_str!("../../templates/app_main.cpp.tera");

pub struct CppEmitter<'a> {
    windows: &'a [IRWindow],
}

impl<'a> CppEmitter<'a> {
    pub fn new(windows: &'a [IRWindow]) -> Self {
        Self { windows }
    }

    pub fn emit(&self, output_dir: &Path) -> Result<()> {
        std::fs::create_dir_all(output_dir)?;

        let mut fs = FeatureSet::new();
        fs.scan(self.windows);
        let headers = fs.required_headers();
        let defines = fs.required_defines();

        // Collect state decls
        let mut state_decls = Vec::new();
        for w in self.windows {
            for sv in &w.state_vars {
                let init = sv.get("init").cloned().unwrap_or_else(|| "0".into());
                let name = sv.get("getter").cloned().unwrap_or_else(|| "unknown".into());
                let ty = infer_cpp_type(&init);
                let init = if ty == "JsArray" && is_array_literal(&init) { "JsArray{}".to_string() } else { init.clone() };
                state_decls.push(serde_json::json!({
                    "signal_name": format!("__st_{}", name),
                    "type": ty,
                    "init": init
                }));
            }
        }

        // Window code via node_emitter (with state map for generic transpilation)
        let mut window_code_parts = Vec::new();
        for win in self.windows {
            let mut state_map = std::collections::HashMap::new();
            for sv in &win.state_vars {
                if let (Some(getter), Some(_setter)) = (sv.get("getter"), sv.get("setter")) {
                    state_map.insert(getter.clone(), format!("__st_{}.get()", getter));
                    // setter name is like setUser for getter user, but we have setter field
                    if let Some(setter) = sv.get("setter") {
                        state_map.insert(setter.clone(), format!("__st_{}.set", getter));
                    }
                }
            }
            let mut code = String::new();
            let var = format!("win_{}", win.window_id);
            code.push_str(&format!("MorphWindow* {var} = new MorphWindow(\"{}\", {}, {}, {});\n", win.title, win.width, win.height, if win.visible { "true" } else { "false" }));
            code.push_str(&format!("wm.registerWindow(\"{}\", {var});\n", win.window_id));
            if win.min_width.is_some() || win.max_width.is_some() || win.min_height.is_some() || win.max_height.is_some() {
                let min_w = win.min_width.map(|v| v.to_string()).unwrap_or_else(|| "-1".into());
                let min_h = win.min_height.map(|v| v.to_string()).unwrap_or_else(|| "-1".into());
                let max_w = win.max_width.map(|v| v.to_string()).unwrap_or_else(|| "-1".into());
                let max_h = win.max_height.map(|v| v.to_string()).unwrap_or_else(|| "-1".into());
                code.push_str(&format!("{var}->setConstraints({min_w}, {min_h}, {max_w}, {max_h});\n"));
            }
            for node in &win.nodes {
                let c = node_emitter::emit_node_with_state(node, Some(&var), &fs.features, &state_map, None);
                if !c.is_empty() { code.push_str(&c); code.push('\n'); }
            }
            for log in &win.startup_logs {
                code.push_str(&format!("    fprintf(stderr, \"{}\\n\");\n", log.replace('\\', "\\\\").replace('"', "\\\"")));
            }
            window_code_parts.push(code);
        }
        let window_code = window_code_parts.join("\n");

        // List factories
        let mut factories = Vec::new();
        for w in self.windows {
            let mut state_map = std::collections::HashMap::new();
            for sv in &w.state_vars {
                if let Some(getter) = sv.get("getter") {
                    state_map.insert(getter.clone(), format!("__st_{}.get()", getter));
                    if let Some(setter) = sv.get("setter") {
                        state_map.insert(setter.clone(), format!("__st_{}.set", getter));
                    }
                }
            }
            for n in logic_emitter::collect_list_nodes(&w.nodes) {
                if let Some(ref tmpl) = n.item_template {
                    let body = node_emitter::emit_node_with_state(tmpl, None, &fs.features, &state_map, None);
                    let mut caps = String::new();
                    if body.contains("__it") { caps.push_str(", &__it"); }
                    if body.contains("__index") { caps.push_str(", &__index"); }
                    let body = body.replace("__LCAPS__", &caps);
                    let mut fac = format!("static MorphNode* __list_factory_{}(morph::ListItemBinding& __b) {{\n", n.node_id);
                    if body.contains("__it") { fac.push_str("    JsValue& __it = __b.item;\n"); }
                    if body.contains("__index") { fac.push_str("    int& __index = __b.index;\n"); }
                    fac.push_str(&body);
                    fac.push_str(&format!("\n    return {};\n}}", tmpl.node_id));
                    factories.push(fac);
                }
            }
        }
        let list_factory_code = factories.join("\n\n");

        // Keyframe registration (per-window keyframes are app-global) — port of
        // Python emitter.py which registers them once before any window.
        let mut keyframe_parts = Vec::new();
        for w in self.windows {
            let kf = node_emitter::keyframe_registration_code(&w.keyframes, &fs.features);
            if !kf.is_empty() { keyframe_parts.push(kf); }
        }
        let keyframe_code = keyframe_parts.join("\n");

        // Extra headers (dedup)
        let mut extra_headers: Vec<String> = self.windows.iter().flat_map(|w| w.extra_headers.clone()).collect();
        extra_headers.sort();
        extra_headers.dedup();

        // Premain code (functions like doLogin, logout)
        let premain_code = self.windows.iter().flat_map(|w| w.premain_functions.clone()).collect::<Vec<_>>().join("\n\n");

        // Render via Tera one_off
        let mut ctx = tera::Context::new();
        ctx.insert("windows", &self.windows);
        ctx.insert("window_code", &window_code);
        ctx.insert("keyframe_code", &keyframe_code);
        ctx.insert("list_factory_code", &list_factory_code);
        ctx.insert("headers", &headers);
        ctx.insert("extra_headers", &extra_headers);
        ctx.insert("defines", &defines);
        ctx.insert("dev_mode", &false);
        ctx.insert("premain_code", &premain_code);
        ctx.insert("state_decls", &state_decls);
        ctx.insert("native_mode", &false);
        let cpp_includes: Vec<serde_json::Value> = vec![];
        ctx.insert("cpp_includes", &cpp_includes);

        let rendered = tera::Tera::one_off(TEMPLATE, &ctx, false).unwrap_or_else(|e| format!("// Tera error: {}\n{}", e, TEMPLATE));

        std::fs::write(output_dir.join("app.cpp"), rendered)?;
        Ok(())
    }
}

fn infer_cpp_type(init: &str) -> String {
    let s = init.trim();
    if s == "true" || s == "false" { return "bool".into(); }
    if s.starts_with('"') || s.starts_with('\'') { return "std::string".into(); }
    if s.contains('.') { return "double".into(); }
    if s.parse::<i64>().is_ok() { return "int".into(); }
    if is_array_literal(s) { return "JsArray".into(); }
    "auto".into()
}

fn is_array_literal(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('[') && t.ends_with(']')
}
