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
            for name in &w.reactive_consts {
                state_map.insert(name.clone(), format!("{name}()"));
            }
            for n in logic_emitter::collect_list_nodes(&w.nodes) {
                if let Some(ref tmpl) = n.item_template {
                    let body = node_emitter::emit_node_with_state(
                        tmpl,
                        None,
                        &fs.features,
                        &state_map,
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

        // Premain code (functions like doLogin, logout). Event emit
        // placeholders lower to static accessors like window code.
        let premain_code = lower_channels(
            self.windows
                .iter()
                .flat_map(|w| w.premain_functions.clone())
                .collect::<Vec<_>>()
                .join("\n\n"),
        );

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

        std::fs::write(output_dir.join("app.cpp"), rendered)?;

        // Per-project native contract: signal/channel definitions +
        // wrappers for user C++. Always generated (not just native mode)
        // so `#include "morph_api.h"` in app.cpp never dangles.
        let api_header = generate_morph_api_header(self.windows, &premain_code, &event_decls);
        std::fs::write(output_dir.join("morph_api.h"), api_header)?;

        // Generate _morph_state.h for native mode (signals + JSX wrappers + function decls)
        if native_mode {
            let state_header = logic_emitter::generate_state_header(
                self.windows,
                std::slice::from_ref(&premain_code),
                false,
            );
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
/// namespace (hand-built IR) falls back to global scope.
fn shared_expr(ns: &str, accessor: &str) -> String {
    if ns.is_empty() {
        format!("{accessor}()")
    } else {
        format!("morph_mods::{ns}::{accessor}()")
    }
}

/// Fully-qualified event channel accessor call. Mirrors `shared_expr`.
fn event_expr(ns: &str, accessor: &str) -> String {
    if ns.is_empty() {
        format!("{accessor}()")
    } else {
        format!("morph_mods::{ns}::{accessor}()")
    }
}

/// Definitions for the `mid` declarations in `morph_api.h`: switch
/// dispatch over per-instance `__st_` statics, emitted in `app.cpp`
/// after the state signals. Bounds-safe by construction (unknown index
/// → no-op; `get_` returns `T{}`). Instance statics are build-time
/// constants with no dynamic lifetime, so no liveness mask is needed:
/// a write to a detached (conditionally unmounted) instance updates
/// its own static harmlessly and never touches another instance.
fn generate_mid_code(windows: &[IRWindow], premain_code: &str) -> String {
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
        // Distinct states: getter → (type, [(index, signal)]).
        let mut states: Vec<(&str, String, Vec<(usize, String)>)> = Vec::new();
        let mut state_idx: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for a in assigns {
            let getter = a.get("getter").map_or("", String::as_str);
            if getter.is_empty() {
                continue;
            }
            let ty = infer_cpp_type(a.get("init").map_or("0", String::as_str));
            if ty == "auto" {
                continue;
            }
            let index = a.get("index").map_or("0", String::as_str).parse::<usize>().unwrap_or(0);
            let signal = a.get("signal").map_or("", String::as_str).to_string();
            match state_idx.get(getter) {
                Some(&i) => states[i].2.push((index, signal)),
                None => {
                    state_idx.insert(getter.to_string(), states.len());
                    states.push((getter, ty, vec![(index, signal)]));
                }
            }
        }
        if states.is_empty() {
            continue;
        }
        out.push("namespace morph_mods {".to_string());
        out.push(format!("namespace {ns} {{"));
        for (getter, ty, mut cases) in states {
            cases.sort_by_key(|(index, _)| *index);
            let set_fn = format!("set_{getter}");
            let get_fn = format!("get_{getter}");
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
                        format!("morph_mods::{ns}::{name}")
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
                lines.push(format!(
                    "    morph_mods::{ns}::set_{getter}(morph_mods::{ns}::{c}, {probe});"
                ));
                lines.push(format!(
                    "    check(morph_mods::{ns}::get_{getter}(morph_mods::{ns}::{c}) == {probe}, \"mid:{c}\");"
                ));
            }
        }
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
/// redefine a user function (same rule as `_morph_state.h`).
fn premain_names(premain_code: &str) -> std::collections::HashSet<String> {
    premain_code
        .split("\n\n")
        .filter_map(logic_emitter::extract_function_decl)
        .filter_map(|decl| logic_emitter::fn_name(&decl))
        .collect()
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
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    let mut block_by_ns: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut push_entry = |ns: &str, entry_lines: Vec<String>| {
        if ns.is_empty() {
            // Global-scope entries are spliced inline at the end; collect
            // them under a sentinel and flush after namespaced blocks.
            if let Some(&idx) = block_by_ns.get("") {
                blocks[idx].1.extend(entry_lines);
            } else {
                block_by_ns.insert(String::new(), blocks.len());
                blocks.push((String::new(), entry_lines));
            }
            return;
        }
        if let Some(&idx) = block_by_ns.get(ns) {
            blocks[idx].1.extend(entry_lines);
        } else {
            block_by_ns.insert(ns.to_string(), blocks.len());
            blocks.push((ns.to_string(), entry_lines));
        }
    };

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
                push_entry(ns, entry);
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
            push_entry(ns, entry);
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
            let comp =
                assigns.first().map(|a| a.get("comp").map_or("", String::as_str)).unwrap_or("");
            let module =
                assigns.first().map(|a| a.get("module").map_or("", String::as_str)).unwrap_or("");
            for (_, c, mid, loc) in &consts {
                entry.push(format!("// <{comp} mid=\"{mid}\"> ({module}:{loc})"));
                entry.push(format!(
                    "constexpr uint32_t {c} = {};",
                    consts.iter().position(|(_, cc, _, _)| cc == c).unwrap_or(0)
                ));
            }
            // One set_/get_ pair per distinct state (suffix getter).
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
            for (getter, _, init) in states {
                let ty = infer_cpp_type(init);
                if ty == "auto" {
                    continue;
                }
                let set_fn = format!("set_{getter}");
                let get_fn = format!("get_{getter}");
                if !taken.contains(set_fn.as_str()) {
                    entry.push(format!("void {set_fn}(uint32_t mid, {ty} v);"));
                }
                if !taken.contains(get_fn.as_str()) {
                    entry.push(format!("{ty} {get_fn}(uint32_t mid);"));
                }
            }
            push_entry(ns.as_str(), entry);
        }
    }

    for (ns, member_lines) in blocks {
        if ns.is_empty() {
            lines.extend(member_lines);
            continue;
        }
        lines.push("namespace morph_mods {".to_string());
        lines.push(format!("namespace {ns} {{"));
        lines.extend(member_lines);
        lines.push("}".to_string());
        lines.push("}".to_string());
    }
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
        assert!(api.contains("namespace morph_mods {"), "module namespace: {api}");
        assert!(api.contains("namespace cart_12345678 {"), "store namespace: {api}");
        assert!(
            app.contains("morph_mods::cart_12345678::shared_cart_count().get()"),
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
            app.contains("morph_mods::store_abc123::evt_resetEvent().emit(JsObject{});"),
            "emit lowered: {app}"
        );
        assert!(
            app.contains("morph_mods::store_abc123::evt_resetEvent().on([](const JsValue& __ch_0)"),
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
        assert!(api.contains("void set_count(uint32_t mid, int v);"), "decl: {api}");
        assert!(api.contains("int get_count(uint32_t mid);"), "decl: {api}");
        assert!(app.contains("case 0: __st_inst1_count.set(v); break;"), "def: {app}");
        assert!(app.contains("case 0: return __st_inst1_count.get();"), "def: {app}");
        assert!(app.contains("--morph-self-test"), "flag: {app}");
        assert!(
            app.contains("morph_mods::counter::set_count(morph_mods::counter::MID_HERO, 7);"),
            "self-test: {app}"
        );
        assert!(app.contains("[morph-self-test]"), "summary: {app}");
        let _ = std::fs::remove_dir_all(&dir);
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
