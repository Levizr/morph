use anyhow::Result;
use morph_ir::IRWindow;
use std::path::Path;

use crate::feature_set::FeatureSet;
use crate::logic_emitter;
use crate::node_emitter;

const TEMPLATE: &str = include_str!("../../templates/app_main.cpp.tera");

pub struct CppEmitter<'a> {
    windows: &'a [IRWindow],
}

impl<'a> CppEmitter<'a> {
    pub const fn new(windows: &'a [IRWindow]) -> Self {
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
                let init = if ty == "JsArray" && is_array_literal(&init) {
                    "JsArray{}".to_string()
                } else {
                    init.clone()
                };
                state_decls.push(serde_json::json!({
                    "signal_name": format!("__st_{}", name),
                    "type": ty,
                    "init": init
                }));
            }
        }

        // Event declarations for static channel emission + morph_api.h.
        // Deduped by identity key (module path + event name).
        let mut event_decls: Vec<std::collections::HashMap<String, String>> = Vec::new();
        {
            let mut seen_keys = std::collections::HashSet::new();
            for w in self.windows {
                for ev in &w.event_decls {
                    let key = ev.get("key").cloned().unwrap_or_default();
                    if key.is_empty() || !seen_keys.insert(key.clone()) {
                        continue;
                    }
                    if ev.get("accessor").map_or(true, |a| a.is_empty()) {
                        continue;
                    }
                    event_decls.push(ev.clone());
                }
            }
        }
        // Legacy string channel id → namespace accessor expression. The
        // builder still emits `morph::channel("<id>")` placeholders (shared
        // with the dev TU); the build TU lowers every occurrence to the
        // static accessor so no string lookup survives in app.cpp.
        // Entries are (raw id, `morph::channel("<escaped>")` needle, expr).
        let channel_lower: Vec<(String, String, String)> = event_decls
            .iter()
            .map(|ev| {
                let raw = ev.get("channel").cloned().unwrap_or_default();
                let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
                let expr = event_expr(
                    ev.get("ns").map_or("", String::as_str),
                    ev.get("accessor").map_or("", String::as_str),
                );
                (raw, format!("morph::channel(\"{escaped}\")"), expr)
            })
            .collect();
        let lower_channels = |src: String| -> String {
            let mut out = src;
            for (_, needle, expr) in &channel_lower {
                if out.contains(needle.as_str()) {
                    out = out.replace(needle.as_str(), expr);
                }
            }
            out
        };

        // Window code via node_emitter (with state map for generic transpilation)
        let mut window_code_parts = Vec::new();
        for win in self.windows {
            let mut state_map = std::collections::HashMap::new();
            for sv in &win.state_vars {
                if let (Some(getter), Some(_setter)) = (sv.get("getter"), sv.get("setter")) {
                    state_map.insert(getter.clone(), format!("__st_{getter}.get()"));
                    // setter name is like setUser for getter user, but we have setter field
                    if let Some(setter) = sv.get("setter") {
                        state_map.insert(setter.clone(), format!("__st_{getter}.set"));
                    }
                }
            }
            for sv in &win.shared_vars {
                if let (Some(getter), Some(accessor)) = (sv.get("getter"), sv.get("accessor")) {
                    let ns = sv.get("ns").map_or("", String::as_str);
                    let read = shared_expr(ns, accessor);
                    state_map.insert(getter.clone(), format!("{read}.get()"));
                    if let Some(setter) = sv.get("setter") {
                        state_map.insert(setter.clone(), format!("{read}.set"));
                    }
                }
            }
            // Universal module bindings (functions/vars/classes): calls
            // and reads rewrite to the defining namespace (aliases are
            // never local bindings, so they contribute nothing here).
            for b in &win.module_bindings {
                let kind = b.get("kind").map_or("", String::as_str);
                if kind == "import" {
                    if let (Some(local), Some(expr)) = (b.get("local"), b.get("expr")) {
                        if !local.is_empty() && !expr.is_empty() {
                            state_map.insert(local.clone(), expr.clone());
                        }
                    }
                    continue;
                }
                if kind != "function" && kind != "var" && kind != "class" {
                    continue;
                }
                if let (Some(ns), Some(name)) = (b.get("ns"), b.get("name")) {
                    if !ns.is_empty() && !name.is_empty() {
                        state_map.insert(name.clone(), morph_ir::qualified_binding_ref(ns, name));
                    }
                }
            }
            // Reactive const lambdas re-evaluate on every reference.
            for name in &win.reactive_consts {
                state_map.insert(name.clone(), format!("{name}()"));
            }
            let mut code = String::new();
            let var = format!("win_{}", win.window_id);
            code.push_str(&format!(
                "MorphWindow* {var} = new MorphWindow(\"{}\", {}, {}, {});\n",
                win.title,
                win.width,
                win.height,
                if win.visible { "true" } else { "false" }
            ));
            code.push_str(&format!("wm.registerWindow(\"{}\", {var});\n", win.window_id));
            if win.min_width.is_some()
                || win.max_width.is_some()
                || win.min_height.is_some()
                || win.max_height.is_some()
            {
                let min_w = win.min_width.map_or_else(|| "-1".into(), |v| v.to_string());
                let min_h = win.min_height.map_or_else(|| "-1".into(), |v| v.to_string());
                let max_w = win.max_width.map_or_else(|| "-1".into(), |v| v.to_string());
                let max_h = win.max_height.map_or_else(|| "-1".into(), |v| v.to_string());
                code.push_str(&format!(
                    "{var}->setConstraints({min_w}, {min_h}, {max_w}, {max_h});\n"
                ));
            }
            for node in &win.nodes {
                let c = node_emitter::emit_node_with_state(
                    node,
                    Some(&var),
                    &fs.features,
                    &state_map,
                    None,
                );
                if !c.is_empty() {
                    code.push_str(&c);
                    code.push('\n');
                }
            }
            for log in &win.startup_logs {
                code.push_str(&format!(
                    "    fprintf(stderr, \"{}\\n\");\n",
                    log.replace('\\', "\\\\").replace('"', "\\\"")
                ));
            }
            // morphEffect declarations: empty deps run once, otherwise the
            // effect auto-subscribes to whatever signals the body reads.
            for ed in &win.effect_decls {
                let lambda = ed.get("lambda").map_or("", String::as_str);
                if lambda.is_empty() {
                    continue;
                }
                let deps = ed.get("deps").map_or("", |d| d.trim());
                if deps == "[]" {
                    code.push_str("    { // morphEffect (run once)\n");
                    code.push_str(&format!("        auto __ef_fn = {lambda};\n"));
                    code.push_str("        __ef_fn();\n");
                    code.push_str("    }\n");
                } else {
                    code.push_str(&format!("    morph::create_effect({lambda});\n"));
                }
            }
            for sub in &win.channel_subs {
                let channel = sub.get("channel").map_or("", String::as_str);
                let body = sub.get("body").map_or("", String::as_str);
                if channel.is_empty() || body.is_empty() {
                    continue;
                }
                // Prefer the static namespace accessor; hand-built IR
                // without event decls falls back to the string registry.
                let target = channel_lower
                    .iter()
                    .find(|(raw, _, _)| raw == channel)
                    .map(|(_, _, expr)| expr.clone())
                    .unwrap_or_else(|| {
                        let escaped = channel.replace('\\', "\\\\").replace('"', "\\\"");
                        format!("morph::channel(\"{escaped}\")")
                    });
                code.push_str(&format!("    {target}.on({body});\n"));
            }
            window_code_parts.push(lower_channels(code));
        }
        let window_code = window_code_parts.join("\n");

        // List factories
        let mut factories = Vec::new();
        for w in self.windows {
            let mut state_map = std::collections::HashMap::new();
            for sv in &w.state_vars {
                if let Some(getter) = sv.get("getter") {
                    state_map.insert(getter.clone(), format!("__st_{getter}.get()"));
                    if let Some(setter) = sv.get("setter") {
                        state_map.insert(setter.clone(), format!("__st_{getter}.set"));
                    }
                }
            }
            for sv in &w.shared_vars {
                if let (Some(getter), Some(accessor)) = (sv.get("getter"), sv.get("accessor")) {
                    let ns = sv.get("ns").map_or("", String::as_str);
                    let read = shared_expr(ns, accessor);
                    state_map.insert(getter.clone(), format!("{read}.get()"));
                    if let Some(setter) = sv.get("setter") {
                        state_map.insert(setter.clone(), format!("{read}.set"));
                    }
                }
            }
            for b in &w.module_bindings {
                let kind = b.get("kind").map_or("", String::as_str);
                if kind == "import" {
                    if let (Some(local), Some(expr)) = (b.get("local"), b.get("expr")) {
                        if !local.is_empty() && !expr.is_empty() {
                            state_map.insert(local.clone(), expr.clone());
                        }
                    }
                    continue;
                }
                if kind != "function" && kind != "var" && kind != "class" {
                    continue;
                }
                if let (Some(ns), Some(name)) = (b.get("ns"), b.get("name")) {
                    if !ns.is_empty() && !name.is_empty() {
                        state_map.insert(name.clone(), morph_ir::qualified_binding_ref(ns, name));
                    }
                }
            }
            for name in &w.reactive_consts {
                state_map.insert(name.clone(), format!("{name}()"));
            }
            for n in logic_emitter::collect_list_nodes(&w.nodes) {
                if let Some(ref tmpl) = n.item_template {
                    // Item templates resolve the map callback's parameters
                    // (`item`/`index`, whatever the user named them) to the
                    // factory's `__it` / `__index`. Scoped clone: the outer
                    // map must not see these bindings.
                    let mut tmpl_map = state_map.clone();
                    if n.list_item_param.is_empty() {
                        tmpl_map.insert("item".to_string(), "__it".to_string());
                    } else {
                        tmpl_map.insert(n.list_item_param.clone(), "__it".to_string());
                    }
                    if !n.list_index_param.is_empty() {
                        tmpl_map.insert(n.list_index_param.clone(), "__index".to_string());
                    }
                    let body = node_emitter::emit_node_with_state(
                        tmpl,
                        None,
                        &fs.features,
                        &tmpl_map,
                        None,
                    );
                    let mut caps = String::new();
                    if body.contains("__it") {
                        caps.push_str(", &__it");
                    }
                    if body.contains("__index") {
                        caps.push_str(", &__index");
                    }
                    let body = body.replace("__LCAPS__", &caps);
                    let mut fac = format!(
                        "static MorphNode* __list_factory_{}(morph::ListItemBinding& __b) {{\n",
                        n.node_id
                    );
                    if body.contains("__it") {
                        fac.push_str("    JsValue& __it = __b.item;\n");
                    }
                    if body.contains("__index") {
                        fac.push_str("    int& __index = __b.index;\n");
                    }
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
            if !kf.is_empty() {
                keyframe_parts.push(kf);
            }
        }
        let keyframe_code = keyframe_parts.join("\n");

        // Extra headers (dedup)
        let mut extra_headers: Vec<String> =
            self.windows.iter().flat_map(|w| w.extra_headers.clone()).collect();
        extra_headers.sort();
        extra_headers.dedup();

        // Premain entries, raw (pre-join) for definition lookup.
        let premain_entries: Vec<String> =
            self.windows.iter().flat_map(|w| w.premain_functions.clone()).collect();
        // Universal module bindings across windows, deduped by identity.
        let mut module_bindings_all: Vec<std::collections::HashMap<String, String>> = Vec::new();
        {
            let mut seen_keys = std::collections::HashSet::new();
            for w in self.windows {
                for b in &w.module_bindings {
                    let key = b.get("key").cloned().unwrap_or_default();
                    if key.is_empty() || !seen_keys.insert(key.clone()) {
                        continue;
                    }
                    module_bindings_all.push(b.clone());
                }
            }
        }
        // Class definitions move to the header (full definition needed
        // for `new`; a second copy in premain would redefine). Everything
        // else stays; event placeholders lower everywhere including moved
        // class bodies.
        let mut moved_classes: Vec<(String, String)> = Vec::new();
        let mut kept_entries: Vec<String> = Vec::new();
        for entry in premain_entries {
            match move_class_entry(&entry, &module_bindings_all) {
                Some((ns, inner)) => {
                    moved_classes.push((ns, lower_channels(inner)));
                }
                None => kept_entries.push(entry),
            }
        }
        // Premain code (functions like doLogin, logout). Event emit
        // placeholders lower to static accessors like window code.
        let premain_code = lower_channels(kept_entries.join("\n\n"));

        // Native mode: user C++ imports via `import "./file.cpp"`
        // Native mode: user C++ imports via `import "./file.cpp"`
        let mut cpp_includes: Vec<serde_json::Value> = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();
        for w in self.windows {
            for import in &w.cpp_imports {
                if let Some(path) = import.get("path") {
                    if !path.is_empty() && seen_paths.insert(path.clone()) {
                        let import_path = import.get("import_path").map_or("", String::as_str);
                        cpp_includes.push(serde_json::json!({
                            "import_path": import_path,
                            "path": path,
                        }));
                    }
                }
            }
        }
        let native_mode = !cpp_includes.is_empty();

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
        ctx.insert("native_mode", &native_mode);
        ctx.insert("cpp_includes", &cpp_includes);

        // `mid` indexed-accessor definitions (after the state signals) +
        // headless self-test body for `--morph-self-test`.
        let mid_code = generate_mid_code(self.windows, &premain_code);
        ctx.insert("mid_code", &mid_code);
        let self_test_code = generate_self_test(self.windows);
        ctx.insert("self_test_code", &self_test_code);

        let rendered = tera::Tera::one_off(TEMPLATE, &ctx, false)
            .unwrap_or_else(|e| format!("// Tera error: {e}\n{TEMPLATE}"));
        // Same hazard as the dev TU: snippet bodies spliced into lambdas
        // must not carry namespace-scope `morph::js_cmp` helpers.
        let rendered = morpher::codegen::js_comparison::hoist_js_cmp_preludes(&rendered);

        std::fs::write(output_dir.join("app.cpp"), rendered)?;

        // Per-project native contract: signal/channel definitions +
        // wrappers for user C++. Always generated (not just native mode)
        // so `#include "morph_api.h"` in app.cpp never dangles.
        let wrapped: Vec<(String, String)> =
            kept_entries.iter().filter_map(|e| logic_emitter::split_module_ns(e)).collect();
        let api_header = generate_morph_api_header(
            self.windows,
            &premain_code,
            &event_decls,
            &module_bindings_all,
            &moved_classes,
            &wrapped,
        );
        std::fs::write(output_dir.join("morph_api.h"), api_header)?;

        // Generate _morph_state.h for native mode (signals + JSX wrappers;
        // module function decls live namespaced via split parsing now).
        if native_mode {
            let state_header = logic_emitter::generate_state_header(self.windows, &kept_entries);
            std::fs::write(output_dir.join("_morph_state.h"), state_header)?;
        }
        Ok(())
    }
}

fn infer_cpp_type(init: &str) -> String {
    let s = init.trim();
    if s == "true" || s == "false" {
        return "bool".into();
    }
    if s.starts_with('"') || s.starts_with('\'') {
        return "std::string".into();
    }
    if s.contains('.') {
        return "double".into();
    }
    if s.parse::<i64>().is_ok() {
        return "int".into();
    }
    if is_array_literal(s) {
        return "JsArray".into();
    }
    "auto".into()
}

fn is_array_literal(s: &str) -> bool {
    let t = s.trim();
    t.starts_with('[') && t.ends_with(']')
}

fn clean_shared_init(init: &str, ty: &str) -> String {
    let t = init.trim();
    if ty == "JsArray" && is_array_literal(t) {
        return "JsArray{}".to_string();
    }
    if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        return format!("\"{}\"", t[1..t.len() - 1].replace('"', "\\\""));
    }
    t.to_string()
}

/// Fully-qualified shared accessor call in the generated program. Empty
/// namespace (hand-built IR) falls back to global scope. Absolute
/// (`::app::...`): premain bodies sit inside their own namespace blocks,
/// where a leading `app::` would resolve through `app::app`.
fn shared_expr(ns: &str, accessor: &str) -> String {
    if ns.is_empty() {
        format!("{accessor}()")
    } else {
        format!("::{}::{ns}::{accessor}()", morph_ir::MODULE_NS_ROOT)
    }
}

/// Fully-qualified event channel accessor call. Mirrors `shared_expr`.
fn event_expr(ns: &str, accessor: &str) -> String {
    if ns.is_empty() {
        format!("{accessor}()")
    } else {
        format!("::{}::{ns}::{accessor}()", morph_ir::MODULE_NS_ROOT)
    }
}

/// Accessor names for a `mid`-indexed state: the exact JSX suffix
/// names (`setCount`/`count`), falling back to `set_<getter>` only when
/// the suffix setter is absent (never in practice — `morphState`
/// always declares one).
fn mid_fn_names(getter: &str, setter: &str) -> (String, String) {
    let set_fn = if setter.is_empty() { format!("set_{getter}") } else { setter.to_string() };
    (set_fn, getter.to_string())
}

/// Definitions for the `mid` declarations in `morph_api.h`: switch
/// dispatch over per-instance `__st_` statics, emitted in `app.cpp`
/// after the state signals. Bounds-safe by construction (unknown index
/// → no-op; the getter returns `T{}`). Instance statics are build-time
/// constants with no dynamic lifetime, so no liveness mask is needed:
/// a write to a detached (conditionally unmounted) instance updates
/// its own static harmlessly and never touches another instance.
pub(crate) fn generate_mid_code(windows: &[IRWindow], premain_code: &str) -> String {
    let taken = premain_names(premain_code);
    let mut by_ns: std::collections::HashMap<
        String,
        Vec<&std::collections::HashMap<String, String>>,
    > = std::collections::HashMap::new();
    let mut ns_order: Vec<String> = Vec::new();
    for w in windows {
        for a in &w.mid_assignments {
            let ns = a.get("ns").map_or("", String::as_str);
            if ns.is_empty() {
                continue;
            }
            if !by_ns.contains_key(ns) {
                ns_order.push(ns.to_string());
            }
            by_ns.entry(ns.to_string()).or_default().push(a);
        }
    }
    let mut out = Vec::new();
    for ns in ns_order {
        let assigns = &by_ns[&ns];
        // Distinct states: getter → (setter, type, [(index, signal)]).
        let mut states: Vec<(&str, &str, String, Vec<(usize, String)>)> = Vec::new();
        let mut state_idx: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for a in assigns {
            let getter = a.get("getter").map_or("", String::as_str);
            if getter.is_empty() {
                continue;
            }
            let setter = a.get("setter").map_or("", String::as_str);
            let ty = infer_cpp_type(a.get("init").map_or("0", String::as_str));
            if ty == "auto" {
                continue;
            }
            let index = a.get("index").map_or("0", String::as_str).parse::<usize>().unwrap_or(0);
            let signal = a.get("signal").map_or("", String::as_str).to_string();
            match state_idx.get(getter) {
                Some(&i) => states[i].3.push((index, signal)),
                None => {
                    state_idx.insert(getter.to_string(), states.len());
                    states.push((getter, setter, ty, vec![(index, signal)]));
                }
            }
        }
        if states.is_empty() {
            continue;
        }
        out.push(format!("namespace {} {{", morph_ir::MODULE_NS_ROOT));
        out.push(format!("namespace {ns} {{"));
        for (getter, setter, ty, mut cases) in states {
            cases.sort_by_key(|(index, _)| *index);
            let (set_fn, get_fn) = mid_fn_names(getter, setter);
            if !taken.contains(set_fn.as_str()) {
                out.push(format!("void {set_fn}(uint32_t mid, {ty} v) {{"));
                out.push("    switch (mid) {".to_string());
                for (index, signal) in &cases {
                    out.push(format!("        case {index}: {signal}.set(v); break;"));
                }
                out.push("        default: break;".to_string());
                out.push("    }".to_string());
                out.push("}".to_string());
            }
            if !taken.contains(get_fn.as_str()) {
                out.push(format!("{ty} {get_fn}(uint32_t mid) {{"));
                out.push("    switch (mid) {".to_string());
                for (index, signal) in &cases {
                    out.push(format!("        case {index}: return {signal}.get();"));
                }
                out.push(format!("        default: return {ty}{{}};"));
                out.push("    }".to_string());
                out.push("}".to_string());
            }
        }
        out.push("}".to_string());
        out.push("}".to_string());
    }
    out.join("\n")
}

/// Headless runtime self-test (`binary --morph-self-test`): assertions
/// over shared stores, event delivery, and `mid`-indexed state. Runs
/// before GLFW init, so it needs no display. Returns process exit code.
fn generate_self_test(windows: &[IRWindow]) -> String {
    let mut lines = vec![
        "int morph_self_test() {".to_string(),
        "    int failures = 0;".to_string(),
        "    int checks = 0;".to_string(),
        "    auto check = [&](bool ok, const char* name) {".to_string(),
        "        ++checks;".to_string(),
        "        if (!ok) { ++failures; printf(\"[morph-self-test] FAIL %s\\n\", name); }"
            .to_string(),
        "    };".to_string(),
    ];
    // Shared roundtrips through the header wrappers.
    {
        let mut seen = std::collections::HashSet::new();
        for w in windows {
            for sv in &w.shared_vars {
                let key = sv.get("key").map_or("", String::as_str);
                let getter = sv.get("getter").map_or("", String::as_str);
                let setter = sv.get("setter").map_or("", String::as_str);
                if key.is_empty()
                    || getter.is_empty()
                    || setter.is_empty()
                    || !seen.insert(key.to_string())
                {
                    continue;
                }
                let init = sv.get("init").map_or("0", String::as_str);
                let ty = match sv.get("type").filter(|t| *t != "auto") {
                    Some(t) => t.to_string(),
                    None => infer_cpp_type(init),
                };
                let (probe, back, cmp) = match ty.as_str() {
                    "int" => ("41".to_string(), "42".to_string(), "==".to_string()),
                    "double" => ("3.25".to_string(), "3.5".to_string(), "==".to_string()),
                    "bool" => ("false".to_string(), "true".to_string(), "==".to_string()),
                    "std::string" => (
                        "\"__probe__\"".to_string(),
                        "\"__selftest__\"".to_string(),
                        "==".to_string(),
                    ),
                    _ => continue,
                };
                let ns = sv.get("ns").map_or("", String::as_str);
                let q = |name: &str| {
                    if ns.is_empty() {
                        name.to_string()
                    } else {
                        format!("{}::{ns}::{name}", morph_ir::MODULE_NS_ROOT)
                    }
                };
                lines.push(format!("    {}({});", q(setter), probe));
                lines.push(format!(
                    "    check({}() {cmp} {}, \"shared:{key}\");",
                    q(getter),
                    probe
                ));
                lines.push(format!("    {}({});", q(setter), back));
            }
        }
    }
    // Event delivery: subscribe, emit, expect the listener to run.
    {
        let mut seen = std::collections::HashSet::new();
        for w in windows {
            for ev in &w.event_decls {
                let key = ev.get("key").map_or("", String::as_str);
                if key.is_empty() || !seen.insert(key.to_string()) {
                    continue;
                }
                let expr = event_expr(
                    ev.get("ns").map_or("", String::as_str),
                    ev.get("accessor").map_or("", String::as_str),
                );
                let flag = format!("__selftest_hit_{}", seen.len());
                lines.push(format!("    static bool {flag} = false;"));
                lines.push(format!("    {flag} = false;"));
                lines.push(format!("    {expr}.on([&](const JsValue&) {{ {flag} = true; }});"));
                lines.push(format!("    {expr}.emit(JsObject{{}});"));
                lines.push(format!("    check({flag}, \"event:{key}\");"));
            }
        }
    }
    // mid-indexed writes read back through the indexed getters.
    {
        let mut seen_consts = std::collections::HashSet::new();
        for w in windows {
            for a in &w.mid_assignments {
                let c = a.get("const").map_or("", String::as_str);
                let getter = a.get("getter").map_or("", String::as_str);
                if c.is_empty()
                    || getter.is_empty()
                    || !seen_consts.insert(format!("{c}::{getter}"))
                {
                    continue;
                }
                let ty = infer_cpp_type(a.get("init").map_or("0", String::as_str));
                let probe = match ty.as_str() {
                    "int" => "7".to_string(),
                    "double" => "2.5".to_string(),
                    "bool" => "true".to_string(),
                    "std::string" => "\"__mid__\"".to_string(),
                    _ => continue,
                };
                let ns = a.get("ns").map_or("", String::as_str);
                let root = morph_ir::MODULE_NS_ROOT;
                let (set_fn, get_fn) =
                    mid_fn_names(getter, a.get("setter").map_or("", String::as_str));
                lines.push(format!("    {root}::{ns}::{set_fn}({root}::{ns}::{c}, {probe});"));
                lines.push(format!(
                    "    check({root}::{ns}::{get_fn}({root}::{ns}::{c}) == {probe}, \"mid:{c}\");"
                ));
            }
        }
    }
    // JsObject/JsArray value semantics: hashed lookups, sorted
    // enumeration, no-insert-on-read, no-throw string indexing.
    {
        lines.push("    JsObject __st_obj;".to_string());
        lines.push("    __st_obj.set(\"b\", JsValue(2));".to_string());
        lines.push("    __st_obj.set(\"a\", JsValue(1));".to_string());
        lines.push(
            "    check(__st_obj.get(\"missing\").is_undefined(), \"obj:missing-undefined\");"
                .to_string(),
        );
        lines.push("    check(!__st_obj.has(\"missing\"), \"obj:no-insert-on-read\");".to_string());
        lines.push("    check(__st_obj.sorted_keys() == std::vector<std::string>{\"a\", \"b\"}, \"obj:sorted-keys\");".to_string());
        lines.push("    JsArray __st_arr{JsValue(10), JsValue(20), JsValue(30)};".to_string());
        lines.push("    check(__st_arr[\"length\"] == JsValue(3), \"arr:length\");".to_string());
        lines.push("    check(__st_arr[\"1\"] == JsValue(20), \"arr:digit-index\");".to_string());
        lines.push(
            "    check(__st_arr[\"01\"].is_undefined(), \"arr:leading-zero-named\");".to_string(),
        );
        lines.push("    check(__st_arr[\"9\"].is_undefined(), \"arr:oob-undefined\");".to_string());
        lines.push(
            "    check(__st_arr[\"zzz\"].is_undefined(), \"arr:garbage-undefined\");".to_string(),
        );
    }
    lines.push(
        "    printf(\"[morph-self-test] %d checks, %d failures\\n\", checks, failures);"
            .to_string(),
    );
    lines.push("    return failures == 0 ? 0 : 1;".to_string());
    lines.push("}".to_string());
    lines.join("\n")
}

/// Names of functions defined in premain, so generated wrappers never
/// redefine a user function (same rule as `_morph_state.h`). Only legacy
/// flat entries count: namespaced definitions cannot collide across
/// namespaces, and same-namespace collisions are hard errors at seed
/// time — counting qualified names here would only suppress wrappers
/// that belong to other namespaces.
fn premain_names(premain_code: &str) -> std::collections::HashSet<String> {
    premain_code
        .split("\n\n")
        .filter_map(logic_emitter::extract_function_decl)
        .filter_map(|decl| logic_emitter::fn_name(&decl))
        .collect()
}

/// If a wrapped premain entry defines a module-bound class, return its
/// `(ns, inner)` for relocation to the header (callers need the full
/// definition for `new`; a copy left in premain would redefine it).
/// Anything else stays put.
fn move_class_entry(
    entry: &str,
    bindings: &[std::collections::HashMap<String, String>],
) -> Option<(String, String)> {
    let (ns, inner) = logic_emitter::split_module_ns(entry)?;
    let first_line = inner.trim_start().lines().next().unwrap_or("");
    let class_name = first_line
        .strip_prefix("class ")
        .or_else(|| first_line.strip_prefix("struct "))
        .and_then(|rest| rest.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$').next())
        .filter(|name| !name.is_empty())?;
    let bound = bindings.iter().any(|b| {
        b.get("kind").map_or(false, |k| k == "class")
            && b.get("ns").map_or("", String::as_str) == ns
            && b.get("name").map_or("", String::as_str) == class_name
    });
    if bound {
        Some((ns, inner))
    } else {
        None
    }
}

/// Lines introducing a RENAMED re-export alias (`export {a as b}`) in
/// its namespace. Same-name aliases use plain `using` (handled at the
/// call site); renamed ones need a same-named entity per target kind:
/// forwarding function(s), `auto&` reference, or type alias. Empty when
/// the target's declarations cannot be resolved (safe degradation: the
/// ultimate namespace stays callable).
fn alias_wrapper(
    alias: &str,
    target_expr: &str,
    target_kind: &str,
    target_decls: &[String],
) -> Vec<String> {
    match target_kind {
        "function" => target_decls
            .iter()
            .filter_map(|decl| {
                let paren = decl.find('(')?;
                let ret_and_name = decl[..paren].trim();
                let mut parts = ret_and_name.rsplitn(2, char::is_whitespace);
                let fn_name = parts.next()?;
                let ret = parts.next().unwrap_or("").trim();
                if fn_name.is_empty() || ret.is_empty() {
                    return None;
                }
                let params = decl[paren + 1..].trim_end_matches(')').trim();
                let mut args = Vec::new();
                if !params.is_empty() && params != "void" {
                    for param in split_top_level(params, ',') {
                        let name = param
                            .rsplit(|c: char| !c.is_alphanumeric() && c != '_' && c != '$')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_start_matches('*')
                            .trim_start_matches('&')
                            .trim();
                        if name.is_empty()
                            || name == "const"
                            || !name
                                .chars()
                                .next()
                                .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
                        {
                            return None;
                        }
                        args.push(name.to_string());
                    }
                }
                Some(format!(
                    "inline {ret} {alias}({params}) {{ return {target_expr}({}); }}",
                    args.join(", ")
                ))
            })
            .collect(),
        "var" => vec![format!("inline auto& {alias} = {target_expr};")],
        "class" => vec![format!("using {alias} = {target_expr};")],
        _ => Vec::new(),
    }
}

/// Split on a delimiter at nesting depth 0 (for parameter lists with
/// `std::function<void()>`-style nested brackets).
fn split_top_level(s: &str, delim: char) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] as char {
            '"' | '\'' => {
                i = crate::node_emitter::js_skip_string(s, i);
                continue;
            }
            '<' | '(' | '[' | '{' => depth += 1,
            '>' | ')' | ']' | '}' => depth -= 1,
            c if c == delim && depth == 0 => {
                out.push(s[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(s[start..].to_string());
    out
}

/// `extern Type name;` for a namespace-scope variable definition, or
/// `None` when the shape is not a plain declarator (const/auto/complex
/// forms stay definition-only in premain).
fn extern_var_decl(inner: &str, name: &str) -> Option<String> {
    let t = inner.trim();
    for prefix in [
        "const ",
        "static ",
        "inline ",
        "auto ",
        "template",
        "typedef ",
        "using ",
        "namespace ",
        "class ",
        "struct ",
        "enum ",
        "#",
    ] {
        if t.starts_with(prefix) {
            return None;
        }
    }
    // First `=` at depth 0 (skipping strings) separates declarator.
    let bytes = t.as_bytes();
    let mut depth = 0i32;
    let mut i = 0;
    let mut eq = None;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' || b == b'\'' {
            i = crate::node_emitter::js_skip_string(t, i);
            continue;
        }
        if b == b'(' || b == b'[' || b == b'{' {
            depth += 1;
        } else if b == b')' || b == b']' || b == b'}' {
            depth -= 1;
        } else if b == b'=' && depth == 0 {
            // `==`/`!=`/`<=`/`>=` can only appear inside an initializer,
            // which a declarator-first scan never reaches first... except
            // `bool x = (a == b)`: the `(` already raised depth. A bare
            // `=` here starts the initializer.
            eq = Some(i);
            break;
        }
        i += 1;
    }
    let lhs = match eq {
        Some(e) => t[..e].trim(),
        None => t.trim_end_matches(';').trim(),
    };
    if lhs.is_empty() || lhs.contains('(') {
        return None;
    }
    let has_name =
        lhs.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$').any(|w| w == name);
    if !has_name {
        return None;
    }
    Some(format!("extern {lhs};"))
}

/// (namespace, lines) groups preserving first-seen order, shared by the
/// build and dev `morph_api.h` generators so both flavors group entries
/// identically. Global-scope entries are spliced inline at the end.
struct NsBlocks {
    blocks: Vec<(String, Vec<String>)>,
    by_ns: std::collections::HashMap<String, usize>,
}

impl NsBlocks {
    fn new() -> Self {
        NsBlocks { blocks: Vec::new(), by_ns: std::collections::HashMap::new() }
    }

    fn push(&mut self, ns: &str, entry_lines: Vec<String>) {
        if let Some(&idx) = self.by_ns.get(ns) {
            self.blocks[idx].1.extend(entry_lines);
        } else {
            self.by_ns.insert(ns.to_string(), self.blocks.len());
            self.blocks.push((ns.to_string(), entry_lines));
        }
    }

    fn flush(self, lines: &mut Vec<String>) {
        for (ns, member_lines) in self.blocks {
            if ns.is_empty() {
                lines.extend(member_lines);
                continue;
            }
            lines.push(format!("namespace {} {{", morph_ir::MODULE_NS_ROOT));
            lines.push(format!("namespace {ns} {{"));
            lines.extend(member_lines);
            lines.push("}".to_string());
            lines.push("}".to_string());
        }
    }
}

/// `mid` constants + accessor declarations for one component namespace,
/// shared by the build and dev headers (definitions live in `app.cpp`
/// and the dev TU respectively).
fn mid_header_entries(
    assigns: &[&std::collections::HashMap<String, String>],
    taken: &std::collections::HashSet<String>,
) -> Vec<String> {
    let mut entry = Vec::new();
    // Constants first, in index order.
    let mut consts: Vec<(&str, &str, &str, &str)> = Vec::new(); // (index, const, mid, loc)
    let mut seen_consts = std::collections::HashSet::new();
    for a in assigns {
        let c = a.get("const").map_or("", String::as_str);
        if c.is_empty() || !seen_consts.insert(c.to_string()) {
            continue;
        }
        consts.push((
            a.get("index").map_or("0", String::as_str),
            c,
            a.get("mid").map_or("", String::as_str),
            a.get("loc").map_or("", String::as_str),
        ));
    }
    consts.sort_by_key(|(idx, _, _, _)| idx.parse::<usize>().unwrap_or(0));
    let comp = assigns.first().map_or("", |a| a.get("comp").map_or("", String::as_str));
    let module = assigns.first().map_or("", |a| a.get("module").map_or("", String::as_str));
    for (_, c, mid, loc) in &consts {
        entry.push(format!("// <{comp} mid=\"{mid}\"> ({module}:{loc})"));
        entry.push(format!(
            "constexpr uint32_t {c} = {};",
            consts.iter().position(|(_, cc, _, _)| cc == c).unwrap_or(0)
        ));
    }
    // One accessor pair per distinct state (exact JSX suffix names).
    let mut states: Vec<(&str, &str, &str)> = Vec::new(); // (getter, setter, init)
    let mut seen_states = std::collections::HashSet::new();
    for a in assigns {
        let g = a.get("getter").map_or("", String::as_str);
        if g.is_empty() || !seen_states.insert(g.to_string()) {
            continue;
        }
        states.push((
            g,
            a.get("setter").map_or("", String::as_str),
            a.get("init").map_or("0", String::as_str),
        ));
    }
    for (getter, setter, init) in states {
        let ty = infer_cpp_type(init);
        if ty == "auto" {
            continue;
        }
        let (set_fn, get_fn) = mid_fn_names(getter, setter);
        if !taken.contains(set_fn.as_str()) {
            entry.push(format!("void {set_fn}(uint32_t mid, {ty} v);"));
        }
        if !taken.contains(get_fn.as_str()) {
            entry.push(format!("{ty} {get_fn}(uint32_t mid);"));
        }
    }
    entry
}

/// Build the per-project `morph_api.h`: the native developer's contract.
/// Inline signal/channel definitions live here (not in `app.cpp`) so user
/// C++ included at the top of `app.cpp` sees declarations before use, and
/// `inline` keeps them safe across translation units. `app.cpp` complexity
/// is irrelevant; this header's DX is sacred: thin wrappers + mapping
/// comments, zero string plumbing.
fn generate_morph_api_header(
    windows: &[IRWindow],
    premain_code: &str,
    event_decls: &[std::collections::HashMap<String, String>],
    module_bindings: &[std::collections::HashMap<String, String>],
    moved_classes: &[(String, String)],
    wrapped: &[(String, String)],
) -> String {
    let mut lines = vec![
        "#pragma once".to_string(),
        "// Generated by Morph — do not edit. This is the native developer's".to_string(),
        "// contract: include it from user C++ (`#include \"morph_api.h\"`) and".to_string(),
        "// call the accessors/wrappers below. Never read app.cpp.".to_string(),
        // Runtime base, spelled explicitly (never `#include "morph_api.h"`:
        // this file SHADOWS the runtime header by include order).
        "#include \"types/js_value.h\"".to_string(),
        "#include \"types/js_object.h\"".to_string(),
        "#include \"reactivity/signal.h\"".to_string(),
        "#include \"reactivity/channel.h\"".to_string(),
        String::new(),
    ];
    let taken = premain_names(premain_code);
    // (namespace, lines) groups preserving first-seen order.
    let mut blocks = NsBlocks::new();

    // Shared stores: definition + get_/set_ wrappers.
    {
        let mut seen_keys = std::collections::HashSet::new();
        for w in windows {
            for sv in &w.shared_vars {
                let key = sv.get("key").map_or("", String::as_str);
                let accessor = sv.get("accessor").map_or("", String::as_str);
                let getter = sv.get("getter").map_or("", String::as_str);
                let setter = sv.get("setter").map_or("", String::as_str);
                if key.is_empty() || accessor.is_empty() || !seen_keys.insert(key.to_string()) {
                    continue;
                }
                let init_raw = sv.get("init").map_or("0", String::as_str);
                let mut ty = sv.get("type").map_or("auto", String::as_str).to_string();
                if ty == "auto" {
                    ty = infer_cpp_type(init_raw);
                }
                if ty == "auto" || getter.is_empty() {
                    continue;
                }
                let init = clean_shared_init(init_raw, &ty);
                let ns = sv.get("ns").map_or("", String::as_str);
                let module = sv.get("key").map_or("", String::as_str);
                let mut entry = vec![
                    format!("// {getter} ({module})"),
                    format!(
                        "inline morph::Signal<{ty}>& {accessor}() {{ static morph::Signal<{ty}> s({init}); return s; }}"
                    ),
                ];
                if !taken.contains(getter) {
                    entry.push(format!("inline {ty} {getter}() {{ return {accessor}().get(); }}"));
                }
                if !setter.is_empty() && !taken.contains(setter) {
                    entry.push(format!("inline void {setter}({ty} v) {{ {accessor}().set(v); }}"));
                }
                blocks.push(ns, entry);
            }
        }
    }

    // Events: channel definition + emit_/notify_ wrappers.
    {
        for ev in event_decls {
            let accessor = ev.get("accessor").map_or("", String::as_str);
            let name = ev.get("event").map_or("", String::as_str);
            if accessor.is_empty() || name.is_empty() {
                continue;
            }
            let ns = ev.get("ns").map_or("", String::as_str);
            let module = ev.get("module").map_or("", String::as_str);
            let mut entry = vec![
                format!("// {name} ({module})"),
                format!(
                    "inline morph::Channel& {accessor}() {{ static morph::Channel c; return c; }}"
                ),
            ];
            let emit_fn = format!("emit_{name}");
            let notify_fn = format!("notify_{name}");
            if !taken.contains(emit_fn.as_str()) {
                entry.push(format!(
                    "inline void {emit_fn}(const JsValue& payload) {{ {accessor}().emit(payload); }}"
                ));
            }
            if !taken.contains(notify_fn.as_str()) {
                entry.push(format!(
                    "inline void {notify_fn}() {{ {accessor}().emit(JsObject{{}}); }}"
                ));
            }
            blocks.push(ns, entry);
        }
    }

    // `mid` native index: constants + declarations. Definitions live in
    // app.cpp after the state signals (switch dispatch over `__st_`
    // statics); the header declares them so user code compiles.
    {
        let mut by_ns: std::collections::HashMap<
            String,
            Vec<&std::collections::HashMap<String, String>>,
        > = std::collections::HashMap::new();
        let mut ns_order: Vec<String> = Vec::new();
        for w in windows {
            for a in &w.mid_assignments {
                let ns = a.get("ns").map_or("", String::as_str);
                if ns.is_empty() {
                    continue;
                }
                if !by_ns.contains_key(ns) {
                    ns_order.push(ns.to_string());
                }
                by_ns.entry(ns.to_string()).or_default().push(a);
            }
        }
        for ns in ns_order {
            let assigns = &by_ns[&ns];
            blocks.push(ns.as_str(), mid_header_entries(assigns, &taken));
        }
    }

    // Universal module bindings: function declarations, var externs,
    // moved class definitions, and re-export aliases. Definitions live
    // once at their defining namespace (premain, or here for moved
    // classes); this header is the discovery surface for native C++.
    // Order matters: function/var declarations, then classes (method
    // bodies may call module functions), then aliases (every target
    // must be declared above — including moved classes).
    {
        let find_defns = |ns: &str, name: &str| -> Vec<String> {
            wrapped
                .iter()
                .filter(|(w_ns, _)| w_ns == ns)
                .filter_map(|(_, inner)| logic_emitter::extract_function_decl(inner))
                .filter(|decl| logic_emitter::fn_name(decl).as_deref() == Some(name))
                .map(|decl| decl.trim_end_matches(';').trim().to_string())
                .collect()
        };
        let mut emit_alias = |b: &std::collections::HashMap<String, String>,
                              blocks: &mut NsBlocks| {
            let ns = b.get("ns").map_or("", String::as_str);
            let name = b.get("name").map_or("", String::as_str);
            let module = b.get("module").map_or("", String::as_str);
            let target_ns = b.get("target_ns").map_or("", String::as_str);
            let target_name = b.get("target_name").map_or("", String::as_str);
            if ns.is_empty() || name.is_empty() || target_ns.is_empty() || target_name.is_empty() {
                return;
            }
            // Absolute: aliases live inside namespace blocks, where a
            // leading `app::` would resolve through `app::app`.
            let target_expr =
                format!("::{r}::{target_ns}::{target_name}", r = morph_ir::MODULE_NS_ROOT);
            let mut entry = vec![format!("// {name} (re-export of {target_expr})")];
            if name == target_name {
                entry.push(format!("using {target_expr};"));
            } else {
                let target_kind = module_bindings
                    .iter()
                    .find(|t| {
                        t.get("ns").map_or("", String::as_str) == target_ns
                            && t.get("name").map_or("", String::as_str) == target_name
                    })
                    .map_or("", |t| t.get("kind").map_or("", String::as_str));
                let target_decls: Vec<String> = wrapped
                    .iter()
                    .filter(|(w_ns, _)| w_ns == target_ns)
                    .filter_map(|(_, inner)| logic_emitter::extract_function_decl(inner))
                    .filter(|decl| logic_emitter::fn_name(decl).as_deref() == Some(target_name))
                    .map(|decl| decl.trim_end_matches(';').trim().to_string())
                    .collect();
                entry.extend(alias_wrapper(name, &target_expr, target_kind, &target_decls));
            }
            if entry.len() > 1 {
                blocks.push(ns, entry);
            }
        };
        let mut seen_keys = std::collections::HashSet::new();
        let mut aliases: Vec<&std::collections::HashMap<String, String>> = Vec::new();
        for b in module_bindings {
            let key = b.get("key").map_or("", String::as_str);
            if key.is_empty() || !seen_keys.insert(key.to_string()) {
                continue;
            }
            let kind = b.get("kind").map_or("", String::as_str);
            if kind == "alias" {
                aliases.push(b);
                continue;
            }
            let ns = b.get("ns").map_or("", String::as_str);
            let name = b.get("name").map_or("", String::as_str);
            let module = b.get("module").map_or("", String::as_str);
            if ns.is_empty() || name.is_empty() {
                continue;
            }
            match kind {
                "function" => {
                    let mut entry = vec![format!("// {name} ({module})")];
                    for decl in find_defns(ns, name) {
                        entry.push(format!("{decl};"));
                    }
                    if entry.len() > 1 {
                        blocks.push(ns, entry);
                    }
                }
                "var" => {
                    let mut entry = vec![format!("// {name} ({module})")];
                    for (_, inner) in wrapped.iter().filter(|(w_ns, _)| w_ns == ns) {
                        if let Some(decl) = extern_var_decl(inner, name) {
                            entry.push(decl);
                            break;
                        }
                    }
                    if entry.len() > 1 {
                        blocks.push(ns, entry);
                    }
                }
                "alias" => {
                    emit_alias(b, &mut blocks);
                }
                _ => {}
            }
        }
        // Moved class definitions, grouped by namespace.
        for (ns, inner) in moved_classes {
            blocks.push(ns, vec![inner.clone()]);
        }
        // Aliases last: every target (functions, vars, moved classes)
        // is declared above.
        for b in aliases {
            emit_alias(b, &mut blocks);
        }
    }

    blocks.flush(&mut lines);
    lines.push(String::new());
    lines.join("\n")
}

/// Dev-flow `morph_api.h`: the same declaration surface as the build
/// header — same namespaces, same wrapper names — bound to dev-TU
/// definitions instead of static ones.
///
/// The dev TU (`app_logic.cpp`) `#include`s user C++ into itself, so this
/// header is wrappers-only, never definitions:
/// - shared `cart()`/`setCart()` call the TU's namespaced `accessor()`.
/// - `evt_x()` returns `morph::channel("<id>")` — the same string-registry
///   channel the dev TU wires listeners on; `emit_`/`notify_` route through it.
/// - `mid` constants + declarations; the switch-dispatch definitions are
///   emitted into the dev TU by `emit_logic`.
/// - functions: declarations (as in dev `_morph_state.h`; duplicates legal).
/// - vars: `extern` decls (the dev TU defines them with external linkage).
/// - re-export aliases: `using` / inline wrappers (targets are defined in
///   the dev TU premain above the user-C++ include, so they resolve).
/// Classes need no header entry: the TU definitions are visible above the
/// include, and a second copy here would redefine.
pub fn generate_morph_api_header_dev(windows: &[IRWindow], premain_parts: &[String]) -> String {
    let mut lines = vec![
        "#pragma once".to_string(),
        "// Generated by Morph — do not edit. Dev-flow contract: same".to_string(),
        "// namespaces and wrapper names as the build header, bound to the".to_string(),
        "// dev TU (string-registry channels, TU-local signals). Refreshed".to_string(),
        "// on every rebuild; bodies may differ from `morph build`.".to_string(),
        "#include \"types/js_value.h\"".to_string(),
        "#include \"types/js_object.h\"".to_string(),
        "#include \"reactivity/signal.h\"".to_string(),
        "#include \"reactivity/channel.h\"".to_string(),
        String::new(),
    ];
    let premain_code = premain_parts.join("\n\n");
    let taken = premain_names(&premain_code);
    let mut blocks = NsBlocks::new();

    // Shared stores: wrappers over the dev TU accessors (defined in the
    // TU next to the backing signals; never redefined here).
    {
        let mut seen_keys = std::collections::HashSet::new();
        for w in windows {
            for sv in &w.shared_vars {
                let key = sv.get("key").map_or("", String::as_str);
                let accessor = sv.get("accessor").map_or("", String::as_str);
                let getter = sv.get("getter").map_or("", String::as_str);
                let setter = sv.get("setter").map_or("", String::as_str);
                if key.is_empty() || accessor.is_empty() || !seen_keys.insert(key.to_string()) {
                    continue;
                }
                let init_raw = sv.get("init").map_or("0", String::as_str);
                let mut ty = sv.get("type").map_or("auto", String::as_str).to_string();
                if ty == "auto" {
                    ty = infer_cpp_type(init_raw);
                }
                if ty == "auto" || getter.is_empty() {
                    continue;
                }
                let ns = sv.get("ns").map_or("", String::as_str);
                let module = sv.get("key").map_or("", String::as_str);
                // Absolute `::app::...` reference: the dev TU includes
                // `_morph_state.h` (which declares `app::app`) before this
                // header, so a leading `app::` inside `namespace app::<ns>`
                // would resolve to `app::app::<ns>`. The build header is
                // parsed before `app::app` exists and never hits this.
                let source = if ns.is_empty() {
                    format!("{accessor}()")
                } else {
                    format!("::{r}::{ns}::{accessor}()", r = morph_ir::MODULE_NS_ROOT)
                };
                // The TU defines the accessor *after* the user-C++ include,
                // so forward-declare it (same `static` linkage, same TU —
                // one entity, no ODR issue; the header is included once).
                let mut entry = vec![
                    format!("// {getter} ({module}) [dev]"),
                    format!("static morph::Signal<{ty}>& {accessor}();"),
                ];
                if !taken.contains(getter) {
                    entry.push(format!("inline {ty} {getter}() {{ return {source}.get(); }}"));
                }
                if !setter.is_empty() && !taken.contains(setter) {
                    entry.push(format!("inline void {setter}({ty} v) {{ {source}.set(v); }}"));
                }
                blocks.push(ns, entry);
            }
        }
    }

    // Events: channel functions over the string registry (the id the dev
    // TU wires listeners on) + emit_/notify_ wrappers.
    {
        let mut seen_keys = std::collections::HashSet::new();
        for w in windows {
            for ev in &w.event_decls {
                let key = ev.get("key").map_or("", String::as_str);
                let accessor = ev.get("accessor").map_or("", String::as_str);
                let name = ev.get("event").map_or("", String::as_str);
                let channel = ev.get("channel").map_or("", String::as_str);
                if key.is_empty()
                    || accessor.is_empty()
                    || name.is_empty()
                    || channel.is_empty()
                    || !seen_keys.insert(key.to_string())
                {
                    continue;
                }
                let escaped = channel.replace('\\', "\\\\").replace('"', "\\\"");
                let ns = ev.get("ns").map_or("", String::as_str);
                let module = ev.get("module").map_or("", String::as_str);
                let mut entry = vec![
                    format!("// {name} ({module}) [dev]"),
                    format!(
                        "inline morph::Channel& {accessor}() {{ return morph::channel(\"{escaped}\"); }}"
                    ),
                ];
                let emit_fn = format!("emit_{name}");
                let notify_fn = format!("notify_{name}");
                if !taken.contains(emit_fn.as_str()) {
                    entry.push(format!(
                        "inline void {emit_fn}(const JsValue& payload) {{ {accessor}().emit(payload); }}"
                    ));
                }
                if !taken.contains(notify_fn.as_str()) {
                    entry.push(format!(
                        "inline void {notify_fn}() {{ {accessor}().emit(JsObject{{}}); }}"
                    ));
                }
                blocks.push(ns, entry);
            }
        }
    }

    // `mid` constants + declarations (definitions live in the dev TU).
    {
        let mut by_ns: std::collections::HashMap<
            String,
            Vec<&std::collections::HashMap<String, String>>,
        > = std::collections::HashMap::new();
        let mut ns_order: Vec<String> = Vec::new();
        for w in windows {
            for a in &w.mid_assignments {
                let ns = a.get("ns").map_or("", String::as_str);
                if ns.is_empty() {
                    continue;
                }
                if !by_ns.contains_key(ns) {
                    ns_order.push(ns.to_string());
                }
                by_ns.entry(ns.to_string()).or_default().push(a);
            }
        }
        for ns in ns_order {
            let assigns = &by_ns[&ns];
            blocks.push(ns.as_str(), mid_header_entries(assigns, &taken));
        }
    }

    // Module functions: declarations (the dev `_morph_state.h` carries
    // the same; repeated identical declarations are legal C++).
    // Module vars: `extern` decls (defined in the dev TU premain).
    // Re-export aliases: `using` / inline wrappers — targets are defined
    // in the dev TU premain *above* the user-C++ include, so they resolve.
    // Classes need no header entry: the TU definitions are visible above.
    {
        let wrapped: Vec<(String, String)> =
            premain_parts.iter().filter_map(|e| logic_emitter::split_module_ns(e)).collect();
        let mut bindings_all: Vec<&std::collections::HashMap<String, String>> = Vec::new();
        {
            let mut seen_keys = std::collections::HashSet::new();
            for w in windows {
                for b in &w.module_bindings {
                    let key = b.get("key").map_or("", String::as_str);
                    if key.is_empty() || !seen_keys.insert(key.to_string()) {
                        continue;
                    }
                    bindings_all.push(b);
                }
            }
        }
        for b in &bindings_all {
            let kind = b.get("kind").map_or("", String::as_str);
            let ns = b.get("ns").map_or("", String::as_str);
            let name = b.get("name").map_or("", String::as_str);
            let module = b.get("module").map_or("", String::as_str);
            if ns.is_empty() || name.is_empty() {
                continue;
            }
            match kind {
                "function" => {
                    let mut entry = vec![format!("// {name} ({module}) [dev]")];
                    for (_, inner) in wrapped.iter().filter(|(w_ns, _)| w_ns == ns) {
                        if let Some(decl) = logic_emitter::extract_function_decl(inner) {
                            if logic_emitter::fn_name(&decl).as_deref() == Some(name) {
                                entry.push(format!("{};", decl.trim_end_matches(';').trim()));
                                break;
                            }
                        }
                    }
                    if entry.len() > 1 {
                        blocks.push(ns, entry);
                    }
                }
                "var" => {
                    let mut entry = vec![format!("// {name} ({module}) [dev]")];
                    for (_, inner) in wrapped.iter().filter(|(w_ns, _)| w_ns == ns) {
                        if let Some(decl) = extern_var_decl(inner, name) {
                            entry.push(decl);
                            break;
                        }
                    }
                    if entry.len() > 1 {
                        blocks.push(ns, entry);
                    }
                }
                "alias" => {
                    let target_ns = b.get("target_ns").map_or("", String::as_str);
                    let target_name = b.get("target_name").map_or("", String::as_str);
                    if target_ns.is_empty() || target_name.is_empty() {
                        continue;
                    }
                    // Absolute reference (see the shared section above: a
                    // leading `app::` inside `namespace app::<ns>` would
                    // resolve through `app::app` from `_morph_state.h`).
                    let target_expr =
                        format!("::{r}::{target_ns}::{target_name}", r = morph_ir::MODULE_NS_ROOT);
                    let mut entry = vec![format!("// {name} (re-export of {target_expr})")];
                    if name == target_name {
                        entry.push(format!("using {target_expr};"));
                    } else {
                        let target_kind = bindings_all
                            .iter()
                            .find(|t| {
                                t.get("ns").map_or("", String::as_str) == target_ns
                                    && t.get("name").map_or("", String::as_str) == target_name
                            })
                            .map_or("", |t| t.get("kind").map_or("", String::as_str));
                        let target_decls: Vec<String> = wrapped
                            .iter()
                            .filter(|(w_ns, _)| w_ns == target_ns)
                            .filter_map(|(_, inner)| logic_emitter::extract_function_decl(inner))
                            .filter(|decl| {
                                logic_emitter::fn_name(decl).as_deref() == Some(target_name)
                            })
                            .map(|decl| decl.trim_end_matches(';').trim().to_string())
                            .collect();
                        entry.extend(alias_wrapper(name, &target_expr, target_kind, &target_decls));
                    }
                    if entry.len() > 1 {
                        blocks.push(ns, entry);
                    }
                }
                _ => {}
            }
        }
    }

    blocks.flush(&mut lines);
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use morph_ir::IRNode;

    fn test_window() -> IRWindow {
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            modal: false,
            renderer: "flash".to_string(),
            nodes: Vec::new(),
            startup_logs: Vec::new(),
            premain_functions: vec!["auto helper()\n{\n    return 1;\n}".to_string()],
            extra_headers: Vec::new(),
            state_vars: Vec::new(),
            reactive_consts: Vec::new(),
            shared_vars: Vec::new(),
            effect_decls: vec![
                [
                    ("lambda".to_string(), "[]() { run_once(); }".to_string()),
                    ("deps".to_string(), "[]".to_string()),
                ]
                .into_iter()
                .collect(),
                [
                    ("lambda".to_string(), "[]() { track(); }".to_string()),
                    ("deps".to_string(), "[count]".to_string()),
                ]
                .into_iter()
                .collect(),
                [
                    ("lambda".to_string(), "[]() { always(); }".to_string()),
                    ("deps".to_string(), String::new()),
                ]
                .into_iter()
                .collect(),
            ],
            channel_subs: Vec::new(),
            event_decls: Vec::new(),
            mid_assignments: Vec::new(),
            module_bindings: Vec::new(),
            cpp_imports: Vec::new(),
            keyframes: std::collections::HashMap::new(),
        }
    }

    fn shared_window() -> IRWindow {
        use std::collections::HashMap;
        let mut sv = HashMap::new();
        sv.insert("key".to_string(), "cart.count".to_string());
        sv.insert("accessor".to_string(), "shared_cart_count".to_string());
        sv.insert("type".to_string(), "int".to_string());
        sv.insert("init".to_string(), "0".to_string());
        sv.insert("getter".to_string(), "count".to_string());
        sv.insert("setter".to_string(), "setCount".to_string());
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            renderer: "flash".to_string(),
            nodes: vec![IRNode {
                node_id: "node_0001".to_string(),
                node_type: "__expr__".to_string(),
                reactive_text: "count".to_string(),
                ..Default::default()
            }],
            shared_vars: vec![sv],
            channel_subs: Vec::new(),
            ..Default::default()
        }
    }

    fn namespaced_shared_window() -> IRWindow {
        let mut window = shared_window();
        window.shared_vars[0].insert("ns".to_string(), "cart_12345678".to_string());
        window
    }

    #[test]
    fn shared_store_emits_accessor_and_maps_reads() {
        let windows = vec![shared_window()];
        let dir = std::env::temp_dir().join(format!("morph_shared_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        let api = std::fs::read_to_string(dir.join("morph_api.h")).unwrap();
        assert!(app.contains("#include \"morph_api.h\""), "api header included: {app}");
        assert!(
            api.contains("inline morph::Signal<int>& shared_cart_count()"),
            "accessor emitted in header: {api}"
        );
        assert!(api.contains("inline int count()"), "getter wrapper: {api}");
        assert!(api.contains("inline void setCount(int v)"), "setter wrapper: {api}");
        assert!(app.contains("shared_cart_count().get()"), "reactive read mapped: {app}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shared_namespace_wraps_accessor_and_maps_qualified_reads() {
        let windows = vec![namespaced_shared_window()];
        let dir = std::env::temp_dir().join(format!("morph_shared_ns_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        let api = std::fs::read_to_string(dir.join("morph_api.h")).unwrap();
        assert!(api.contains("namespace app {"), "module namespace: {api}");
        assert!(api.contains("namespace cart_12345678 {"), "store namespace: {api}");
        assert!(
            app.contains("::app::cart_12345678::shared_cart_count().get()"),
            "qualified read: {app}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn event_window() -> IRWindow {
        use std::collections::HashMap;
        let mut ev = HashMap::new();
        ev.insert("key".to_string(), "/proj/src/Store.mx::resetEvent".to_string());
        ev.insert("ns".to_string(), "store_abc123".to_string());
        ev.insert("accessor".to_string(), "evt_resetEvent".to_string());
        ev.insert("event".to_string(), "resetEvent".to_string());
        ev.insert("channel".to_string(), "evt:/proj/src/Store.mx::resetEvent".to_string());
        ev.insert("module".to_string(), "/proj/src/Store.mx".to_string());
        let mut sub = HashMap::new();
        sub.insert("channel".to_string(), "evt:/proj/src/Store.mx::resetEvent".to_string());
        sub.insert(
            "body".to_string(),
            "[](const JsValue& __ch_0) { __st_inst0_count.set(0); }".to_string(),
        );
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            renderer: "flash".to_string(),
            premain_functions: vec![
                "morph::channel(\"evt:/proj/src/Store.mx::resetEvent\").emit(JsObject{});"
                    .to_string(),
            ],
            event_decls: vec![ev],
            channel_subs: vec![sub],
            ..Default::default()
        }
    }

    #[test]
    fn static_event_channels_replace_string_registry() {
        let windows = vec![event_window()];
        let dir = std::env::temp_dir().join(format!("morph_evt_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        let api = std::fs::read_to_string(dir.join("morph_api.h")).unwrap();
        assert!(!app.contains("morph::channel(\"evt:"), "no string lookup in app.cpp: {app}");
        assert!(
            app.contains("::app::store_abc123::evt_resetEvent().emit(JsObject{});"),
            "emit lowered: {app}"
        );
        assert!(
            app.contains("::app::store_abc123::evt_resetEvent().on([](const JsValue& __ch_0)"),
            "subscribe lowered: {app}"
        );
        assert!(
            api.contains("inline morph::Channel& evt_resetEvent()"),
            "channel static in header: {api}"
        );
        assert!(api.contains("inline void emit_resetEvent("), "emit wrapper: {api}");
        assert!(api.contains("inline void notify_resetEvent()"), "notify wrapper: {api}");
        assert!(api.contains("// resetEvent (/proj/src/Store.mx)"), "mapping comment: {api}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn mid_window() -> IRWindow {
        use std::collections::HashMap;
        let mut sv = HashMap::new();
        sv.insert("getter".to_string(), "inst1_count".to_string());
        sv.insert("setter".to_string(), "inst1_setCount".to_string());
        sv.insert("init".to_string(), "0".to_string());
        sv.insert("instance".to_string(), "1".to_string());
        let mut a = HashMap::new();
        a.insert("key".to_string(), "/proj/Counter.mx::Counter::hero".to_string());
        a.insert("ns".to_string(), "counter".to_string());
        a.insert("comp".to_string(), "Counter".to_string());
        a.insert("mid".to_string(), "hero".to_string());
        a.insert("const".to_string(), "MID_HERO".to_string());
        a.insert("index".to_string(), "0".to_string());
        a.insert("signal".to_string(), "__st_inst1_count".to_string());
        a.insert("getter".to_string(), "count".to_string());
        a.insert("setter".to_string(), "setCount".to_string());
        a.insert("init".to_string(), "0".to_string());
        a.insert("module".to_string(), "/proj/Counter.mx".to_string());
        a.insert("loc".to_string(), "5:7".to_string());
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            renderer: "flash".to_string(),
            state_vars: vec![sv],
            mid_assignments: vec![a],
            ..Default::default()
        }
    }

    #[test]
    fn mid_constants_decls_defs_and_self_test() {
        let windows = vec![mid_window()];
        let dir = std::env::temp_dir().join(format!("morph_mid_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        let api = std::fs::read_to_string(dir.join("morph_api.h")).unwrap();
        assert!(api.contains("constexpr uint32_t MID_HERO = 0;"), "const: {api}");
        assert!(api.contains("// <Counter mid=\"hero\"> (/proj/Counter.mx:5:7)"), "comment: {api}");
        assert!(api.contains("void setCount(uint32_t mid, int v);"), "decl: {api}");
        assert!(api.contains("int count(uint32_t mid);"), "decl: {api}");
        assert!(app.contains("case 0: __st_inst1_count.set(v); break;"), "def: {app}");
        assert!(app.contains("case 0: return __st_inst1_count.get();"), "def: {app}");
        assert!(app.contains("--morph-self-test"), "flag: {app}");
        assert!(
            app.contains("app::counter::setCount(app::counter::MID_HERO, 7);"),
            "self-test: {app}"
        );
        assert!(app.contains("[morph-self-test]"), "summary: {app}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn bindings_window() -> IRWindow {
        use std::collections::HashMap;
        let mut fb = HashMap::new();
        fb.insert("key".to_string(), "/proj/u.ts::loadData".to_string());
        fb.insert("kind".to_string(), "function".to_string());
        fb.insert("ns".to_string(), "u".to_string());
        fb.insert("name".to_string(), "loadData".to_string());
        fb.insert("module".to_string(), "/proj/u.ts".to_string());
        let mut vb = HashMap::new();
        vb.insert("key".to_string(), "/proj/u.ts::token".to_string());
        vb.insert("kind".to_string(), "var".to_string());
        vb.insert("ns".to_string(), "u".to_string());
        vb.insert("name".to_string(), "token".to_string());
        vb.insert("module".to_string(), "/proj/u.ts".to_string());
        let mut cb = HashMap::new();
        cb.insert("key".to_string(), "/proj/u.ts::User".to_string());
        cb.insert("kind".to_string(), "class".to_string());
        cb.insert("ns".to_string(), "u".to_string());
        cb.insert("name".to_string(), "User".to_string());
        cb.insert("module".to_string(), "/proj/u.ts".to_string());
        let mut ab = HashMap::new();
        ab.insert("key".to_string(), "/proj/n.ts::loadData".to_string());
        ab.insert("kind".to_string(), "alias".to_string());
        ab.insert("ns".to_string(), "n".to_string());
        ab.insert("name".to_string(), "loadData".to_string());
        ab.insert("target_ns".to_string(), "u".to_string());
        ab.insert("target_name".to_string(), "loadData".to_string());
        ab.insert("module".to_string(), "/proj/n.ts".to_string());
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            renderer: "flash".to_string(),
            premain_functions: vec![
                "namespace app {\nnamespace u {\nint loadData()\n{\nreturn 1;\n}\n}\n}".to_string(),
                "namespace app {\nnamespace u {\nstd::string token = \"abc\";\n}\n}".to_string(),
                "namespace app {\nnamespace u {\nclass User {\npublic:\nint x;\n};\n}\n}"
                    .to_string(),
            ],
            module_bindings: vec![fb, vb, cb, ab],
            ..Default::default()
        }
    }

    fn list_window() -> IRWindow {
        use morph_ir::IRNode;
        let tmpl = IRNode {
            node_id: "node_0002".to_string(),
            node_type: "__text__".to_string(),
            reactive_text: "item.name".to_string(),
            ..Default::default()
        };
        let list = IRNode {
            node_id: "node_0001".to_string(),
            node_type: "__list__".to_string(),
            list_expr: "items".to_string(),
            list_key_expr: "item.id".to_string(),
            item_template: Some(Box::new(tmpl)),
            list_item_param: "item".to_string(),
            list_index_param: String::new(),
            ..Default::default()
        };
        IRWindow {
            window_id: "main".to_string(),
            title: "Test".to_string(),
            width: 800,
            height: 600,
            visible: true,
            renderer: "flash".to_string(),
            nodes: vec![list],
            ..Default::default()
        }
    }

    #[test]
    fn list_factory_binds_item_param_and_captures() {
        let windows = vec![list_window()];
        let dir = std::env::temp_dir().join(format!("morph_list_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        // Item member access lowers to subscripting on the factory binding.
        assert!(app.contains("__it[\"name\"]"), "item lowers: {app}");
        assert!(!app.contains("item.name"), "no raw item: {app}");
        // Effects referencing the binding capture it; the factory declares it.
        assert!(app.contains("&__it"), "capture: {app}");
        assert!(app.contains("JsValue& __it = __b.item;"), "binding decl: {app}");
        // Key expression uses the same binding.
        assert!(app.contains("__it[\"id\"]"), "key lowers: {app}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn module_bindings_emit_decls_aliases_and_move_classes() {
        let windows = vec![bindings_window()];
        let dir = std::env::temp_dir().join(format!("morph_bindings_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        let api = std::fs::read_to_string(dir.join("morph_api.h")).unwrap();
        // Function declaration + mapping comment.
        assert!(api.contains("int loadData();"), "func decl: {api}");
        assert!(api.contains("// loadData (/proj/u.ts)"), "comment: {api}");
        // Var extern, definition stays in premain.
        assert!(api.contains("extern std::string token;"), "var extern: {api}");
        assert!(app.contains("std::string token = \"abc\";"), "var defn stays: {app}");
        // Class moved whole to the header, gone from premain.
        assert!(api.contains("class User {"), "class moved: {api}");
        assert!(!app.contains("class User {"), "class left premain: {app}");
        // Same-name re-export alias via using.
        assert!(api.contains("using ::app::u::loadData;"), "alias: {api}");
        // Aliases come after class definitions (using needs the target).
        let class_pos = api.find("class User {").expect("class in header");
        let alias_pos = api.find("using ::app::u::loadData;").expect("alias in header");
        assert!(class_pos < alias_pos, "classes before aliases");
        // Function definition itself stays namespaced in app.cpp.
        assert!(app.contains("int loadData()"), "func defn stays: {app}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn dev_window() -> IRWindow {
        use std::collections::HashMap;
        let mut w = mid_window();
        let mut sv = HashMap::new();
        sv.insert("key".to_string(), "cart.count".to_string());
        sv.insert("accessor".to_string(), "shared_cart_count".to_string());
        sv.insert("type".to_string(), "int".to_string());
        sv.insert("init".to_string(), "0".to_string());
        sv.insert("getter".to_string(), "count".to_string());
        sv.insert("setter".to_string(), "setCount".to_string());
        sv.insert("ns".to_string(), "cartstore".to_string());
        w.shared_vars.push(sv);
        let mut ev = HashMap::new();
        ev.insert("key".to_string(), "/proj/CartStore.mx::cartChanged".to_string());
        ev.insert("accessor".to_string(), "evt_cartChanged".to_string());
        ev.insert("event".to_string(), "cartChanged".to_string());
        ev.insert("channel".to_string(), "evt:/proj/CartStore.mx::cartChanged".to_string());
        ev.insert("ns".to_string(), "cartstore".to_string());
        ev.insert("module".to_string(), "/proj/CartStore.mx".to_string());
        w.event_decls.push(ev);
        let mut fb = HashMap::new();
        fb.insert("key".to_string(), "/proj/u.ts::loadData".to_string());
        fb.insert("kind".to_string(), "function".to_string());
        fb.insert("ns".to_string(), "u".to_string());
        fb.insert("name".to_string(), "loadData".to_string());
        fb.insert("module".to_string(), "/proj/u.ts".to_string());
        w.module_bindings.push(fb);
        w.premain_functions.push(
            "namespace app {\nnamespace u {\nint loadData()\n{\nreturn 1;\n}\n}\n}".to_string(),
        );
        let mut ab = HashMap::new();
        ab.insert("key".to_string(), "/proj/n.ts::loadData".to_string());
        ab.insert("kind".to_string(), "alias".to_string());
        ab.insert("ns".to_string(), "n".to_string());
        ab.insert("name".to_string(), "loadData".to_string());
        ab.insert("target_ns".to_string(), "u".to_string());
        ab.insert("target_name".to_string(), "loadData".to_string());
        ab.insert("module".to_string(), "/proj/n.ts".to_string());
        w.module_bindings.push(ab);
        w
    }

    #[test]
    fn dev_header_binds_wrappers_to_dev_tu() {
        let windows = vec![dev_window()];
        let premain: Vec<String> =
            windows.iter().flat_map(|w| w.premain_functions.clone()).collect();
        let api = generate_morph_api_header_dev(&windows, &premain);
        // Shared wrappers call the TU accessor; no signal defined here.
        assert!(
            api.contains("static morph::Signal<int>& shared_cart_count();"),
            "accessor fwd-decl: {api}"
        );
        assert!(
            api.contains(
                "inline int count() { return ::app::cartstore::shared_cart_count().get(); }"
            ),
            "shared read: {api}"
        );
        assert!(
            api.contains(
                "inline void setCount(int v) { ::app::cartstore::shared_cart_count().set(v); }"
            ),
            "shared write: {api}"
        );
        assert!(!api.contains("static morph::Signal<int> s("), "no defns: {api}");
        // Events route through the string-registry channel id.
        assert!(
            api.contains(
                "inline morph::Channel& evt_cartChanged() { return morph::channel(\"evt:/proj/CartStore.mx::cartChanged\"); }"
            ),
            "channel fn: {api}"
        );
        assert!(!api.contains("static morph::Channel c;"), "no static channel: {api}");
        assert!(
            api.contains(
                "inline void emit_cartChanged(const JsValue& payload) { evt_cartChanged().emit(payload); }"
            ),
            "emit: {api}"
        );
        // Mid constants + declarations (definitions live in the dev TU).
        assert!(api.contains("constexpr uint32_t MID_HERO = 0;"), "mid const: {api}");
        assert!(api.contains("void setCount(uint32_t mid, int v);"), "mid decl: {api}");
        // Function declaration for native callers.
        assert!(api.contains("int loadData();"), "func decl: {api}");
        // Same-name re-export alias (target defined in the dev TU above).
        assert!(api.contains("using ::app::u::loadData;"), "alias: {api}");
    }

    #[test]
    fn effect_decls_emit_run_once_and_subscribed_forms() {
        let windows = vec![test_window()];
        let dir = std::env::temp_dir().join(format!("morph_fx_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        CppEmitter::new(&windows).emit(&dir).unwrap();
        let app = std::fs::read_to_string(dir.join("app.cpp")).unwrap();
        assert!(app.contains("auto helper()"), "premain spliced");
        assert!(app.contains("// morphEffect (run once)"), "run-once block");
        assert!(app.contains("auto __ef_fn = []() { run_once(); };"), "run-once call");
        assert!(app.contains("morph::create_effect([]() { track(); });"), "deps subscribe");
        assert!(app.contains("morph::create_effect([]() { always(); });"), "no-deps subscribe");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
