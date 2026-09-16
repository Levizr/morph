//! Dev hot-reload logic emitter.
//!
//! Mirrors `morph/codegen/logic_emitter.py` (`emit_logic`): generates the
//! `app_logic.cpp` translation unit that `morph_devrt` dlopens. Effects and
//! event wiring resolve nodes through the dev `NodeRegistry`
//! (`nodes.get("<id>")`) instead of direct variables, so a rebuilt tree
//! rewires without recompiling. JS expressions are translated with the
//! `morpher` snippet translator against the ambient state map.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use morph_ir::{IRNode, IRWindow};

use super::node_emitter::{
    css_field_reset, css_to_style_field, css_val_to_cpp, emit_node_with_state, translate_condition,
    translate_js, translate_list_key,
};

/// Generated interop header name (mirrors Python `_STATE_HEADER_NAME`).
pub const STATE_HEADER_NAME: &str = "_morph_state.h";

/// Output of [`emit_logic`]: the logic translation unit plus the optional
/// native-mode interop header.
pub struct LogicOutput {
    pub source: String,
    pub state_header: Option<String>,
}

const LOGIC_PREHEADER: &str = r#"#include <cstdio>
#include <string>
// Dev fast-reload prelude: extern template decls for Signal<T> /
// get_or_create<T> + console.log sinks, all resolved from the devrt host.
#include "logic_prelude.h"
#include "core/node.h"
#include "reactivity/signal.h"
#include "reactivity/channel.h"
#include "core/event.h"
#include "types/js_value.h"
#include "ui/image.h"

// Node registry and signal store (included via -I<runtime>/dev)
#include "signal_store.h"
#include "node_registry.h"

// Runtime color parser for reactive style color expressions
namespace morph {
inline void setColor(float rgba[4], const std::string& c) {
    if (c.size() == 7 && c[0] == '#') {
        rgba[0] = std::stoi(c.substr(1,2), nullptr, 16) / 255.0f;
        rgba[1] = std::stoi(c.substr(3,2), nullptr, 16) / 255.0f;
        rgba[2] = std::stoi(c.substr(5,2), nullptr, 16) / 255.0f;
        rgba[3] = 1.0f;
    } else if (c.size() == 4 && c[0] == '#') {
        rgba[0] = std::stoi(c.substr(1,1) + c[1], nullptr, 16) / 255.0f;
        rgba[1] = std::stoi(c.substr(2,1) + c[2], nullptr, 16) / 255.0f;
        rgba[2] = std::stoi(c.substr(3,1) + c[3], nullptr, 16) / 255.0f;
        rgba[3] = 1.0f;
    }
}
}

"#;

const LOGIC_FOOTER: &str = r#"
void morph_logic_cleanup() {
    for (int i = 0; i < __effect_count; i++) {
        if (__effects[i]) {
            __effects[i]->cleanup();
            __effects[i] = nullptr;
        }
    }
    __effect_count = 0;
}

} // extern "C"
"#;

/// Headers already provided by the preheader or compiler include paths.
const BUILTIN_HEADERS: &[&str] = &[
    "<cstdio>",
    "<string>",
    "\"logic_prelude.h\"",
    "\"core/node.h\"",
    "\"reactivity/signal.h\"",
    "\"reactivity/channel.h\"",
    "\"core/event.h\"",
    "\"types/js_value.h\"",
    "\"signal_store.h\"",
    "\"node_registry.h\"",
];

/// Keyed-list factory emission needs every widget header, like the devrt.
const DEV_FEATURES: &[&str] = &[
    "scroll",
    "radius",
    "text",
    "bold",
    "position",
    "zindex",
    "opacity",
    "flex",
    "cursor",
    "border",
    "transform",
    "animation",
    "display_none",
    "inline",
    "margin_collapse",
    "min_max",
    "border_box",
    "image",
    "button",
    "input",
    "event",
    "hover",
    "active",
    "dirty_rendering",
];

/// Ambient translation maps: JS name → C++ expression / C++ type.
struct AmbientMaps {
    vars: HashMap<String, String>,
    types: HashMap<String, String>,
}

fn shared_static(accessor: &str) -> String {
    format!("__{accessor}")
}

/// Optional per-module namespace in the generated program (`morph_mods::<ns>`).
/// Absent for hand-built IR (unit tests / legacy), which falls back to the
/// global-scope emission below.
fn shared_ns(sv: &HashMap<String, String>) -> String {
    sv.get("ns").cloned().unwrap_or_default()
}

/// Fully-qualified accessor call (`morph_mods::<ns>::shared_x()`).
fn shared_ref(ns: &str, accessor: &str) -> String {
    if ns.is_empty() {
        format!("{accessor}()")
    } else {
        format!("morph_mods::{ns}::{accessor}()")
    }
}

/// Fully-qualified backing signal (`morph_mods::<ns>::__shared_x`).
fn shared_backing_ref(ns: &str, accessor: &str) -> String {
    if ns.is_empty() {
        shared_static(accessor)
    } else {
        format!("morph_mods::{ns}::{}", shared_static(accessor))
    }
}

/// Emit declarations grouped under their optional `morph_mods::<ns>`
/// namespaces. Empty namespaces fall back to global scope.
fn emit_shared_entries(lines: &mut Vec<String>, decls: Vec<(String, String)>) {
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    let mut block_by_ns: HashMap<String, usize> = HashMap::new();
    let mut bare: Vec<String> = Vec::new();
    for (ns, decl) in decls {
        if ns.is_empty() {
            bare.push(decl);
            continue;
        }
        if let Some(&idx) = block_by_ns.get(&ns) {
            blocks[idx].1.push(decl);
        } else {
            block_by_ns.insert(ns.clone(), blocks.len());
            blocks.push((ns, vec![decl]));
        }
    }
    lines.extend(bare);
    for (ns, member_lines) in blocks {
        lines.push("namespace morph_mods {".to_string());
        lines.push(format!("namespace {ns} {{"));
        lines.extend(member_lines);
        lines.push("}".to_string());
        lines.push("}".to_string());
    }
}

fn ambient_maps(windows: &[IRWindow]) -> AmbientMaps {
    let mut vars = HashMap::new();
    let mut types = HashMap::new();
    for w in windows {
        for sv in &w.state_vars {
            let getter = sv.get("getter").map_or("", String::as_str);
            if getter.is_empty() {
                continue;
            }
            vars.insert(getter.to_string(), format!("__st_{getter}.get()"));
            if let Some(init) = sv.get("init") {
                if let Some(ty) = infer_state_type(init) {
                    types.insert(getter.to_string(), ty);
                }
            }
            if let Some(setter) = sv.get("setter") {
                if !setter.is_empty() {
                    vars.insert(setter.clone(), format!("__st_{getter}.set"));
                }
            }
        }
        for sv in &w.shared_vars {
            let (getter, setter, accessor) = (
                sv.get("getter").map_or("", String::as_str),
                sv.get("setter").map_or("", String::as_str),
                sv.get("accessor").map_or("", String::as_str),
            );
            if getter.is_empty() || accessor.is_empty() {
                continue;
            }
            let ns = shared_ns(sv);
            let read = format!("{}.get()", shared_ref(&ns, accessor));
            vars.insert(getter.to_string(), read);
            if !setter.is_empty() {
                vars.insert(setter.to_string(), format!("{}.set", shared_ref(&ns, accessor)));
            }
            if let Some(ty) = sv.get("type").filter(|t| *t != "auto") {
                types.insert(getter.to_string(), ty.clone());
            }
        }
        for name in &w.reactive_consts {
            vars.insert(name.clone(), format!("{name}()"));
        }
    }
    AmbientMaps { vars, types }
}

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

/// Translate a JS expression to C++ with morpher against the ambient map,
/// falling back to the textual substitution when morpher cannot parse it.
fn translate_expr(js: &str, maps: &AmbientMaps) -> String {
    let mut options = morpher::TranslateOptions::default();
    options.state_vars.clone_from(&maps.vars);
    options.state_types.clone_from(&maps.types);
    if let Ok(out) = morpher::translate_snippet(js, "snippet.ts", options) {
        let body = out.body.trim().trim_end_matches(';').trim().to_string();
        if !body.is_empty() {
            return body;
        }
    }
    translate_js(js, &maps.vars).trim().trim_end_matches(';').to_string()
}

/// Translate an event handler target to a `void(JsObject)` lambda body.
fn translate_handler(target: &str, maps: &AmbientMaps) -> String {
    let trimmed = target.trim_start();
    if trimmed.starts_with("[&]") || trimmed.starts_with("[]") {
        return target.to_string();
    }
    let mut body = target.trim().to_string();
    if let Some(idx) = body.find("=>") {
        let after = body[idx + 2..].trim();
        body = if after.starts_with('{') && after.ends_with('}') && after.len() >= 2 {
            after[1..after.len() - 1].trim().to_string()
        } else {
            after.to_string()
        };
    }
    let cpp = translate_expr(&body, maps);
    format!("[](JsObject e) {{ {}; }}", cpp.trim().trim_end_matches(';'))
}

/// Translate a branch condition (already C++-leaning from the builder is
/// fine verbatim, but raw JS must go through morpher first).
fn translate_cond(condition: &str, maps: &AmbientMaps) -> String {
    let translated = translate_expr(condition, maps);
    if translated == condition.trim() {
        translate_condition(condition, &maps.vars)
    } else {
        translated
    }
}

fn event_member(trigger: &str) -> &'static str {
    match trigger {
        "keyup" => "onKeyUp",
        "keydown" => "onKeyDown",
        "dblclick" => "onDoubleClick",
        "mousedown" => "onMouseDown",
        "mouseup" => "onMouseUp",
        "mouseenter" => "onMouseEnter",
        "mouseleave" => "onMouseLeave",
        "change" => "onChange",
        "input" => "onInput",
        "focus" => "onFocus",
        "blur" => "onBlur",
        // Unknown triggers and plain clicks share the click handler,
        // mirroring Python's `.get(trigger, "onClick")` fallback.
        _ => "onClick",
    }
}

fn emit_node_events(
    lines: &mut Vec<String>,
    node: &IRNode,
    maps: &AmbientMaps,
    wired: &mut HashSet<(String, String)>,
    indent: &str,
) {
    if node.node_type == "__expr__" || node.node_type == "__text__" {
        return;
    }
    if node.node_type == "__conditional__" {
        for child in node.then_nodes.iter().chain(node.else_nodes.iter()) {
            emit_node_events(lines, child, maps, wired, indent);
        }
        return;
    }
    for event in &node.events {
        let member = event_member(event.trigger.as_str());
        if !wired.insert((node.node_id.clone(), member.to_string())) {
            continue;
        }
        match event.action.as_str() {
            "call" => {
                let rhs = translate_handler(&event.target, maps);
                lines.push(format!("{indent}if (auto* n = nodes.get(\"{}\")) {{", node.node_id));
                lines.push(format!("{indent}    n->{member} = {rhs};"));
                lines.push(format!("{indent}}}"));
            }
            // open/close/navigate need multi-window management (build
            // only); unknown actions are skipped silently, like Python.
            "log" => {
                let escaped = event.target.replace('"', "\\\"");
                lines.push(format!("{indent}if (auto* n = nodes.get(\"{}\")) {{", node.node_id));
                lines.push(format!(
                    "{indent}    n->{member} = [](JsObject) {{ fprintf(stderr, \"{escaped}\\n\"); }};"
                ));
                lines.push(format!("{indent}}}"));
            }
            _ => {}
        }
    }
    for child in &node.children {
        emit_node_events(lines, child, maps, wired, indent);
    }
}

fn emit_text_effect(lines: &mut Vec<String>, node: &IRNode, maps: &AmbientMaps, indent: &str) {
    if node.reactive_text.is_empty() {
        return;
    }
    let cpp = translate_expr(&node.reactive_text, maps);
    lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
    lines.push(format!("{indent}    auto* n = nodes.get(\"{}\");", node.node_id));
    lines.push(format!("{indent}    if (n) n->setText(morph::str({cpp}));"));
    lines.push(format!("{indent}}});"));
}

fn emit_conditional_effect(
    lines: &mut Vec<String>,
    node: &IRNode,
    maps: &AmbientMaps,
    indent: &str,
) {
    let id = node.node_id.as_str();
    let then_id = node.then_nodes.first().map_or("", |n| n.node_id.as_str());
    let else_id = node.else_nodes.first().map_or("", |n| n.node_id.as_str());
    let condition = translate_cond(&node.condition_expr, maps);
    lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
    lines.push(format!("{indent}    auto* container = nodes.get(\"{id}\");"));
    lines.push(format!("{indent}    if (!container) return;"));
    if !node.condition_expr.is_empty() {
        lines.push(format!("{indent}    container->removeAllChildren();"));
        lines.push(format!("{indent}    if ({condition}) {{"));
        if !then_id.is_empty() {
            lines.push(format!("{indent}        auto* child = nodes.get(\"{then_id}\");"));
            lines.push(format!("{indent}        if (child) {{"));
            lines.push(format!("{indent}            container->addChild(child);"));
            lines.push(format!(
                "{indent}            container->style.explicitWidth = child->style.explicitWidth;"
            ));
            lines.push(format!(
                "{indent}            container->style.explicitHeight = child->style.explicitHeight;"
            ));
            lines.push(format!("{indent}        }}"));
        }
        lines.push(format!("{indent}    }} else {{"));
        if !else_id.is_empty() {
            lines.push(format!("{indent}        auto* child = nodes.get(\"{else_id}\");"));
            lines.push(format!("{indent}        if (child) {{"));
            lines.push(format!("{indent}            container->addChild(child);"));
            lines.push(format!(
                "{indent}            container->style.explicitWidth = child->style.explicitWidth;"
            ));
            lines.push(format!(
                "{indent}            container->style.explicitHeight = child->style.explicitHeight;"
            ));
            lines.push(format!("{indent}        }}"));
        }
        lines.push(format!("{indent}    }}"));
    }
    lines.push(format!("{indent}}});"));
}

fn emit_list_wiring(lines: &mut Vec<String>, node: &IRNode, maps: &AmbientMaps, indent: &str) {
    let id = node.node_id.as_str();
    let array_expr = translate_expr(&node.list_expr, maps);
    let array_expr = array_expr.trim().trim_end_matches(';').to_string();
    lines.push(format!(
        "{indent}if (auto* lc = dynamic_cast<morph::ListContainer*>(nodes.get(\"{id}\"))) {{"
    ));
    lines.push(format!("{indent}    lc->arrayFn = [&]() {{ return {array_expr}; }};"));
    lines.push(format!("{indent}    lc->itemFactory = __list_factory_{id};"));
    if !node.list_key_expr.is_empty() {
        let key_expr = translate_list_key(&node.list_key_expr);
        lines.push(format!(
            "{indent}    lc->keyFn = [&](const JsValue& __it, int __index) -> std::string {{"
        ));
        lines.push(format!("{indent}        return morph::list_key({key_expr}, __index);"));
        lines.push(format!("{indent}    }};"));
    }
    lines.push(format!("{indent}}}"));
    lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
    lines.push(format!(
        "{indent}    auto* lc = dynamic_cast<morph::ListContainer*>(nodes.get(\"{id}\"));"
    ));
    lines.push(format!("{indent}    if (!lc) return;"));
    lines.push(format!("{indent}    lc->reconcile(lc->arrayFn());"));
    lines.push(format!("{indent}}});"));
}

/// Reactive inline style expressions → one effect per property.
fn emit_style_effects(lines: &mut Vec<String>, node: &IRNode, maps: &AmbientMaps, indent: &str) {
    let id = node.node_id.as_str();
    if node.reactive_style.is_empty() {
        return;
    }
    let mut props: Vec<(&String, &String)> = node.reactive_style.iter().collect();
    props.sort_by(|a, b| a.0.cmp(b.0));
    for (css_prop, raw_expr) in props {
        let Some((field_name, val_type)) = css_to_style_field(css_prop) else { continue };
        let cpp = translate_expr(raw_expr, maps);
        let cpp = cpp.trim().trim_end_matches(';').to_string();
        let assignment = match val_type {
            "float" => format!("n->style.{field_name} = (float)({cpp});"),
            "string" => format!("n->style.{field_name} = morph::str({cpp});"),
            "color" => format!("morph::setColor(n->style.{field_name}, morph::str({cpp}));"),
            "transform" => {
                format!("morph::setCssTransform(n->style, morph::str({cpp}), n->w, n->h);")
            }
            _ => continue,
        };
        lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
        lines.push(format!("{indent}    auto* n = nodes.get(\"{id}\");"));
        lines.push(format!("{indent}    if (!n) return;"));
        lines.push(format!("{indent}    n->interruptStateTransitions();"));
        lines.push(format!("{indent}    {assignment}"));
        lines.push(format!("{indent}    n->markDirty(PaintDirty);"));
        if val_type == "float" {
            lines.push(format!("{indent}    n->markDirty(LayoutDirty);"));
        }
        lines.push(format!("{indent}}});"));
    }
}

/// A container font-size expression also applies to direct TextNode children.
fn emit_font_size_effects(
    lines: &mut Vec<String>,
    node: &IRNode,
    maps: &AmbientMaps,
    indent: &str,
) {
    let Some(raw_expr) = node.reactive_style.get("font-size") else { return };
    if css_to_style_field("font-size").map(|(_, t)| t) != Some("float") {
        return;
    }
    let cpp = translate_expr(raw_expr, maps);
    let cpp = cpp.trim().trim_end_matches(';').to_string();
    for child in &node.children {
        if child.node_type != "__text__" {
            continue;
        }
        let child_id = child.node_id.as_str();
        lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
        lines.push(format!("{indent}    auto* n = nodes.get(\"{child_id}\");"));
        lines.push(format!("{indent}    if (!n) return;"));
        lines.push(format!("{indent}    n->interruptStateTransitions();"));
        lines.push(format!("{indent}    n->style.fontSize = (float)({cpp});"));
        lines.push(format!("{indent}    n->markDirty(LayoutDirty);"));
        lines.push(format!("{indent}    n->markDirty(PaintDirty);"));
        lines.push(format!("{indent}}});"));
    }
}

/// Reactive attrs (src on img, value on input, generic otherwise).
fn emit_attr_effects(lines: &mut Vec<String>, node: &IRNode, maps: &AmbientMaps, indent: &str) {
    let id = node.node_id.as_str();
    if node.reactive_attrs.is_empty() {
        return;
    }
    let mut attrs: Vec<(&String, &String)> = node.reactive_attrs.iter().collect();
    attrs.sort_by(|a, b| a.0.cmp(b.0));
    for (attr_key, raw_expr) in attrs {
        let cpp = translate_expr(raw_expr, maps);
        let cpp = cpp.trim().trim_end_matches(';').to_string();
        lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
        lines.push(format!("{indent}    auto* n = nodes.get(\"{id}\");"));
        lines.push(format!("{indent}    if (!n) return;"));
        if attr_key == "src" && node.node_type == "img" {
            lines.push(format!("{indent}    auto* img = static_cast<ImageNode*>(n);"));
            lines.push(format!("{indent}    std::string _src = morph::str({cpp});"));
            lines.push(format!("{indent}    if (img->src != _src) {{"));
            lines.push(format!("{indent}        img->src = _src;"));
            lines.push(format!("{indent}        img->loaded = false;"));
            lines.push(format!("{indent}    }}"));
        } else if attr_key == "value" && node.node_type == "input" {
            lines.push(format!(
                "{indent}    static_cast<InputNode*>(n)->setValue(morph::str({cpp}));"
            ));
        } else {
            lines.push(format!("{indent}    n->{attr_key} = morph::str({cpp});"));
        }
        lines.push(format!("{indent}    n->markDirty(PaintDirty);"));
        lines.push(format!("{indent}}});"));
    }
}

/// Reactive className binding.
fn emit_class_effect(lines: &mut Vec<String>, node: &IRNode, indent: &str) {
    if node.reactive_class.is_empty() {
        return;
    }
    let id = node.node_id.as_str();
    let cpp = node.reactive_class.trim().trim_end_matches(';').trim().to_string();
    lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
    lines.push(format!("{indent}    auto* n = nodes.get(\"{id}\");"));
    lines.push(format!("{indent}    if (!n) return;"));
    lines.push(format!("{indent}    n->setClassName(morph::str({cpp}));"));
    lines.push(format!("{indent}}});"));
}

/// Direct condition → style effects (no string searching).
fn emit_conditional_class_effects(lines: &mut Vec<String>, node: &IRNode, indent: &str) {
    let id = node.node_id.as_str();
    for effect in &node.class_conditional_effects {
        let cond_cpp = effect.condition.trim().trim_end_matches(';').trim().to_string();
        lines.push(format!("{indent}__effects[__effect_count++] = morph::create_effect([&]() {{"));
        lines.push(format!("{indent}    auto* n = nodes.get(\"{id}\");"));
        lines.push(format!("{indent}    if (!n) return;"));
        lines.push(format!("{indent}    n->interruptStateTransitions();"));
        lines.push(format!("{indent}    if ({cond_cpp}) {{"));
        let mut on_props: Vec<(&String, &String)> = effect.on_styles.iter().collect();
        on_props.sort_by(|a, b| a.0.cmp(b.0));
        for (prop, val) in &on_props {
            for assignment in css_val_to_cpp("n", prop, val, indent) {
                lines.push(assignment);
            }
        }
        lines.push(format!("{indent}    }} else {{"));
        if effect.off_styles.is_empty() {
            let mut reset_fields: Vec<&str> = Vec::new();
            for prop in on_props.iter().map(|p| p.0.as_str()) {
                if let Some((field, _)) = css_to_style_field(prop) {
                    if !reset_fields.contains(&field) {
                        reset_fields.push(field);
                    }
                }
            }
            reset_fields.sort_unstable();
            for field in reset_fields {
                for assignment in css_field_reset("n", field, indent) {
                    lines.push(assignment);
                }
            }
        } else {
            let mut off_props: Vec<(&String, &String)> = effect.off_styles.iter().collect();
            off_props.sort_by(|a, b| a.0.cmp(b.0));
            for (prop, val) in off_props {
                for assignment in css_val_to_cpp("n", prop, val, indent) {
                    lines.push(assignment);
                }
            }
        }
        lines.push(format!("{indent}    }}"));
        lines.push(format!("{indent}    n->markDirty(PaintDirty);"));
        lines.push(format!("{indent}}});"));
    }
}

fn emit_node_effects(lines: &mut Vec<String>, node: &IRNode, maps: &AmbientMaps, indent: &str) {
    if node.node_type == "__text__" || node.node_type == "__expr__" {
        emit_text_effect(lines, node, maps, indent);
        return;
    }

    if node.node_type == "__conditional__" {
        emit_conditional_effect(lines, node, maps, indent);
        for child in node.then_nodes.iter().chain(node.else_nodes.iter()) {
            emit_node_effects(lines, child, maps, indent);
        }
        return;
    }

    if node.node_type == "__list__" {
        emit_list_wiring(lines, node, maps, indent);
        return;
    }

    // ── Reactive style (inline style expressions) ──
    emit_style_effects(lines, node, maps, indent);

    // ── Propagate font-size to child TextNodes ──
    emit_font_size_effects(lines, node, maps, indent);

    // ── Reactive attrs (src/alt on img, value on input) ──
    emit_attr_effects(lines, node, maps, indent);

    // ── Reactive className ──
    emit_class_effect(lines, node, indent);

    // ── Conditional class style effects ──
    emit_conditional_class_effects(lines, node, indent);

    for child in &node.children {
        emit_node_effects(lines, child, maps, indent);
    }
}

/// C++ type for a `morphState` init literal (mirrors `_get_cpp_type`).
fn infer_cpp_type(init: &str) -> String {
    let raw = init.trim();
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return "std::string".to_string();
    }
    if raw == "true" || raw == "false" {
        return "bool".to_string();
    }
    if raw.starts_with('"') {
        return "std::string".to_string();
    }
    if raw.starts_with('[') {
        return "JsArray".to_string();
    }
    if raw.contains('.') {
        return "double".to_string();
    }
    if raw.parse::<i64>().is_ok() || (raw.starts_with('-') && raw[1..].parse::<i64>().is_ok()) {
        return "int".to_string();
    }
    "auto".to_string()
}

/// Clean a `morphState` init literal for C++ (quotes → `"`, arrays → `JsArray`).
fn clean_init(init: &str) -> String {
    let raw = init.trim();
    if raw.starts_with('\'') && raw.ends_with('\'') && raw.len() >= 2 {
        return format!("\"{}\"", &raw[1..raw.len() - 1]);
    }
    if raw.starts_with('[') && raw.ends_with(']') {
        return array_init_to_cpp(raw);
    }
    raw.to_string()
}

/// Convert a JS array literal to a `JsArray` initializer expression.
fn array_init_to_cpp(raw: &str) -> String {
    let stripped = raw.trim();
    if !stripped.starts_with('[') || !stripped.ends_with(']') {
        return stripped.to_string();
    }
    let inner = stripped[1..stripped.len() - 1].trim();
    if inner.is_empty() {
        return "JsArray{}".to_string();
    }
    let mut items = Vec::new();
    for part in split_top_level(inner) {
        let part = part.trim();
        if part.starts_with('\'') && part.ends_with('\'') && part.len() >= 2 {
            items.push(format!("\"{}\"", part[1..part.len() - 1].replace('"', "\\\"")));
        } else if part == "true" || part == "false" {
            items.push(part.to_string());
        } else if part == "null" {
            items.push("JsNull{}".to_string());
        } else if part == "undefined" {
            items.push("JsUndefined{}".to_string());
        } else if part.starts_with('[') && part.ends_with(']') {
            items.push(array_init_to_cpp(part));
        } else {
            items.push(part.to_string());
        }
    }
    format!("JsArray{{{}}}", items.join(", "))
}

/// Split on top-level commas, respecting quotes and nested brackets.
fn split_top_level(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut current = String::new();
    for ch in s.chars() {
        if let Some(q) = quote {
            current.push(ch);
            if ch == q {
                quote = None;
            }
        } else if ch == '\'' || ch == '"' {
            quote = Some(ch);
            current.push(ch);
        } else if ch == '[' || ch == '{' || ch == '(' {
            depth += 1;
            current.push(ch);
        } else if ch == ']' || ch == '}' || ch == ')' {
            depth -= 1;
            current.push(ch);
        } else if ch == ',' && depth == 0 {
            parts.push(current.trim().to_string());
            current = String::new();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

/// Guard expressions for an effect deps list when every dep is a known
/// state or shared getter.
fn effect_dep_exprs(
    deps: &str,
    state_getters: &HashSet<String>,
    shared: &HashMap<String, String>,
) -> Option<Vec<String>> {
    let deps = deps.trim();
    if deps.is_empty() || deps == "[]" {
        return None;
    }
    let mut seen = HashSet::new();
    let mut exprs = Vec::new();
    for token in deps.split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '$') {
        if token.is_empty() || !seen.insert(token.to_string()) {
            continue;
        }
        if let Some(expr) = shared.get(token) {
            exprs.push(expr.clone());
        } else if state_getters.contains(token) {
            exprs.push(format!("__st_{token}.get()"));
        } else {
            return None;
        }
    }
    if exprs.is_empty() {
        None
    } else {
        Some(exprs)
    }
}

fn has_input(nodes: &[IRNode]) -> bool {
    nodes.iter().any(|n| {
        n.node_type == "input"
            || has_input(&n.children)
            || has_input(&n.then_nodes)
            || has_input(&n.else_nodes)
    })
}

pub fn collect_list_nodes(nodes: &[IRNode]) -> Vec<&IRNode> {
    let mut out = Vec::new();
    for n in nodes {
        if n.node_type == "__list__" {
            out.push(n);
        }
        if let Some(ref tmpl) = n.item_template {
            out.extend(collect_list_nodes(std::slice::from_ref(tmpl)));
        }
        out.extend(collect_list_nodes(&n.children));
        out.extend(collect_list_nodes(&n.then_nodes));
        out.extend(collect_list_nodes(&n.else_nodes));
    }
    out
}

fn dev_features() -> HashSet<String> {
    DEV_FEATURES.iter().map(ToString::to_string).collect()
}

/// File-scope item factory for a `__list__` node (mirrors Python
/// `emit_item_factory` + the build-mode factory block in `cpp/mod.rs`).
fn emit_list_factory(node: &IRNode, maps: &AmbientMaps) -> Option<String> {
    let tmpl = node.item_template.as_deref()?;
    let features = dev_features();
    let body = emit_node_with_state(tmpl, None, &features, &maps.vars, None);
    let mut caps = String::new();
    if body.contains("__it") {
        caps.push_str(", &__it");
    }
    if body.contains("__index") {
        caps.push_str(", &__index");
    }
    let body = body.replace("__LCAPS__", &caps);
    let mut factory = format!(
        "static MorphNode* __list_factory_{}(morph::ListItemBinding& __b) {{\n",
        node.node_id
    );
    if body.contains("__it") {
        factory.push_str("    JsValue& __it = __b.item;\n");
    }
    if body.contains("__index") {
        factory.push_str("    int& __index = __b.index;\n");
    }
    factory.push_str(&body);
    let _ = writeln!(factory, "\n    return {tmpl_node};\n}}", tmpl_node = tmpl.node_id.as_str());
    Some(factory)
}

/// Remove `static inline` / `static` linkage so the function gets external
/// linkage (visible to user `.cpp` code in native mode).
fn strip_static_function(cpp: &str) -> String {
    let text = cpp.trim();
    for prefix in ["static inline", "static", "inline"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            if rest.starts_with(char::is_whitespace) {
                return rest.trim_start().to_string();
            }
        }
    }
    text.to_string()
}

/// Extract `ret name(params);` from transpiled C++ (mirrors
/// `extract_function_decl`). Returns None for globals, lambdas and `main`.
pub(crate) fn extract_function_decl(cpp: &str) -> Option<String> {
    let mut text = cpp.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let mut template_prefix = String::new();
    while text.starts_with("template") {
        let end = text.find('>')?;
        template_prefix = text[..=end].to_string();
        text = text[end + 1..].trim_start().to_string();
    }
    for prefix in ["static inline", "static", "inline"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            if rest.starts_with(char::is_whitespace) {
                text = rest.trim_start().to_string();
                break;
            }
        }
    }
    let mut depth = 0i32;
    for (i, ch) in text.char_indices() {
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
        } else if ch == '{' && depth == 0 {
            let header = text[..i].trim_end();
            if !header.contains('(') || !header.contains(')') {
                return None;
            }
            if header.contains("= [") {
                return None;
            }
            if has_word(header, "main") {
                return None;
            }
            if template_prefix.is_empty() {
                return Some(format!("{header};"));
            }
            return Some(format!("{template_prefix}\n{header};"));
        }
    }
    None
}

fn has_word(text: &str, word: &str) -> bool {
    let mut start = 0;
    while let Some(pos) = text[start..].find(word) {
        let abs = start + pos;
        let before_ok = abs == 0 || !is_ident_char(text.as_bytes()[abs - 1]);
        let after_ok =
            abs + word.len() >= text.len() || !is_ident_char(text.as_bytes()[abs + word.len()]);
        if before_ok && after_ok {
            return true;
        }
        start = abs + 1;
    }
    false
}

const fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Build the `_morph_state.h` content for native mode (mirrors
/// `generate_state_header`): extern signals, JSX state wrappers and JSX
/// function declarations. Per-instance signals are skipped; shared-store
/// wrappers use the accessor in build mode, the backing static in dev mode.
pub fn generate_state_header(windows: &[IRWindow], premain: &[String], dev_mode: bool) -> String {
    let mut lines = vec![
        "#pragma once".to_string(),
        "// Generated by Morph — do not edit".to_string(),
        "#include \"morph_api.h\"".to_string(),
        String::new(),
    ];
    let mut jsx_names = HashSet::new();
    for part in premain {
        if let Some(decl) = extract_function_decl(part) {
            if let Some(name) = fn_name(&decl) {
                jsx_names.insert(name);
            }
        }
    }
    let mut signals: Vec<(String, String)> = Vec::new();
    let mut seen_signals = HashSet::new();
    for w in windows {
        for sv in &w.state_vars {
            if morph_ir::is_instance_slot(sv) {
                continue;
            }
            let getter = sv.get("getter").map_or("", String::as_str);
            if getter.is_empty() || !seen_signals.insert(getter.to_string()) {
                continue;
            }
            let init = sv.get("init").map_or("0", String::as_str);
            signals.push((format!("__st_{getter}"), infer_cpp_type(init)));
        }
    }
    if !signals.is_empty() {
        lines.push("// ── morphState signals (defined in the generated TU) ──".to_string());
        for (signal, cpp_type) in &signals {
            lines.push(format!("extern morph::Signal<{cpp_type}> {signal};"));
        }
        lines.push(String::new());
        lines.push("// ── JSX state wrappers — call from C++ like setState() ──".to_string());
        for (signal, cpp_type) in &signals {
            let base = signal.strip_prefix("__st_").unwrap_or(signal);
            if !jsx_names.contains(base) {
                lines.push(format!("inline {cpp_type} {base}() {{ return {signal}.get(); }}"));
            }
            let upper = base[..1].to_uppercase();
            let setter = format!("set{upper}{}", &base[1..]);
            if !jsx_names.contains(setter.as_str()) {
                lines.push(format!("inline void {setter}({cpp_type} v) {{ {signal}.set(v); }}"));
            }
        }
        lines.push(String::new());
    }
    // morphShared wrappers are NOT emitted here: the generated
    // `morph_api.h` (always included before this header) already defines
    // them exactly once — duplicating them here is a redefinition error.
    let mut func_decls = Vec::new();
    let mut seen_decls = HashSet::new();
    for part in premain {
        if let Some(decl) = extract_function_decl(part) {
            if seen_decls.insert(decl.clone()) {
                func_decls.push(decl);
            }
        }
    }
    if !func_decls.is_empty() {
        lines.push("// ── JSX functions (defined in the generated TU) ──".to_string());
        lines.extend(func_decls);
    }
    lines.join("\n")
}

pub(crate) fn fn_name(decl: &str) -> Option<String> {
    let paren = decl.find('(')?;
    let before = decl[..paren].trim_end();
    let name: String = before
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$')
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// File-scope signal statics (persist across `morph_logic_init` calls).
fn emit_signal_statics(lines: &mut Vec<String>, windows: &[IRWindow], native_mode: bool) {
    let mut seen_signals = HashSet::new();
    let mut signal_lines = Vec::new();
    for w in windows {
        for sv in &w.state_vars {
            let getter = sv.get("getter").map_or("", String::as_str);
            if getter.is_empty() || !seen_signals.insert(getter.to_string()) {
                continue;
            }
            let init = sv.get("init").map_or("0", String::as_str);
            let cleaned = clean_init(init);
            let cpp_type = infer_cpp_type(init);
            if native_mode {
                signal_lines.push(format!("morph::Signal<{cpp_type}> __st_{getter}({cleaned});"));
            } else {
                signal_lines
                    .push(format!("static morph::Signal<{cpp_type}> __st_{getter}({cleaned});"));
            }
        }
    }
    if !signal_lines.is_empty() {
        lines.push(String::new());
        lines.push("// ── morphState signals ──".to_string());
        lines.extend(signal_lines);
    }
    let mut seen_keys = HashSet::new();
    let mut shared_lines: Vec<(String, String)> = Vec::new();
    for w in windows {
        for sv in &w.shared_vars {
            let key = sv.get("key").map_or("", String::as_str);
            let accessor = sv.get("accessor").map_or("", String::as_str);
            if key.is_empty() || accessor.is_empty() || !seen_keys.insert(key.to_string()) {
                continue;
            }
            let init = sv.get("init").map_or("0", String::as_str);
            let cleaned = clean_init(init);
            let cpp_type = sv
                .get("type")
                .filter(|t| *t != "auto")
                .cloned()
                .unwrap_or_else(|| infer_cpp_type(init));
            let ns = shared_ns(sv);
            let backing = shared_static(accessor);
            let decl = if native_mode {
                format!("morph::Signal<{cpp_type}> {backing}({cleaned});")
            } else {
                format!("static morph::Signal<{cpp_type}> {backing}({cleaned});")
            };
            shared_lines.push((ns, decl));
        }
    }
    if !shared_lines.is_empty() {
        lines.push(String::new());
        lines.push("// ── morphShared signals (keyed) ──".to_string());
        emit_shared_entries(lines, shared_lines);
    }
    // Builder premain references shared stores through `{accessor}()`
    // (the same convention as the build TU), so the dev TU defines those
    // accessors over the file-scope backing signals above.
    let mut seen_accessors = HashSet::new();
    let mut accessor_lines: Vec<(String, String)> = Vec::new();
    for w in windows {
        for sv in &w.shared_vars {
            let (key, accessor) = (
                sv.get("key").map_or("", String::as_str),
                sv.get("accessor").map_or("", String::as_str),
            );
            if key.is_empty() || accessor.is_empty() || !seen_accessors.insert(key.to_string()) {
                continue;
            }
            let init = sv.get("init").map_or("0", String::as_str);
            let cpp_type = sv
                .get("type")
                .filter(|t| *t != "auto")
                .cloned()
                .unwrap_or_else(|| infer_cpp_type(init));
            let ns = shared_ns(sv);
            let backing = shared_static(accessor);
            accessor_lines.push((
                ns,
                format!("static morph::Signal<{cpp_type}>& {accessor}() {{ return {backing}; }}"),
            ));
        }
    }
    if !accessor_lines.is_empty() {
        lines.push(String::new());
        lines.push("// ── morphShared accessors (match the build TU) ──".to_string());
        emit_shared_entries(lines, accessor_lines);
    }
}

/// Keyed list item factories (file scope, shared with the wiring).
fn emit_factories(lines: &mut Vec<String>, list_nodes: &[&IRNode], maps: &AmbientMaps) {
    let mut factory_lines = Vec::new();
    for list_node in list_nodes {
        if let Some(factory) = emit_list_factory(list_node, maps) {
            factory_lines.push(factory);
        }
    }
    if !factory_lines.is_empty() {
        lines.push(String::new());
        lines.push("// ── Keyed list item factories ──".to_string());
        for factory in factory_lines {
            lines.push(factory);
            lines.push(String::new());
        }
    }
}

/// `morph_logic_rewire`: wire events + (re)create effects + subscriptions.
fn emit_rewire(
    all_nodes: &[&IRNode],
    effect_decls: &[&HashMap<String, String>],
    guarded: &HashMap<usize, Vec<String>>,
    maps: &AmbientMaps,
    channel_subs: &[&HashMap<String, String>],
) -> Vec<String> {
    let mut rewire =
        vec!["void morph_logic_rewire(::NodeRegistry& nodes, ::SignalStore& store) {".to_string()];
    rewire.push("    (void)store;".to_string());
    // Subscriptions re-run on every rewire: clear first so handlers never
    // stack up across hot reloads.
    if !channel_subs.is_empty() {
        rewire.push("    morph::clear_channels();".to_string());
    }
    rewire.push(String::new());
    let mut wired: HashSet<(String, String)> = HashSet::new();
    for node in all_nodes {
        let mut event_lines = Vec::new();
        emit_node_events(&mut event_lines, node, maps, &mut wired, "    ");
        rewire.extend(event_lines);
    }
    rewire.push(String::new());
    for node in all_nodes {
        let mut effect_lines = Vec::new();
        emit_node_effects(&mut effect_lines, node, maps, "    ");
        rewire.extend(effect_lines);
    }
    rewire.push(String::new());
    for sub in channel_subs {
        let channel = sub.get("channel").map_or("", String::as_str);
        let body = sub.get("body").map_or("", String::as_str);
        if channel.is_empty() || body.is_empty() {
            continue;
        }
        let escaped = channel.replace('\\', "\\\\").replace('"', "\\\"");
        rewire.push(format!("    morph::channel(\"{escaped}\").on({body});"));
    }
    if !channel_subs.is_empty() {
        rewire.push(String::new());
    }
    let mut guard_idx = 0;
    for (idx, decl) in effect_decls.iter().enumerate() {
        let deps = decl.get("deps").map_or("", String::as_str);
        if deps == "[]" {
            continue;
        }
        let lambda = decl.get("lambda").map_or("", String::as_str);
        if let Some(exprs) = guarded.get(&idx) {
            let sig_expr = exprs
                .iter()
                .map(|e| format!("morph::str({e})"))
                .collect::<Vec<_>>()
                .join(" + \"|\" + ");
            rewire.push("    {".to_string());
            rewire.push(format!("        auto __ef = {lambda};"));
            rewire.push(
                "        __effects[__effect_count++] = morph::create_effect([&, __ef]() {"
                    .to_string(),
            );
            rewire.push(format!("            std::string __sig = {sig_expr};"));
            rewire.push(format!("            if (__sig == __esig_{guard_idx}) return;"));
            rewire.push(format!("            __esig_{guard_idx} = __sig;"));
            rewire.push("            __ef();".to_string());
            rewire.push("        });".to_string());
            rewire.push("    }".to_string());
            guard_idx += 1;
        } else {
            rewire
                .push(format!("    __effects[__effect_count++] = morph::create_effect({lambda});"));
        }
    }
    rewire.push("}".to_string());
    rewire
}

/// `morph_logic_init`: sync signals from the store, run once-effects, rewire.
fn emit_init(
    lines: &mut Vec<String>,
    windows: &[IRWindow],
    effect_decls: &[&HashMap<String, String>],
) {
    lines.push("void morph_logic_init(::NodeRegistry& nodes, ::SignalStore& store) {".to_string());
    let mut seen_init = HashSet::new();
    for w in windows {
        for sv in &w.state_vars {
            let getter = sv.get("getter").map_or("", String::as_str);
            if getter.is_empty() || !seen_init.insert(getter.to_string()) {
                continue;
            }
            let init = sv.get("init").map_or("0", String::as_str);
            let cleaned = clean_init(init);
            let cpp_type = infer_cpp_type(init);
            lines.push(format!(
                "    __st_{getter}.set(store.get_or_create<{cpp_type}>(\"{getter}\", {cleaned}).get());"
            ));
        }
    }
    let mut seen_shared = HashSet::new();
    for w in windows {
        for sv in &w.shared_vars {
            let key = sv.get("key").map_or("", String::as_str);
            let accessor = sv.get("accessor").map_or("", String::as_str);
            if key.is_empty() || accessor.is_empty() || !seen_shared.insert(key.to_string()) {
                continue;
            }
            let init = sv.get("init").map_or("0", String::as_str);
            let cleaned = clean_init(init);
            let cpp_type = sv
                .get("type")
                .filter(|t| *t != "auto")
                .cloned()
                .unwrap_or_else(|| infer_cpp_type(init));
            let ns = shared_ns(sv);
            let backing = shared_backing_ref(&ns, accessor);
            let escaped = key.replace('\\', "\\\\").replace('"', "\\\"");
            lines.push(format!(
                "    {backing}.set(store.get_or_create<{cpp_type}>(\"{escaped}\", {cleaned}).get());"
            ));
        }
    }
    if !seen_init.is_empty() {
        lines.push(String::new());
    }
    for decl in effect_decls {
        if decl.get("deps").map_or("", String::as_str) != "[]" {
            continue;
        }
        let lambda = decl.get("lambda").map_or("", String::as_str);
        lines.push("    { // morphEffect (run once)".to_string());
        lines.push(format!("        auto __ef_fn = {lambda};"));
        lines.push("        __ef_fn();".to_string());
        lines.push("    }".to_string());
    }
    lines.push("    morph_logic_rewire(nodes, store);".to_string());
    lines.push("}".to_string());
}

/// Generate the dev logic translation unit (plus native-mode state header).
/// Widget + extra includes for the logic TU. Returns whether any window
/// uses native mode (user `.cpp` imports needing the state header).
fn emit_includes(lines: &mut Vec<String>, windows: &[IRWindow], list_nodes: &[&IRNode]) -> bool {
    if !list_nodes.is_empty() {
        lines.push("#include \"ui/morph_list.h\"".to_string());
        lines.push("#include \"ui/rect.h\"".to_string());
        lines.push("#include \"ui/text.h\"".to_string());
        lines.push("#include \"ui/button.h\"".to_string());
    }
    if windows.iter().flat_map(|w| w.nodes.iter()).any(|n| has_input(std::slice::from_ref(n))) {
        lines.push("#include \"ui/input.h\"".to_string());
    }
    let mut extra_headers: Vec<String> = Vec::new();
    for w in windows {
        for header in &w.extra_headers {
            if BUILTIN_HEADERS.contains(&header.as_str()) {
                continue;
            }
            let mut cleaned = header.clone();
            for prefix in
                ["\"../../runtime/cpp/", "\"../../runtime/cpp/types/", "\"../../runtime/cpp/dev/"]
            {
                if let Some(rest) = cleaned.strip_prefix(prefix) {
                    cleaned = format!("\"{rest}");
                    break;
                }
            }
            if !extra_headers.contains(&cleaned) {
                extra_headers.push(cleaned);
            }
        }
    }
    extra_headers.sort();
    for header in &extra_headers {
        lines.push(format!("#include {header}"));
    }
    windows.iter().any(|w| !w.cpp_imports.is_empty())
}

/// Dedup premain functions across windows, preserving order.
fn collect_premain(windows: &[IRWindow]) -> Vec<String> {
    let mut premain_parts = Vec::new();
    let mut seen = HashSet::new();
    for w in windows {
        for func in &w.premain_functions {
            if seen.insert(func.clone()) {
                premain_parts.push(func.clone());
            }
        }
    }
    premain_parts
}

/// Native-mode interop block: state header + user `.cpp` includes.
/// Returns the state header content.
fn emit_native_block(
    lines: &mut Vec<String>,
    windows: &[IRWindow],
    premain_parts: &[String],
) -> String {
    let mut import_paths: Vec<String> = Vec::new();
    let mut seen = HashSet::new();
    for w in windows {
        for import in &w.cpp_imports {
            if let Some(path) = import.get("path") {
                if !path.is_empty() && seen.insert(path.clone()) {
                    import_paths.push(path.clone());
                }
            }
        }
    }
    let state_header = generate_state_header(windows, premain_parts, true);
    lines.push(String::new());
    lines.push("// ── Generated interop declarations (morphState + JSX functions) ──".to_string());
    lines.push(format!("#include \"{STATE_HEADER_NAME}\""));
    for path in &import_paths {
        lines.push(format!("// User C++ import: {path}"));
        lines.push(format!("#include \"{path}\""));
    }
    state_header
}

pub fn emit_logic(windows: &[IRWindow]) -> LogicOutput {
    let mut lines = vec![LOGIC_PREHEADER.to_string()];
    let maps = ambient_maps(windows);
    // Bare getter names for effect-dep guarding (`count`, not `__st_count`).
    let state_getters: HashSet<String> = windows
        .iter()
        .flat_map(|w| w.state_vars.iter())
        .filter_map(|sv| sv.get("getter"))
        .filter(|g| !g.is_empty())
        .cloned()
        .collect();
    let mut shared_guards: HashMap<String, String> = HashMap::new();
    for w in windows {
        for sv in &w.shared_vars {
            let (getter, accessor) = (
                sv.get("getter").map_or("", String::as_str),
                sv.get("accessor").map_or("", String::as_str),
            );
            if getter.is_empty() || accessor.is_empty() {
                continue;
            }
            let ns = shared_ns(sv);
            shared_guards
                .entry(getter.to_string())
                .or_insert_with(|| format!("{}.get()", shared_ref(&ns, accessor)));
        }
    }

    let all_nodes: Vec<&IRNode> = windows.iter().flat_map(|w| w.nodes.iter()).collect();
    let mut list_nodes = Vec::new();
    for node in &all_nodes {
        list_nodes.extend(collect_list_nodes(std::slice::from_ref(node)));
    }
    // Collect channel_subs for the rewire.
    let mut channel_subs: Vec<&HashMap<String, String>> = Vec::new();
    for w in windows {
        channel_subs.extend(w.channel_subs.iter());
    }
    let native_mode = emit_includes(&mut lines, windows, &list_nodes);
    let premain_parts = collect_premain(windows);
    let state_header = if native_mode {
        Some(emit_native_block(&mut lines, windows, &premain_parts))
    } else {
        None
    };

    // ── File-scope signal statics (persist across morph_logic_init calls) ──
    emit_signal_statics(&mut lines, windows, native_mode);

    // ── File-scope premain functions ──
    for func in &premain_parts {
        lines.push(String::new());
        if native_mode {
            lines.push(strip_static_function(func));
        } else {
            lines.push(func.clone());
        }
    }

    // ── Keyed list item factories ──
    emit_factories(&mut lines, &list_nodes, &maps);

    // ── Effect declarations (guarded when deps map to state vars) ──
    let mut effect_decls: Vec<&HashMap<String, String>> = Vec::new();
    for w in windows {
        effect_decls.extend(w.effect_decls.iter());
    }
    let mut guarded: HashMap<usize, Vec<String>> = HashMap::new();
    for (idx, decl) in effect_decls.iter().enumerate() {
        let deps = decl.get("deps").map_or("", String::as_str);
        if let Some(exprs) = effect_dep_exprs(deps, &state_getters, &shared_guards) {
            guarded.insert(idx, exprs);
        }
    }

    lines.push("extern \"C\" {".to_string());
    lines.push(String::new());
    lines.push("static int __effect_count = 0;".to_string());
    lines.push(String::new());
    for i in 0..guarded.len() {
        lines.push(format!("static std::string __esig_{i};"));
    }
    if !guarded.is_empty() {
        lines.push(String::new());
    }

    // ── morph_logic_rewire ──
    let rewire = emit_rewire(&all_nodes, &effect_decls, &guarded, &maps, &channel_subs);
    let effect_count = rewire.iter().filter(|l| l.contains("__effects[__effect_count++]")).count();
    lines.extend(rewire);
    lines.push(String::new());

    // ── morph_logic_init ──
    emit_init(&mut lines, windows, &effect_decls);
    lines.push(LOGIC_FOOTER.to_string());

    // Size the effects array by emission-site count.
    let array_line = format!("static morph::EffectNode* __effects[{}];", effect_count.max(1));
    if let Some(pos) = lines.iter().position(|l| l.trim() == "extern \"C\" {") {
        lines.insert(pos + 1, array_line);
    }

    LogicOutput { source: lines.join("\n"), state_header }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logic_output_has_dev_entrypoints() {
        let output = emit_logic(&[]);
        assert!(output
            .source
            .contains("void morph_logic_rewire(::NodeRegistry& nodes, ::SignalStore& store)"));
        assert!(output
            .source
            .contains("void morph_logic_init(::NodeRegistry& nodes, ::SignalStore& store)"));
        assert!(output.source.contains("void morph_logic_cleanup()"));
        assert!(output.source.contains("static morph::EffectNode* __effects[1];"));
        assert!(output.state_header.is_none());
    }

    #[test]
    fn signals_use_cleaned_inits() {
        let mut sv = HashMap::new();
        sv.insert("getter".to_string(), "items".to_string());
        sv.insert("setter".to_string(), "setItems".to_string());
        sv.insert("init".to_string(), "['a', 'b']".to_string());
        let window = IRWindow { state_vars: vec![sv], ..Default::default() };
        let output = emit_logic(&[window]);
        assert!(output
            .source
            .contains("static morph::Signal<JsArray> __st_items(JsArray{\"a\", \"b\"});"));
        assert!(output.source.contains("store.get_or_create<JsArray>(\"items\""));
    }

    #[test]
    fn events_wire_through_registry() {
        use morph_ir::{IREvent, IRNode};
        let node = IRNode {
            node_id: "node_0001".to_string(),
            node_type: "button".to_string(),
            events: vec![IREvent {
                trigger: "click".to_string(),
                action: "call".to_string(),
                target: "() => pressClear()".to_string(),
            }],
            ..Default::default()
        };
        let window = IRWindow { nodes: vec![node], ..Default::default() };
        let output = emit_logic(&[window]);
        assert!(output.source.contains("nodes.get(\"node_0001\")"));
        assert!(output.source.contains("n->onClick = [](JsObject e) { pressClear(); };"));
    }

    #[test]
    fn reactive_text_uses_morpher_state_mapping() {
        use morph_ir::IRNode;
        let mut sv = HashMap::new();
        sv.insert("getter".to_string(), "acc".to_string());
        sv.insert("setter".to_string(), "setAcc".to_string());
        sv.insert("init".to_string(), "'0'".to_string());
        let node = IRNode {
            node_id: "node_0009".to_string(),
            node_type: "__expr__".to_string(),
            reactive_text: "acc".to_string(),
            ..Default::default()
        };
        let window = IRWindow { nodes: vec![node], state_vars: vec![sv], ..Default::default() };
        let output = emit_logic(&[window]);
        assert!(output.source.contains("n->setText(morph::str(__st_acc.get()));"));
    }

    fn shared_window() -> IRWindow {
        let mut sv = HashMap::new();
        sv.insert("key".to_string(), "cart.count".to_string());
        sv.insert("accessor".to_string(), "shared_cart_count".to_string());
        sv.insert("type".to_string(), "int".to_string());
        sv.insert("init".to_string(), "0".to_string());
        sv.insert("getter".to_string(), "count".to_string());
        sv.insert("setter".to_string(), "setCount".to_string());
        let node = IRNode {
            node_id: "node_0009".to_string(),
            node_type: "__expr__".to_string(),
            reactive_text: "count".to_string(),
            ..Default::default()
        };
        IRWindow { nodes: vec![node], shared_vars: vec![sv], ..Default::default() }
    }

    fn namespaced_shared_window() -> IRWindow {
        let mut window = shared_window();
        window.shared_vars[0].insert("ns".to_string(), "store_deadbeef".to_string());
        window
    }

    #[test]
    fn shared_syncs_from_store_by_key() {
        let output = emit_logic(&[shared_window()]);
        assert!(
            output.source.contains("static morph::Signal<int> __shared_cart_count(0);"),
            "backing static"
        );
        assert!(
            output.source.contains("store.get_or_create<int>(\"cart.count\", 0)"),
            "keyed sync: {}",
            output.source
        );
        assert!(
            output.source.contains("n->setText(morph::str(shared_cart_count().get()));"),
            "read mapped: {}",
            output.source
        );
    }

    #[test]
    fn shared_namespace_qualifies_statics_accessors_and_reads() {
        let output = emit_logic(&[namespaced_shared_window()]);
        assert!(
            output.source.contains("namespace morph_mods {\nnamespace store_deadbeef {"),
            "module namespace: {}",
            output.source
        );
        assert!(
            output.source.contains(
                "morph_mods::store_deadbeef::__shared_cart_count.set(store.get_or_create<int>"
            ),
            "qualified store sync: {}",
            output.source
        );
        assert!(
            output.source.contains(
                "n->setText(morph::str(morph_mods::store_deadbeef::shared_cart_count().get()));"
            ),
            "qualified read: {}",
            output.source
        );
    }

    #[test]
    fn state_header_skips_shared_wrappers_owned_by_morph_api() {
        // morphShared wrappers live in the generated morph_api.h (always
        // included before this header); emitting them here too is a
        // redefinition error (caught by a real fixture build).
        let windows = vec![shared_window()];
        let build_h = generate_state_header(&windows, &[], false);
        assert!(!build_h.contains("shared_cart_count().get()"), "{build_h}");
        assert!(!build_h.contains("shared_cart_count().set"), "{build_h}");
        let dev_h = generate_state_header(&windows, &[], true);
        assert!(!dev_h.contains("__shared_cart_count.get()"), "{dev_h}");
    }

    #[test]
    fn state_header_skips_namespaced_shared_wrappers() {
        let windows = vec![namespaced_shared_window()];
        let build_h = generate_state_header(&windows, &[], false);
        assert!(
            !build_h.contains("morph_mods::store_deadbeef::shared_cart_count().get()"),
            "{build_h}"
        );
        let dev_h = generate_state_header(&windows, &[], true);
        assert!(
            !dev_h.contains("morph_mods::store_deadbeef::__shared_cart_count.get()"),
            "{dev_h}"
        );
    }

    #[test]
    fn instance_signals_skipped_in_header() {
        let mut sv = HashMap::new();
        sv.insert("getter".to_string(), "inst0_count".to_string());
        sv.insert("setter".to_string(), "inst0_setCount".to_string());
        sv.insert("init".to_string(), "0".to_string());
        sv.insert("instance".to_string(), "1".to_string());
        let window = IRWindow { state_vars: vec![sv], ..Default::default() };
        let h = generate_state_header(&[window], &[], false);
        assert!(!h.contains("inst0_count"), "instance hidden: {h}");
    }

    #[test]
    fn function_decl_extraction_skips_lambdas_and_main() {
        assert_eq!(
            extract_function_decl("static inline\nvoid increment() { return; }"),
            Some("void increment();".to_string())
        );
        assert_eq!(extract_function_decl("auto x = []() { return 1; };"), None);
        assert_eq!(extract_function_decl("int main() { return 0; }"), None);
        assert_eq!(extract_function_decl(""), None);
    }

    #[test]
    fn init_parsing_shapes() {
        assert_eq!(clean_init("'hi'"), "\"hi\"");
        assert_eq!(clean_init("[1, 'a', true]"), "JsArray{1, \"a\", true}");
        assert_eq!(infer_cpp_type("[1]"), "JsArray");
        assert_eq!(infer_cpp_type("3.5"), "double");
        assert_eq!(infer_cpp_type("-3"), "int");
    }
}
