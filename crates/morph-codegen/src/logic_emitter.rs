use morph_ir::{IRWindow, IRNode};

pub fn emit_logic(windows: &[IRWindow]) -> String {
    let mut lines = vec![String::from("// Auto-generated logic — dev hot reload")];
    lines.push(String::from("#include \"logic_prelude.h\""));
    // Collect state vars
    for w in windows {
        for sv in &w.state_vars {
            let name = sv.get("getter").cloned().unwrap_or_else(|| "unknown".into());
            let init = sv.get("init").cloned().unwrap_or_else(|| "0".into());
            let ty = infer_cpp_type(&init);
            lines.push(format!("static morph::Signal<{}> __st_{}({});", ty, name, init));
        }
    }
    lines.push(String::from("extern \"C\" {"));
    lines.push(String::from("void morph_logic_rewire() {"));
    for w in windows {
        for node in &w.nodes {
            emit_node_effects(&mut lines, node);
        }
    }
    lines.push(String::from("}"));
    lines.push(String::from("void morph_logic_init() {}"));
    lines.push(String::from("}"));
    lines.join("\n")
}

fn infer_cpp_type(init: &str) -> String {
    let s = init.trim();
    if s == "true" || s == "false" { return "bool".into(); }
    if s.starts_with('"') || s.starts_with('\'') { return "std::string".into(); }
    if s.contains('.') { return "double".into(); }
    if s.parse::<i64>().is_ok() { return "int".into(); }
    "auto".into()
}

fn emit_node_effects(lines: &mut Vec<String>, node: &IRNode) {
    if !node.reactive_text.is_empty() {
        lines.push(format!("// reactive text for {}", node.node_id));
    }
    for child in &node.children { emit_node_effects(lines, child); }
    for tn in &node.then_nodes { emit_node_effects(lines, tn); }
    for en in &node.else_nodes { emit_node_effects(lines, en); }
    if let Some(ref tmpl) = node.item_template { emit_node_effects(lines, tmpl); }
}

pub fn collect_list_nodes(nodes: &[IRNode]) -> Vec<&IRNode> {
    let mut out = Vec::new();
    for n in nodes {
        if n.node_type == "__list__" { out.push(n); }
        if let Some(ref tmpl) = n.item_template { out.extend(collect_list_nodes(std::slice::from_ref(tmpl))); }
        out.extend(collect_list_nodes(&n.children));
        out.extend(collect_list_nodes(&n.then_nodes));
        out.extend(collect_list_nodes(&n.else_nodes));
    }
    out
}
