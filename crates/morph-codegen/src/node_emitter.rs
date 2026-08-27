use morph_ir::{IRNode, IRStyle};

pub fn fmt(v: f32) -> String {
    if !v.is_finite() { return "0.0f".into(); }
    let s = format!("{:.1}", v);
    if s.contains('.') {
        // Ensure at least one decimal and 'f' suffix
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.contains('.') { format!("{}f", s) } else { format!("{}.0f", trimmed) }
    } else {
        format!("{}.0f", s)
    }
}

pub fn emit_node(node: &IRNode, parent_id: Option<&str>, features: &std::collections::HashSet<String>) -> String {
    let mut lines = Vec::new();
    let indent = "    ";

    if node.node_type == "__list__" {
        return emit_list(node, parent_id, indent);
    }
    if node.node_type == "__text__" {
        let escaped = node.text_content.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        if node.reactive_text.is_empty() {
            lines.push(format!("TextNode* {} = new TextNode(\"{}\");", node.node_id, escaped));
        } else {
            lines.push(format!("TextNode* {} = new TextNode(\"\");", node.node_id));
        }
        lines.push(format!("{}{}->x = {};", indent, node.node_id, fmt(node.x)));
        lines.push(format!("{}{}->y = {};", indent, node.node_id, fmt(node.y)));
        lines.push(format!("{}{}->w = {};", indent, node.node_id, fmt(node.w)));
        lines.push(format!("{}{}->h = {};", indent, node.node_id, fmt(node.h)));
        lines.push(set_style(node, indent, None, features));
        if let Some(pid) = parent_id {
            lines.push(format!("{pid}->addChild({});", node.node_id));
        }
        if !node.reactive_text.is_empty() {
            lines.push(format!("{}->m_associatedEffects.push_back(morph::create_effect([{}]() {{", node.node_id, node.node_id));
            lines.push(format!("{}{}->setText(morph::str({}));", indent, node.node_id, node.reactive_text));
            lines.push("}));".into());
        }
        return lines.join("\n");
    }
    if node.node_type == "__conditional__" {
        return emit_conditional(node, parent_id, indent, features);
    }

    // Regular element
    match node.node_type.as_str() {
        "button" => {
            lines.push(format!("ButtonNode* {} = new ButtonNode();", node.node_id));
            lines.push(format!("{}{}->type = \"button\";", indent, node.node_id));
        }
        "input" => {
            lines.push(format!("InputNode* {} = new InputNode();", node.node_id));
            if let Some(v) = node.attrs.get("value") {
                let esc = v.replace('\\', "\\\\").replace('"', "\\\"");
                lines.push(format!("{}{}->setValue(\"{}\");", indent, node.node_id, esc));
            }
            if let Some(ph) = node.attrs.get("placeholder") {
                let esc = ph.replace('\\', "\\\\").replace('"', "\\\"");
                lines.push(format!("{}{}->placeholder = \"{}\";", indent, node.node_id, esc));
            }
            if let Some(t) = node.attrs.get("type") {
                lines.push(format!("{}{}->inputType = \"{}\";", indent, node.node_id, t.replace('"', "\\\"")));
            }
        }
        "img" => {
            let src = node.attrs.get("src").map(|s| s.as_str()).unwrap_or("");
            let alt = node.attrs.get("alt").map(|s| s.as_str()).unwrap_or("");
            lines.push(format!("ImageNode* {} = new ImageNode(\"{}\", \"{}\");", node.node_id, src.replace('"', "\\\""), alt.replace('"', "\\\"")));
        }
        _ => {
            lines.push(format!("RectNode* {} = new RectNode({}, {}, {}, {});", node.node_id, fmt(node.x), fmt(node.y), fmt(node.w), fmt(node.h)));
        }
    }
    if node.node_type != "button" && node.node_type != "input" && node.node_type != "img" {
        lines.push(format!("{}{}->x = {};", indent, node.node_id, fmt(node.x)));
        lines.push(format!("{}{}->y = {};", indent, node.node_id, fmt(node.y)));
        lines.push(format!("{}{}->w = {};", indent, node.node_id, fmt(node.w)));
        lines.push(format!("{}{}->h = {};", indent, node.node_id, fmt(node.h)));
    }
    lines.push(set_style(node, indent, None, features));
    for ev in &node.events {
        lines.push(format!("{}{}->on_{} = []() {{ {} }};", indent, node.node_id, ev.trigger, ev.target));
    }
    if let Some(pid) = parent_id {
        lines.push(format!("{pid}->addChild({});", node.node_id));
    }
    for child in &node.children {
        lines.push(emit_node(child, Some(&node.node_id), features));
    }
    lines.push(emit_reactive_effects(node, indent));
    lines.join("\n")
}

fn emit_conditional(node: &IRNode, parent_id: Option<&str>, indent: &str, features: &std::collections::HashSet<String>) -> String {
    let mut lines = vec![format!("RectNode* {} = new RectNode(0.0f, 0.0f, 0.0f, 0.0f);", node.node_id)];
    let then_slot = format!("__cond_then_{}", node.node_id);
    let else_slot = format!("__cond_else_{}", node.node_id);
    lines.push(format!("auto {then_slot} = std::make_shared<MorphNode*>(nullptr);"));
    lines.push(format!("auto {else_slot} = std::make_shared<MorphNode*>(nullptr);"));
    if let Some(pid) = parent_id {
        lines.push(format!("{pid}->addChild({});", node.node_id));
    }
    let mut then_code = String::new();
    let mut then_var = String::new();
    for tn in &node.then_nodes {
        then_code = emit_node(tn, None, features);
        then_var = tn.node_id.clone();
        break;
    }
    let mut else_code = String::new();
    let mut else_var = String::new();
    for en in &node.else_nodes {
        else_code = emit_node(en, None, features);
        else_var = en.node_id.clone();
        break;
    }
    lines.push(format!("{}->m_associatedEffects.push_back(morph::create_effect([{}, {then_slot}, {else_slot}]() {{", node.node_id, node.node_id));
    let bi = format!("{indent}    ");
    lines.push(format!("{bi}if ({}) {{", node.condition_expr));
    lines.push(format!("{bi}    if (*{else_slot}) {{ {0}->removeChild(*{else_slot}); delete *{else_slot}; *{else_slot}=nullptr; }}", node.node_id));
    if !then_code.is_empty() {
        lines.push(format!("{bi}    if (!*{then_slot}) {{"));
        for l in then_code.lines() { lines.push(format!("{bi}        {l}")); }
        lines.push(format!("{bi}        {}->addChild({});", node.node_id, then_var));
        lines.push(format!("{bi}        *{then_slot} = {then_var};"));
        lines.push(format!("{bi}    }}"));
    }
    lines.push(format!("{bi}}} else {{"));
    lines.push(format!("{bi}    if (*{then_slot}) {{ {0}->removeChild(*{then_slot}); delete *{then_slot}; *{then_slot}=nullptr; }}", node.node_id));
    if !else_code.is_empty() {
        lines.push(format!("{bi}    if (!*{else_slot}) {{"));
        for l in else_code.lines() { lines.push(format!("{bi}        {l}")); }
        lines.push(format!("{bi}        {}->addChild({});", node.node_id, else_var));
        lines.push(format!("{bi}        *{else_slot} = {else_var};"));
        lines.push(format!("{bi}    }}"));
    }
    lines.push(format!("{bi}}}"));
    lines.push(format!("{bi}{}->markDirty(LayoutDirty);", node.node_id));
    lines.push("}));".into());
    lines.join("\n")
}

fn emit_list(node: &IRNode, parent_id: Option<&str>, indent: &str) -> String {
    let mut lines = vec![format!("morph::ListContainer* {} = new morph::ListContainer({}, {}, {}, {});", node.node_id, fmt(node.x), fmt(node.y), fmt(node.w), fmt(node.h))];
    if let Some(pid) = parent_id { lines.push(format!("{pid}->addChild({});", node.node_id)); }
    lines.push(format!("{}->arrayFn = []() {{ return {}; }};", node.node_id, node.list_expr));
    lines.push(format!("{}->itemFactory = __list_factory_{};", node.node_id, node.node_id));
    if !node.list_key_expr.is_empty() {
        lines.push(format!("{}->keyFn = [](const JsValue& __it, int __index) -> std::string {{ return morph::list_key({}, __index); }};", node.node_id, node.list_key_expr));
    }
    lines.push(format!("{}->m_associatedEffects.push_back(morph::create_effect([{}]() {{ {}->reconcile({}->arrayFn()); }}));", node.node_id, node.node_id, node.node_id, node.node_id));
    lines.join("\n")
}

fn set_style(node: &IRNode, indent: &str, _parent: Option<&IRStyle>, features: &std::collections::HashSet<String>) -> String {
    let s = &node.style;
    let prefix = format!("{}->style", node.node_id);
    let mut lines = Vec::new();
    if s.bg_color != [0.0,0.0,0.0,0.0] {
        lines.push(format!("{prefix}.bgColor[0] = {:.4}f;", s.bg_color[0]));
        lines.push(format!("{prefix}.bgColor[1] = {:.4}f;", s.bg_color[1]));
        lines.push(format!("{prefix}.bgColor[2] = {:.4}f;", s.bg_color[2]));
        lines.push(format!("{prefix}.bgColor[3] = {:.4}f;", s.bg_color[3]));
    }
    if s.border_radius > 0.0 { lines.push(format!("{prefix}.borderRadius = {};", fmt(s.border_radius))); }
    if s.font_size != 16.0 { lines.push(format!("{prefix}.fontSize = {};", fmt(s.font_size))); }
    if s.padding != [0.0,0.0,0.0,0.0] {
        lines.push(format!("{prefix}.padding[0] = {};", fmt(s.padding[0])));
        lines.push(format!("{prefix}.padding[1] = {};", fmt(s.padding[1])));
        lines.push(format!("{prefix}.padding[2] = {};", fmt(s.padding[2])));
        lines.push(format!("{prefix}.padding[3] = {};", fmt(s.padding[3])));
    }
    if s.display != "block" { lines.push(format!("{prefix}.display = \"{}\";", s.display)); }
    if features.contains("flex") && s.gap > 0.0 { lines.push(format!("{prefix}.gap = {};", fmt(s.gap))); }
    if s.opacity != 1.0 { lines.push(format!("{prefix}.opacity = {};", fmt(s.opacity))); }
    if lines.is_empty() { "".into() } else { lines.join(&format!("\n{indent}")) }
}

fn emit_reactive_effects(_node: &IRNode, _indent: &str) -> String { String::new() }
