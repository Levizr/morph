use morph_ir::{IRNode, IRStyle};

pub fn fmt(v: f32) -> String {
    if !v.is_finite() { return "0.0f".into(); }
    let s = format!("{:.1}", v);
    if s.contains('.') {
        let trimmed = s.trim_end_matches('0').trim_end_matches('.');
        if trimmed.contains('.') { format!("{}f", s) } else { format!("{}.0f", trimmed) }
    } else {
        format!("{}.0f", s)
    }
}

fn color4(c: &[f32; 4]) -> String {
    format!("{:.4}f, {:.4}f, {:.4}f, {:.4}f", c[0], c[1], c[2], c[3])
}

fn translate_js(js: &str, state_map: &std::collections::HashMap<String, String>) -> String {
    let mut s = js.trim().to_string();
    if let Some(idx) = s.find("=>") {
        let body = s[idx+2..].trim();
        let inner = if body.starts_with('{') && body.ends_with('}') {
            body[1..body.len()-1].trim().to_string()
        } else { body.to_string() };
        s = inner;
    }
    // Generic state var replacement
    for (from, to) in state_map {
        // Replace "setX(" with map value
        if s.contains(from) {
            let mut res = String::new();
            let mut i = 0;
            while let Some(pos) = s[i..].find(from) {
                let start = i + pos;
                let end = start + from.len();
                let before_ok = start == 0 || {
                    let b = s.as_bytes()[start - 1];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                let after_ok = end >= s.len() || {
                    let b = s.as_bytes()[end];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                let already = start >= 4 && &s[start-4..start] == "__st";
                if before_ok && after_ok && !already {
                    res.push_str(&s[i..start]);
                    res.push_str(to);
                    i = end;
                } else {
                    res.push_str(&s[i..end]);
                    i = end;
                }
            }
            res.push_str(&s[i..]);
            s = res;
        }
    }
    s = s.replace("e.value", "e[\"value\"]");
    s = s.replace(".value", "[\"value\"]");
    s = s.replace("===", "==");
    s = s.replace("!==", "!=");
    let s = translate_dynamic_expr(&s);
    let s = s.trim();
    if s.is_empty() { return "".into(); }
    if s.ends_with(';') || s.ends_with('}') { s.to_string() } else { format!("{};", s) }
}

// ── Dynamic JS expression → C++ ──────────────────────────────────────
// Many event bodies / reactive texts carry literal JS values that have no
// counterpart in C++: object literals `{k: v}`, array literals `[a, b]`,
// array SPREAD `[...arr, x]` and `.slice()`. These are translated with the
// same runtime semantics as Python's TSToCppTranslator `_object_literal` /
// `_array_literal` / `.length` handling, but operating on the (already
// state-substituted) source string rather than an AST.

// Skip a `"..."` / `'...'` string literal starting at `start`; returns idx past
// the closing quote.
fn js_skip_string(s: &str, start: usize) -> usize {
    let b = s.as_bytes();
    let q = b[start] as char;
    let mut i = start + 1;
    while i < b.len() {
        if b[i] == b'\\' { i += 2; continue; }
        if b[i] as char == q { return i + 1; }
        i += 1;
    }
    b.len()
}

// Index of the delimiter that closes the `[ { (` opened at `open`.
fn js_matching_close(s: &str, open: usize) -> usize {
    let b = s.as_bytes();
    let (oc, cc) = match b[open] as char {
        '[' => ('[', ']'), '{' => ('{', '}'), '(' => ('(', ')'), _ => return open,
    };
    let mut depth = 0usize;
    let mut i = open;
    while i < b.len() {
        let c = b[i] as char;
        if c == '"' || c == '\'' { i = js_skip_string(s, i); continue; }
        if c == oc { depth += 1; }
        else if c == cc { depth -= 1; if depth == 0 { return i; } }
        i += 1;
    }
    b.len() - 1
}

// Split `s` on `sep` ignoring separators inside `()`, `[]`, `{}` and strings.
fn js_split_top(s: &str, sep: char) -> Vec<String> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i] as char;
        if c == '"' || c == '\'' { i = js_skip_string(s, i); continue; }
        if c == '(' || c == '[' || c == '{' { depth += 1; }
        else if c == ')' || c == ']' || c == '}' { depth -= 1; }
        else if c == sep && depth == 0 { out.push(s[start..i].trim().to_string()); start = i + 1; }
        i += 1;
    }
    out.push(s[start..].trim().to_string());
    out
}

// Translate an object literal body `k: v, k2: v2` into C++ JsObject members.
fn js_object_members(inner: &str, indent: &str) -> String {
    let pairs = js_split_top(inner, ',');
    let mut members = Vec::new();
    for p in pairs {
        let p = p.trim();
        if p.is_empty() { continue; }
        let (k, v) = match p.find(':') {
            Some(i) => (p[..i].trim().to_string(), p[i + 1..].trim().to_string()),
            None => (p.to_string(), "JsUndefined{}".into()),
        };
        members.push(format!("{indent}{{\"{}\", {}}}", k, translate_js_value(&v)));
    }
    members.join(",\n")
}

// Translate an array-literal body with optional spread `...x` elements.
fn js_array_elements(inner: &str, indent: &str) -> String {
    let inner = inner.trim();
    if inner.is_empty() { return "JsArray{}".to_string(); }
    let elems = js_split_top(inner, ',');
    let has_spread = elems.iter().any(|e| e.trim_start().starts_with("..."));
    if !has_spread {
        let body: Vec<String> =
            elems.iter().filter(|e| !e.trim().is_empty()).map(|e| translate_js_value(e)).collect();
        return format!("JsArray{{{}}}", body.join(", "));
    }
    let mut lines = vec![format!("{indent}[&]() {{"), format!("{indent}    JsArray __a{{}};")];
    let mut idx = 0usize;
    for e in elems {
        let e = e.trim();
        if e.is_empty() { continue; }
        if let Some(arg) = e.strip_prefix("...") {
            let arg = translate_js_value(arg.trim());
            lines.push(format!("{indent}    JsArray __sp_{} = {};", idx, arg));
            lines.push(format!(
                "{indent}    for (int64_t __i_{} = 0; __i_{} < (int64_t)__sp_{}.length(); ++__i_{})",
                idx, idx, idx, idx));
            lines.push(format!("{indent}        __a.push(__sp_{}[__i_{}]);", idx, idx));
        } else {
            lines.push(format!("{indent}    __a.push({});", translate_js_value(e)));
        }
        idx += 1;
    }
    lines.push(format!("{indent}    return __a;"));
    lines.push(format!("{indent}}}()"));
    lines.join("\n")
}

// Does `s` (whole expr) form a balanced `[ ... ]` / `{ ... }` literal?
fn js_is_whole_literal(s: &str, oc: char, cc: char) -> bool {
    if s.is_empty() || !s.starts_with(oc) { return false; }
    let close = js_matching_close(s, 0);
    close == s.len() - 1 && s.as_bytes()[close] as char == cc
}

// Wrap a leaf (non-literal) expression: `.length` → `(int)(X.length())` and
// wrapping string literals that participate in `+` concat into JsString(...).
fn js_transform_leaf(s: &str) -> String {
    let mut out = String::new();
    let mut i = 0usize;
    let mut seg = 0usize; // start of the not-yet-copied run of `s`
    while i < s.len() {
        if s[i..].starts_with(".length") {
            let left_start = js_left_operand_start(&s[..i]);
            out.push_str(&s[seg..left_start]);
            out.push_str(&format!("(int)({})", js_transform_leaf(&s[left_start..i])));
            out.push_str(".length()");
            i += ".length".len();
            seg = i;
            continue;
        }
        let c = s.as_bytes()[i] as char;
        if c == '"' || c == '\'' {
            let end = js_skip_string(s, i);
            let lit = &s[i..end];
            // wrap as JsString(...) when followed (outside any literal) by '+'
            let rest = &s[end..];
            let mut plus = false;
            let mut j = 0usize;
            while j < rest.len() {
                let rc = rest.as_bytes()[j] as char;
                if rc == '+' { plus = true; break; }
                if rc == '"' || rc == '\'' { break; }
                j += 1;
            }
            out.push_str(&s[seg..i]);
            if plus {
                out.push_str(&format!("JsString({})", lit));
            } else {
                out.push_str(lit);
            }
            i = end;
            seg = i;
            continue;
        }
        i += 1;
    }
    out.push_str(&s[seg..]);
    out
}

// Start index of the sub-expression that `X` of `X.length` (`.` is at
// `dot_idx`). Backtracks over calls/member accesses. Assumes `dot_idx` points
// at the `.` in `.length`.
fn js_left_operand_start(s: &str) -> usize {
    let b = s.as_bytes();
    let i = s.len(); // one past end; we only call with s = text before '.'
    // i points past the object expression; back up one
    let mut pos = i;
    while pos > 0 {
        let c = b[pos - 1] as char;
        if c == ')' {
            // skip balanced (...) plus optional call name
            let mut depth = 1;
            let mut j = pos - 1;
            while j > 0 && depth > 0 {
                j -= 1;
                match b[j] as char { '(' => depth -= 1, ')' => depth += 1, _ => {} }
            }
            pos = j;
            continue;
        }
        if c == ']' || c == '}' {
            let (oc, cc) = if c == ']' { ('[', ']') } else { ('{', '}') };
            let mut depth = 1;
            let mut j = pos - 1;
            while j > 0 && depth > 0 {
                j -= 1;
                match b[j] as char {
                    x if x == cc => depth += 1,
                    x if x == oc => depth -= 1,
                    _ => {}
                }
            }
            pos = j;
            continue;
        }
        if c.is_ascii_alphanumeric() || c == '_' || c == '.' {
            pos -= 1;
            continue;
        }
        break;
    }
    pos
}

// Entry: translate any JS object/array literals and .length/string-concat
// that occur inside an arbitrary generated expression (`s` may be a bare
// literal, an operator expression, or a call with such literals in args).
fn translate_dynamic_expr(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() { return s.to_string(); }
    if js_is_whole_literal(s, '{', '}') || js_is_whole_literal(s, '[', ']') {
        return translate_js_value(s);
    }
    // General expression: walk and replace nested object/array literals.
    let b = s.as_bytes();
    let mut out = String::new();
    let mut leaf_start = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i] as char;
        if c == '"' || c == '\'' { i = js_skip_string(s, i); continue; }
        if c == '{' || c == '[' {
            // Is this a literal start (preceded by = : ( , [ { or start)?
            let prev_ok = i == 0 || {
                let p = b[i - 1] as char;
                !(p.is_ascii_alphanumeric() || p == '_' || p == ')' || p == ']' || p == '}')
            };
            // Also treat `[` after something like `set(...` — subscript `x[i]`
            // has prev identifier. Calls receive `[` after `(` -> prev_ok true.
            if prev_ok {
                let close = js_matching_close(s, i);
                out.push_str(&js_transform_leaf(&s[leaf_start..i]));
                let whole = &s[i..=close];
                out.push_str(&translate_js_value(whole));
                i = close + 1;
                leaf_start = i;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&js_transform_leaf(&s[leaf_start..]));
    out
}

// Full literal (object OR array) -> C++. `s` is the inner content.
fn translate_js_value(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() { return s.to_string(); }
    if js_is_whole_literal(s, '{', '}') {
        return format!("JsObject{{\n{}\n}}", js_object_members(&s[1..s.len() - 1], "    "));
    }
    if js_is_whole_literal(s, '[', ']') {
        return js_array_elements(&s[1..s.len() - 1], "");
    }
    js_transform_leaf(s)
}

fn translate_condition(cond: &str, state_map: &std::collections::HashMap<String, String>) -> String {
    let mut s = cond.trim().to_string();
    // Use state_map for getters only (those ending with .get())
    for (from, to) in state_map {
        if to.ends_with(".get()") && s.contains(from) {
            // Only replace whole words
            let mut res = String::new();
            let mut i = 0;
            while let Some(pos) = s[i..].find(from) {
                let start = i + pos;
                let end = start + from.len();
                let before_ok = start == 0 || {
                    let b = s.as_bytes()[start - 1];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                let after_ok = end >= s.len() || {
                    let b = s.as_bytes()[end];
                    !b.is_ascii_alphanumeric() && b != b'_'
                };
                if before_ok && after_ok {
                    res.push_str(&s[i..start]);
                    res.push_str(to);
                    i = end;
                } else {
                    res.push_str(&s[i..end]);
                    i = end;
                }
            }
            res.push_str(&s[i..]);
            s = res;
        }
    }
    s = s.replace("===", "==");
    s = s.replace("!==", "!=");
    s
}

pub fn emit_node(node: &IRNode, parent_id: Option<&str>, features: &std::collections::HashSet<String>) -> String {
    emit_node_with_state(node, parent_id, features, &std::collections::HashMap::new(), None)
}

pub fn emit_node_with_state(node: &IRNode, parent_id: Option<&str>, features: &std::collections::HashSet<String>, state_map: &std::collections::HashMap<String, String>, parent_style: Option<&IRStyle>) -> String {
    let mut lines = Vec::new();
    let indent = "    ";

    if node.node_type == "__list__" {
        return emit_list(node, parent_id, indent, state_map);
    }
    if node.node_type == "__text__" || node.node_type == "__expr__" {
        let escaped = node.text_content.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\t', "\\t");
        if node.reactive_text.is_empty() {
            lines.push(format!("TextNode* {} = new TextNode(\"{}\");", node.node_id, escaped));
        } else {
            lines.push(format!("TextNode* {} = new TextNode(\"\");", node.node_id));
        }
        lines.push(format!("{}{}->x = {};", indent, node.node_id, fmt(node.x)));
        lines.push(format!("{}{}->y = {};", indent, node.node_id, fmt(node.y)));
        lines.push(format!("{}{}->w = {};", indent, node.node_id, fmt(node.w)));
        lines.push(format!("{}{}->h = {};", indent, node.node_id, fmt(node.h)));
        lines.push(set_style(node, indent, parent_style, features, true));
        if let Some(pid) = parent_id {
            lines.push(format!("{pid}->addChild({});", node.node_id));
        }
        if !node.reactive_text.is_empty() {
            let cpp_expr = translate_js(&node.reactive_text, state_map);
            let expr = cpp_expr.trim().trim_end_matches(';').to_string();
            lines.push(format!("{}->m_associatedEffects.push_back(morph::create_effect([{}]() {{", node.node_id, node.node_id));
            lines.push(format!("{}{}->setText(morph::str({}));", indent, node.node_id, expr));
            lines.push("}));".into());
        }
        return lines.join("\n");
    }
    if node.node_type == "__conditional__" {
        return emit_conditional(node, parent_id, indent, features, state_map, parent_style);
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
    lines.push(set_style(node, indent, parent_style, features, false));
    lines.push(emit_hover_style(node, indent, features, false));
    lines.push(emit_active_style(node, indent, features));
    lines.push(emit_transition(node, indent));
    lines.push(emit_animations(node, indent, features));
    lines.push(emit_hover_animations(node, indent, features));
    for ev in &node.events {
        let member = match ev.trigger.as_str() {
            "click" => "onClick",
            "input" => "onInput",
            "change" => "onChange",
            "focus" => "onFocus",
            "blur" => "onBlur",
            "keyup" => "onKeyUp",
            "keydown" => "onKeyDown",
            "mouseenter" => "onMouseEnter",
            "mouseleave" => "onMouseLeave",
            "mousedown" => "onMouseDown",
            "mouseup" => "onMouseUp",
            other => other,
        };
        let rhs = if ev.target.trim_start().starts_with("[&]") || ev.target.trim_start().starts_with("[]") {
            // Already a complete lambda (handler-ref form built in the IR builder).
            ev.target.clone()
        } else {
            let cpp = translate_js(&ev.target, state_map);
            format!("[](JsObject e) {{ {} }}", cpp)
        };
        lines.push(format!("{}{}->{} = {};", indent, node.node_id, member, rhs));
    }
    if let Some(pid) = parent_id {
        lines.push(format!("{pid}->addChild({});", node.node_id));
    }
    for child in &node.children {
        lines.push(emit_node_with_state(child, Some(&node.node_id), features, state_map, Some(&node.style)));
    }
    lines.push(emit_reactive_effects(node, indent, features, state_map));
    lines.join("\n")
}

fn emit_conditional(node: &IRNode, parent_id: Option<&str>, indent: &str, features: &std::collections::HashSet<String>, state_map: &std::collections::HashMap<String, String>, parent_style: Option<&IRStyle>) -> String {
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
        then_code = emit_node_with_state(tn, None, features, state_map, parent_style);
        then_var = tn.node_id.clone();
        break;
    }
    let mut else_code = String::new();
    let mut else_var = String::new();
    for en in &node.else_nodes {
        else_code = emit_node_with_state(en, None, features, state_map, parent_style);
        else_var = en.node_id.clone();
        break;
    }
    lines.push(format!("{}->m_associatedEffects.push_back(morph::create_effect([{}, {then_slot}, {else_slot}]() {{", node.node_id, node.node_id));
    let bi = format!("{indent}    ");
    lines.push(format!("{bi}if ({}) {{", translate_condition(&node.condition_expr, state_map)));
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

fn emit_list(node: &IRNode, parent_id: Option<&str>, _indent: &str, state_map: &std::collections::HashMap<String, String>) -> String {
    let mut lines = vec![format!("morph::ListContainer* {} = new morph::ListContainer({}, {}, {}, {});", node.node_id, fmt(node.x), fmt(node.y), fmt(node.w), fmt(node.h))];
    if let Some(pid) = parent_id { lines.push(format!("{pid}->addChild({});", node.node_id)); }
    let array_expr = translate_js(&node.list_expr, state_map);
    let array_expr = array_expr.trim().trim_end_matches(';').to_string();
    lines.push(format!("{}->arrayFn = []() {{ return {}; }};", node.node_id, array_expr));
    lines.push(format!("{}->itemFactory = __list_factory_{};", node.node_id, node.node_id));
    if !node.list_key_expr.is_empty() {
        let key_expr = translate_list_key(&node.list_key_expr);
        lines.push(format!("{}->keyFn = [](const JsValue& __it, int __index) -> std::string {{ return morph::list_key({}, __index); }};", node.node_id, key_expr));
    }
    lines.push(format!("{}->m_associatedEffects.push_back(morph::create_effect([{}]() {{ {}->reconcile({}->arrayFn()); }}));", node.node_id, node.node_id, node.node_id, node.node_id));
    lines.join("\n")
}

/// Translate a JSX `key={expr}` where the map variable (`item`) refers to the
/// current list element, held in `__it` (a JsValue). JavaScript property access
/// `item.id` -> `__it["id"]`; simple `item` -> `__it`.
fn translate_list_key(expr: &str) -> String {
    let s = expr.trim();
    if let Some(rest) = s.strip_prefix("item.") {
        format!("__it[\"{}\"]", rest)
    } else if s == "item" {
        "__it".to_string()
    } else {
        s.to_string()
    }
}

/// Port of Python `_set_style`: emits every style field whose feature is on.
/// `is_text` controls the color-override/m_colorOverridden behavior for TextNode.
fn set_style(node: &IRNode, indent: &str, parent: Option<&IRStyle>, features: &std::collections::HashSet<String>, is_text: bool) -> String {
    let s = &node.style;
    let prefix = format!("{}->style", node.node_id);
    let mut lines = Vec::new();
    let ind = format!("{indent}{}->style", node.node_id);

    // ── StyleBase (always available) ──
    if s.bg_color != [0.0,0.0,0.0,0.0] {
        lines.push(format!("{ind}.bgColor[0] = {:.4}f;", s.bg_color[0]));
        lines.push(format!("{ind}.bgColor[1] = {:.4}f;", s.bg_color[1]));
        lines.push(format!("{ind}.bgColor[2] = {:.4}f;", s.bg_color[2]));
        lines.push(format!("{ind}.bgColor[3] = {:.4}f;", s.bg_color[3]));
    }
    if s.border_radius > 0.0 { lines.push(format!("{ind}.borderRadius = {};", fmt(s.border_radius))); }
    // font-size is inherited: use the node's own value, else parent's, else 16.
    let fs = if s.font_size != 16.0 { s.font_size }
        else { parent.map(|p| p.font_size).filter(|&v| v != 16.0).unwrap_or(16.0) };
    if fs != 16.0 { lines.push(format!("{ind}.fontSize = {};", fmt(fs))); }
    // font-weight is inherited similarly.
    let fw = if s.font_weight != "normal" && !s.font_weight.is_empty() { s.font_weight.clone() }
        else { parent.map(|p| p.font_weight.clone()).filter(|v| v != "normal" && !v.is_empty()).unwrap_or_else(|| "normal".into()) };
    if fw != "normal" { lines.push(format!("{ind}.fontWeight = \"{}\";", fw)); }
    // color: text nodes use their own color; others inherit unless explicitly set.
    let c = if is_text { s.color }
        else if s.color != [0.0,0.0,0.0,1.0] { s.color }
        else { parent.map(|p| p.color).filter(|&pc| pc != [0.0,0.0,0.0,1.0]).unwrap_or([0.0,0.0,0.0,1.0]) };
    let color_inherited = !is_text && s.color == [0.0,0.0,0.0,1.0] && c != [0.0,0.0,0.0,1.0];
    if c != [0.0,0.0,0.0,1.0] {
        lines.push(format!("{ind}.color[0] = {:.4}f;", c[0]));
        lines.push(format!("{ind}.color[1] = {:.4}f;", c[1]));
        lines.push(format!("{ind}.color[2] = {:.4}f;", c[2]));
        if is_text {
            lines.push(format!("{ind}.color[3] = {:.4}f;", c[3]));
            lines.push(format!("{}->m_colorOverridden = true;", node.node_id));
        } else if color_inherited {
            lines.push(format!("{}->m_colorInherited = true;", node.node_id));
        }
    }
    if s.padding != [0.0,0.0,0.0,0.0] {
        lines.push(format!("{ind}.padding[0] = {};", fmt(s.padding[0])));
        lines.push(format!("{ind}.padding[1] = {};", fmt(s.padding[1])));
        lines.push(format!("{ind}.padding[2] = {};", fmt(s.padding[2])));
        lines.push(format!("{ind}.padding[3] = {};", fmt(s.padding[3])));
    }
    if s.margin != [0.0,0.0,0.0,0.0] {
        for i in 0..4 {
            let v = s.margin[i];
            if v != 0.0 || s.margin_auto[i] {
                let coded = if v.is_finite() { fmt(v) } else { "-1.0f".into() };
                lines.push(format!("{ind}.margin[{i}] = {coded};"));
                if s.margin_auto[i] { lines.push(format!("{ind}.marginAuto[{i}] = true;")); }
            }
        }
    }
    if let Some(w) = s.width { lines.push(format!("{ind}.explicitWidth = {};", fmt(w))); }
    if let Some(h) = s.height { lines.push(format!("{ind}.explicitHeight = {};", fmt(h))); }
    if let Some(v) = s.min_width { lines.push(format!("{ind}.minWidth = {};", fmt(v))); }
    if let Some(v) = s.max_width { lines.push(format!("{ind}.maxWidth = {};", fmt(v))); }
    if let Some(v) = s.min_height { lines.push(format!("{ind}.minHeight = {};", fmt(v))); }
    if let Some(v) = s.max_height { lines.push(format!("{ind}.maxHeight = {};", fmt(v))); }
    if s.overflow != "visible" { lines.push(format!("{ind}.overflow = \"{}\";", s.overflow)); }
    if s.display != "block" { lines.push(format!("{ind}.display = \"{}\";", s.display)); }
    if s.position != "static" { lines.push(format!("{ind}.position = \"{}\";", s.position)); }
    if s.box_sizing != "content-box" { lines.push(format!("{ind}.boxSizing = \"{}\";", s.box_sizing)); }
    if s.text_align != "left" { lines.push(format!("{ind}.textAlign = \"{}\";", s.text_align)); }

    // Long-form transform: emit via runtime parser so % and functions resolve
    // against the element box at layout time (matches Python's resolved matrix).
    if features.contains("transform") {
        if let Some(ops) = &s.transform_ops {
            if let Some((name, args)) = ops.first() {
                if name == "rawcss" {
                    let css = args.first().map(|v| format!("{}", v)).unwrap_or_default();
                    // placeholder (raw transform handled via keyframes); avoid emitting invalid float parse
                    let _ = css;
                }
            }
        }
    }

    // ── FLEX ──
    if features.contains("flex") {
        if s.display == "flex" && s.flex_dir != "row" { lines.push(format!("{ind}.flexDirection = \"{}\";", s.flex_dir)); }
        if s.gap > 0.0 { lines.push(format!("{ind}.gap = {};", fmt(s.gap))); }
        if s.justify_content != "flex-start" { lines.push(format!("{ind}.justifyContent = \"{}\";", s.justify_content)); }
        if s.align_items != "stretch" { lines.push(format!("{ind}.alignItems = \"{}\";", s.align_items)); }
        if s.flex_wrap != "nowrap" { lines.push(format!("{ind}.flexWrap = \"{}\";", s.flex_wrap)); }
        if s.flex_grow != 0.0 { lines.push(format!("{ind}.flexGrow = {};", fmt(s.flex_grow))); }
        if s.flex_shrink != 1.0 { lines.push(format!("{ind}.flexShrink = {};", fmt(s.flex_shrink))); }
        if s.flex_basis != "auto" { lines.push(format!("{ind}.flexBasis = \"{}\";", s.flex_basis)); }
    }

    // ── POSITION ──
    if features.contains("position") {
        if let Some(v) = s.left { lines.push(format!("{ind}.left = {};", fmt(v))); }
        if let Some(v) = s.right { lines.push(format!("{ind}.right = {};", fmt(v))); }
        if let Some(v) = s.top { lines.push(format!("{ind}.top = {};", fmt(v))); }
        if let Some(v) = s.bottom { lines.push(format!("{ind}.bottom = {};", fmt(v))); }
    }

    // ── ZINDEX ──
    if features.contains("zindex") {
        if let Some(z) = s.z_index {
            lines.push(format!("{ind}.zIndex = {};", z));
            lines.push(format!("{ind}.zIndexSet = true;"));
        }
    }

    // ── OPACITY ──
    if features.contains("opacity") {
        if (s.opacity - 1.0).abs() > f32::EPSILON { lines.push(format!("{ind}.opacity = {};", fmt(s.opacity))); }
    }

    // ── CURSOR ──
    if features.contains("cursor") {
        if s.cursor != "default" && !s.cursor.is_empty() { lines.push(format!("{ind}.cursor = \"{}\";", s.cursor)); }
    }

    // ── BORDER ──
    if features.contains("border") {
        if s.border_width > 0.0 { lines.push(format!("{ind}.borderWidth = {};", fmt(s.border_width))); }
        if s.border_color != [0.0,0.0,0.0,1.0] {
            lines.push(format!("{ind}.borderColor[0] = {:.4}f;", s.border_color[0]));
            lines.push(format!("{ind}.borderColor[1] = {:.4}f;", s.border_color[1]));
            lines.push(format!("{ind}.borderColor[2] = {:.4}f;", s.border_color[2]));
            lines.push(format!("{ind}.borderColor[3] = {:.4}f;", s.border_color[3]));
        }
        if s.border_style != "none" && !s.border_style.is_empty() { lines.push(format!("{ind}.borderStyle = \"{}\";", s.border_style)); }
    }

    // ── SCROLL ──
    if features.contains("scroll") {
        if s.scrollbar_width != 8.0 { lines.push(format!("{ind}.scrollbarWidth = {};", fmt(s.scrollbar_width))); }
        if s.scrollbar_track_color != [0.85,0.85,0.85,0.4] {
            lines.push(format!("{ind}.scrollbarTrackColor[0] = {:.4}f;", s.scrollbar_track_color[0]));
            lines.push(format!("{ind}.scrollbarTrackColor[1] = {:.4}f;", s.scrollbar_track_color[1]));
            lines.push(format!("{ind}.scrollbarTrackColor[2] = {:.4}f;", s.scrollbar_track_color[2]));
            lines.push(format!("{ind}.scrollbarTrackColor[3] = {:.4}f;", s.scrollbar_track_color[3]));
        }
        if s.scrollbar_thumb_color != [0.5,0.5,0.5,0.6] {
            lines.push(format!("{ind}.scrollbarThumbColor[0] = {:.4}f;", s.scrollbar_thumb_color[0]));
            lines.push(format!("{ind}.scrollbarThumbColor[1] = {:.4}f;", s.scrollbar_thumb_color[1]));
            lines.push(format!("{ind}.scrollbarThumbColor[2] = {:.4}f;", s.scrollbar_thumb_color[2]));
            lines.push(format!("{ind}.scrollbarThumbColor[3] = {:.4}f;", s.scrollbar_thumb_color[3]));
        }
        if s.scrollbar_border_radius != 4.0 { lines.push(format!("{ind}.scrollbarBorderRadius = {};", fmt(s.scrollbar_border_radius))); }
    }

    let _ = prefix;
    if lines.is_empty() { "".into() } else { lines.join("\n") }
}

/// Emit `node->hoverStyle = new MorphStyle(); ...` delta overrides.
/// Returns "" when there is no hover_style.
fn emit_hover_style(node: &IRNode, indent: &str, features: &std::collections::HashSet<String>, _alloc: bool) -> String {
    let Some(s) = &node.hover_style else { return String::new(); };
    let base = &node.style;
    let hv = format!("{}->hoverStyle", node.node_id);
    let mut o: Vec<String> = Vec::new();
    if s.bg_color != [0.0,0.0,0.0,0.0] && s.bg_color != base.bg_color {
        o.push(format!("{hv}->bgColor[0] = {:.4}f;", s.bg_color[0]));
        o.push(format!("{hv}->bgColor[1] = {:.4}f;", s.bg_color[1]));
        o.push(format!("{hv}->bgColor[2] = {:.4}f;", s.bg_color[2]));
        o.push(format!("{hv}->bgColor[3] = {:.4}f;", s.bg_color[3]));
    }
    if s.color != [0.0,0.0,0.0,1.0] && s.color != base.color {
        o.push(format!("{hv}->color[0] = {:.4}f;", s.color[0]));
        o.push(format!("{hv}->color[1] = {:.4}f;", s.color[1]));
        o.push(format!("{hv}->color[2] = {:.4}f;", s.color[2]));
    }
    if s.border_color != [0.0,0.0,0.0,1.0] && s.border_color != base.border_color {
        o.push(format!("{hv}->borderColor[0] = {:.4}f;", s.border_color[0]));
        o.push(format!("{hv}->borderColor[1] = {:.4}f;", s.border_color[1]));
        o.push(format!("{hv}->borderColor[2] = {:.4}f;", s.border_color[2]));
        o.push(format!("{hv}->borderColor[3] = {:.4}f;", s.border_color[3]));
    }
    if s.border_radius > 0.0 && s.border_radius != base.border_radius { o.push(format!("{hv}->borderRadius = {};", fmt(s.border_radius))); }
    if s.border_width > 0.0 && s.border_width != base.border_width { o.push(format!("{hv}->borderWidth = {};", fmt(s.border_width))); }
    if s.border_style != "none" && !s.border_style.is_empty() && s.border_style != base.border_style { o.push(format!("{hv}->borderStyle = \"{}\";", s.border_style)); }
    if s.padding != [0.0,0.0,0.0,0.0] && s.padding != base.padding {
        o.push(format!("{hv}->padding[0] = {};", fmt(s.padding[0])));
        o.push(format!("{hv}->padding[1] = {};", fmt(s.padding[1])));
        o.push(format!("{hv}->padding[2] = {};", fmt(s.padding[2])));
        o.push(format!("{hv}->padding[3] = {};", fmt(s.padding[3])));
    }
    if s.font_size != 16.0 && s.font_size != base.font_size { o.push(format!("{hv}->fontSize = {};", fmt(s.font_size))); }
    if s.font_weight != "normal" && !s.font_weight.is_empty() && s.font_weight != base.font_weight { o.push(format!("{hv}->fontWeight = \"{}\";", s.font_weight)); }
    if s.width.is_some() && s.width != base.width { o.push(format!("{hv}->explicitWidth = {};", fmt(s.width.unwrap()))); }
    if s.height.is_some() && s.height != base.height { o.push(format!("{hv}->explicitHeight = {};", fmt(s.height.unwrap()))); }
    if s.display != "block" && s.display != base.display { o.push(format!("{hv}->display = \"{}\";", s.display)); }
    if s.overflow != "visible" && s.overflow != base.overflow { o.push(format!("{hv}->overflow = \"{}\";", s.overflow)); }
    if s.position != "static" && s.position != base.position { o.push(format!("{hv}->position = \"{}\";", s.position)); }
    if s.box_sizing != "content-box" && s.box_sizing != base.box_sizing { o.push(format!("{hv}->boxSizing = \"{}\";", s.box_sizing)); }
    if features.contains("flex") {
        if s.flex_dir != "row" && s.flex_dir != base.flex_dir { o.push(format!("{hv}->flexDirection = \"{}\";", s.flex_dir)); }
        if s.gap > 0.0 && s.gap != base.gap { o.push(format!("{hv}->gap = {};", fmt(s.gap))); }
        if s.justify_content != "flex-start" && s.justify_content != base.justify_content { o.push(format!("{hv}->justifyContent = \"{}\";", s.justify_content)); }
        if s.align_items != "stretch" && s.align_items != base.align_items { o.push(format!("{hv}->alignItems = \"{}\";", s.align_items)); }
        if s.flex_wrap != "nowrap" && s.flex_wrap != base.flex_wrap { o.push(format!("{hv}->flexWrap = \"{}\";", s.flex_wrap)); }
    }
    if features.contains("position") {
        if s.left.is_some() && s.left != base.left { o.push(format!("{hv}->left = {};", fmt(s.left.unwrap()))); }
        if s.right.is_some() && s.right != base.right { o.push(format!("{hv}->right = {};", fmt(s.right.unwrap()))); }
        if s.top.is_some() && s.top != base.top { o.push(format!("{hv}->top = {};", fmt(s.top.unwrap()))); }
        if s.bottom.is_some() && s.bottom != base.bottom { o.push(format!("{hv}->bottom = {};", fmt(s.bottom.unwrap()))); }
    }
    if features.contains("zindex") {
        if s.z_index.is_some() && s.z_index != base.z_index {
            o.push(format!("{hv}->zIndex = {};", s.z_index.unwrap()));
            o.push(format!("{hv}->zIndexSet = true;"));
        }
    }
    if features.contains("opacity") {
        if (s.opacity - 1.0).abs() > f32::EPSILON && s.opacity != base.opacity { o.push(format!("{hv}->opacity = {};", fmt(s.opacity))); }
    }
    if features.contains("cursor") {
        if s.cursor != "default" && !s.cursor.is_empty() && s.cursor != base.cursor { o.push(format!("{hv}->cursor = \"{}\";", s.cursor)); }
    }
    if o.is_empty() { return String::new(); }
    let mut lines = vec![format!("{} = new MorphStyle(); // delta only", hv)];
    for l in o { lines.push(format!("{indent}{l}")); }
    lines.join("\n")
}

fn emit_active_style(node: &IRNode, indent: &str, features: &std::collections::HashSet<String>) -> String {
    let Some(s) = &node.active_style else { return String::new(); };
    let base = &node.style;
    let hv = format!("{}->activeStyle", node.node_id);
    let mut o: Vec<String> = Vec::new();
    if s.bg_color != [0.0,0.0,0.0,0.0] && s.bg_color != base.bg_color {
        o.push(format!("{hv}->bgColor[0] = {:.4}f;", s.bg_color[0]));
        o.push(format!("{hv}->bgColor[1] = {:.4}f;", s.bg_color[1]));
        o.push(format!("{hv}->bgColor[2] = {:.4}f;", s.bg_color[2]));
        o.push(format!("{hv}->bgColor[3] = {:.4}f;", s.bg_color[3]));
    }
    if s.color != [0.0,0.0,0.0,1.0] && s.color != base.color {
        o.push(format!("{hv}->color[0] = {:.4}f;", s.color[0]));
        o.push(format!("{hv}->color[1] = {:.4}f;", s.color[1]));
        o.push(format!("{hv}->color[2] = {:.4}f;", s.color[2]));
    }
    if s.border_radius > 0.0 && s.border_radius != base.border_radius { o.push(format!("{hv}->borderRadius = {};", fmt(s.border_radius))); }
    if s.border_width > 0.0 && s.border_width != base.border_width { o.push(format!("{hv}->borderWidth = {};", fmt(s.border_width))); }
    if features.contains("zindex") && s.z_index.is_some() && s.z_index != base.z_index {
        o.push(format!("{hv}->zIndex = {};", s.z_index.unwrap()));
        o.push(format!("{hv}->zIndexSet = true;"));
    }
    if features.contains("opacity") && (s.opacity - 1.0).abs() > f32::EPSILON && s.opacity != base.opacity { o.push(format!("{hv}->opacity = {};", fmt(s.opacity))); }
    if o.is_empty() { return String::new(); }
    let mut lines = vec![format!("{} = new MorphStyle(); // delta only", hv)];
    for l in o { lines.push(format!("{indent}{l}")); }
    lines.join("\n")
}

fn emit_transition(node: &IRNode, indent: &str) -> String {
    if node.transition_duration <= 0.0 { return String::new(); }
    let easing = match node.transition_easing.as_str() {
        "linear" => "Easing::Linear",
        "ease-in" => "Easing::EaseIn",
        "ease-out" => "Easing::EaseOut",
        _ => "Easing::EaseInOut",
    };
    format!("{}{}->m_transitionDuration = {};\n{}{}->m_transitionEasing = {};",
        indent, node.node_id, fmt(node.transition_duration),
        indent, node.node_id, easing)
}

fn anim_easing(a: &morph_ir::IRAnimation) -> &'static str {
    match a.easing.as_str() {
        "linear" => "Easing::Linear",
        "ease-in" => "Easing::EaseIn",
        "ease-out" => "Easing::EaseOut",
        _ => "Easing::EaseInOut",
    }
}
fn anim_direction(a: &morph_ir::IRAnimation) -> &'static str {
    match a.direction.as_str() {
        "reverse" => "AnimDirection::Reverse",
        "alternate" => "AnimDirection::Alternate",
        "alternate-reverse" => "AnimDirection::AlternateReverse",
        _ => "AnimDirection::Normal",
    }
}
fn anim_fill(a: &morph_ir::IRAnimation) -> &'static str {
    match a.fill_mode.as_str() {
        "forwards" => "AnimFillMode::Forwards",
        "backwards" => "AnimFillMode::Backwards",
        "both" => "AnimFillMode::Both",
        _ => "AnimFillMode::None",
    }
}
fn anim_iterations(n: f32) -> String {
    if n < 0.0 { "-1.0f".into() } else { fmt(n) }
}

fn emit_animations(node: &IRNode, indent: &str, features: &std::collections::HashSet<String>) -> String {
    if !features.contains("animation") || node.animations.is_empty() { return String::new(); }
    let prefix = format!("{}->style.animations", node.node_id);
    let mut lines = Vec::new();
    for a in &node.animations {
        let running = if a.play_state == "running" { "true" } else { "false" };
        lines.push(format!("{}{prefix}.push_back(CssAnimation{{\"{}\", {}, {}, {}, {}, {}, {}, {}}});",
            indent, a.name, fmt(a.duration), anim_easing(a), fmt(a.delay),
            anim_iterations(a.iterations), anim_direction(a), anim_fill(a), running));
    }
    lines.join("\n")
}

fn emit_hover_animations(node: &IRNode, indent: &str, features: &std::collections::HashSet<String>) -> String {
    if !features.contains("animation") || node.hover_animations.is_empty() { return String::new(); }
    let mut lines = vec![];
    let hv = format!("{}->hoverStyle", node.node_id);
    for (i, a) in node.hover_animations.iter().enumerate() {
        let running = if a.play_state == "running" { "true" } else { "false" };
        if i == 0 {
            lines.push(format!("{indent}if (!{hv}) {hv} = new MorphStyle(); // delta only"));
        }
        lines.push(format!("{indent}{hv}->animations.push_back(CssAnimation{{\"{}\", {}, {}, {}, {}, {}, {}, {}}});",
            a.name, fmt(a.duration), anim_easing(a), fmt(a.delay),
            anim_iterations(a.iterations), anim_direction(a), anim_fill(a), running));
    }
    lines.join("\n")
}

/// CSS property → (C++ style field name, value type). Matches Python's
/// `_CSS_TO_STYLE_FIELD` in logic_emitter.py so reactive styles and
/// conditional-class effects write to the same C++ style fields.
fn css_to_style_field(css_prop: &str) -> Option<(&'static str, &'static str)> {
    Some(match css_prop {
        "background-color" => ("bgColor", "color"),
        "color" => ("color", "color"),
        "border-color" => ("borderColor", "color"),
        "scrollbar-track-color" => ("scrollbarTrackColor", "color"),
        "scrollbar-thumb-color" => ("scrollbarThumbColor", "color"),
        "width" => ("explicitWidth", "float"),
        "min-width" => ("minWidth", "float"),
        "max-width" => ("maxWidth", "float"),
        "height" => ("explicitHeight", "float"),
        "min-height" => ("minHeight", "float"),
        "max-height" => ("maxHeight", "float"),
        "border-radius" => ("borderRadius", "float"),
        "font-size" => ("fontSize", "float"),
        "gap" => ("gap", "float"),
        "left" => ("left", "float"),
        "right" => ("right", "float"),
        "top" => ("top", "float"),
        "bottom" => ("bottom", "float"),
        "flex-grow" => ("flexGrow", "float"),
        "flex-shrink" => ("flexShrink", "float"),
        "scrollbar-width" => ("scrollbarWidth", "float"),
        "scrollbar-border-radius" => ("scrollbarBorderRadius", "float"),
        "border-width" => ("borderWidth", "float"),
        "z-index" => ("zIndex", "int"),
        "opacity" => ("opacity", "float"),
        "font-weight" => ("fontWeight", "string"),
        "text-align" => ("textAlign", "string"),
        "display" => ("display", "string"),
        "flex-direction" => ("flexDirection", "string"),
        "flex-wrap" => ("flexWrap", "string"),
        "justify-content" => ("justifyContent", "string"),
        "align-items" => ("alignItems", "string"),
        "cursor" => ("cursor", "string"),
        "border-style" => ("borderStyle", "string"),
        "box-sizing" => ("boxSizing", "string"),
        "overflow" => ("overflow", "string"),
        "position" => ("position", "string"),
        "flex-basis" => ("flexBasis", "string"),
        "transform" => ("transform", "transform"),
        _ => return None,
    })
}

/// Default/clean value per style field (used when resetting class-based styles).
fn css_field_reset(node_var: &str, field_name: &str, indent: &str) -> Vec<String> {
    if field_name == "transform" {
        return vec![format!("{indent}        morph::resetCssTransform({node_var}->style);")];
    }
    let prefix = format!("{node_var}->style.{field_name}");
    let lines: Vec<String> = match field_name {
        "bgColor" => vec!["[0] = 0.0000f;", "[1] = 0.0000f;", "[2] = 0.0000f;", "[3] = 0.0000f;"]
            .into_iter().map(|s| format!("{indent}        {prefix}{s}")).collect(),
        "color" | "borderColor" => vec!["[0] = 0.0000f;", "[1] = 0.0000f;", "[2] = 0.0000f;", "[3] = 1.0000f;"]
            .into_iter().map(|s| format!("{indent}        {prefix}{s}")).collect(),
        "scrollbarTrackColor" => vec!["[0] = 0.8500f;", "[1] = 0.8500f;", "[2] = 0.8500f;", "[3] = 0.4000f;"]
            .into_iter().map(|s| format!("{indent}        {prefix}{s}")).collect(),
        "scrollbarThumbColor" => vec!["[0] = 0.5000f;", "[1] = 0.5000f;", "[2] = 0.5000f;", "[3] = 0.6000f;"]
            .into_iter().map(|s| format!("{indent}        {prefix}{s}")).collect(),
        "explicitWidth" | "explicitHeight" | "minWidth" | "maxWidth" | "minHeight" | "maxHeight" => {
            vec![format!("{indent}        {prefix} = -1.0f;")]
        }
        "borderRadius" | "left" | "right" | "top" | "bottom" | "flexGrow" | "scrollbarWidth" | "borderWidth" => {
            vec![format!("{indent}        {prefix} = 0.0f;")]
        }
        "fontSize" => vec![format!("{indent}        {prefix} = 16.0f;")],
        "gap" => vec![format!("{indent}        {prefix} = 0.0f;")],
        "flexShrink" => vec![format!("{indent}        {prefix} = 1.0f;")],
        "scrollbarBorderRadius" => vec![format!("{indent}        {prefix} = 4.0f;")],
        "opacity" => vec![format!("{indent}        {prefix} = 1.0f;")],
        "zIndex" => vec![format!("{indent}        {prefix} = 0;"), format!("{indent}        {node_var}->style.zIndexSet = false;")],
        "fontWeight" => vec![format!("{indent}        {prefix} = \"normal\";")],
        "textAlign" => vec![format!("{indent}        {prefix} = \"left\";")],
        "display" => vec![format!("{indent}        {prefix} = \"block\";")],
        "flexDirection" => vec![format!("{indent}        {prefix} = \"row\";")],
        "flexWrap" => vec![format!("{indent}        {prefix} = \"nowrap\";")],
        "justifyContent" => vec![format!("{indent}        {prefix} = \"flex-start\";")],
        "alignItems" => vec![format!("{indent}        {prefix} = \"stretch\";")],
        "cursor" => vec![format!("{indent}        {prefix} = \"default\";")],
        "borderStyle" => vec![format!("{indent}        {prefix} = \"none\";")],
        "boxSizing" => vec![format!("{indent}        {prefix} = \"content-box\";")],
        "overflow" => vec![format!("{indent}        {prefix} = \"visible\";")],
        "position" => vec![format!("{indent}        {prefix} = \"static\";")],
        "flexBasis" => vec![format!("{indent}        {prefix} = \"auto\";")],
        _ => vec![],
    };
    lines
}

/// Resolve a literal CSS value to C++ `<node_var>->style.<field>` assignments.
fn css_val_to_cpp(node_var: &str, css_prop: &str, css_val: &str, indent: &str) -> Vec<String> {
    let Some((field_name, val_type)) = css_to_style_field(css_prop) else { return vec![] };
    let prefix = format!("{node_var}->style.{field_name}");
    let out: Vec<String> = match val_type {
        "float" => {
            let v = css_val.trim();
            let num = v.trim_end_matches("px").trim();
            match num.parse::<f32>() {
                Ok(f) => vec![format!("{indent}        {prefix} = {}f;", fmt(f))],
                Err(_) => vec![],
            }
        }
        "int" => {
            let raw = css_val.trim();
            let v: f32 = if raw.eq_ignore_ascii_case("auto") { 0.0 } else {
                raw.parse().unwrap_or(0.0)
            };
            vec![format!("{indent}        {prefix} = {};", v as i64), format!("{indent}        {node_var}->style.zIndexSet = true;")]
        }
        "string" => {
            let esc = css_val.replace('\\', "\\\\").replace('"', "\\\"");
            vec![format!("{indent}        {prefix} = \"{esc}\";")]
        }
        "color" => {
            match parse_color_val(css_val) {
                Some((r, g, b, a)) => vec![
                    format!("{indent}        {prefix}[0] = {r:.4}f;"),
                    format!("{indent}        {prefix}[1] = {g:.4}f;"),
                    format!("{indent}        {prefix}[2] = {b:.4}f;"),
                    format!("{indent}        {prefix}[3] = {a:.4}f;"),
                ],
                None => vec![],
            }
        }
        "transform" => {
            let esc = css_val.replace('\\', "\\\\").replace('"', "\\\"");
            vec![format!("{indent}        morph::setCssTransform({node_var}->style, \"{esc}\", {node_var}->w, {node_var}->h);")]
        }
        _ => vec![],
    };
    out
}

fn parse_color_val(s: &str) -> Option<(f32, f32, f32, f32)> {
    let c = s.trim().to_lowercase();
    if let Some(hex) = c.strip_prefix('#') {
        let (r, g, b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&format!("{}{}", &hex[0..1], &hex[0..1]), 16).ok()?;
                let g = u8::from_str_radix(&format!("{}{}", &hex[1..2], &hex[1..2]), 16).ok()?;
                let b = u8::from_str_radix(&format!("{}{}", &hex[2..3], &hex[2..3]), 16).ok()?;
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b)
            }
            _ => return None,
        };
        return Some((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0));
    }
    Some(match c.as_str() {
        "transparent" => (0.0, 0.0, 0.0, 0.0),
        "white" => (1.0, 1.0, 1.0, 1.0),
        "black" => (0.0, 0.0, 0.0, 1.0),
        "red" => (1.0, 0.0, 0.0, 1.0),
        "blue" => (0.0, 0.0, 1.0, 1.0),
        _ => return None,
    })
}

/// Port of Python `_emit_node_effects` reactive portions (logic_emitter.py:652-772):
/// reactive style, font-size propagation, reactive attrs, reactive className,
/// and conditional-class style effects. Uses the Rust emitter's
/// `m_associatedEffects` / direct node-pointer mechanism (not Python's dev-mode
/// `__effects` array / `nodes` map), consistent with `emit_conditional`/reactive text.
fn emit_reactive_effects(node: &IRNode, indent: &str, _features: &std::collections::HashSet<String>, state_map: &std::collections::HashMap<String, String>) -> String {
    let mut lines = Vec::new();
    let id = &node.node_id;

    let open_effect = |captures: &str| format!("{indent}{id}->m_associatedEffects.push_back(morph::create_effect([{captures}]() {{");
    let close_effect = "}));";

    // ── Reactive style (inline style expressions) ──
    if !node.reactive_style.is_empty() {
        let mut props: Vec<(&String, &String)> = node.reactive_style.iter().collect();
        props.sort_by(|a, b| a.0.cmp(b.0));
        for (css_prop, raw_expr) in props {
            let Some((field_name, val_type)) = css_to_style_field(css_prop) else { continue };
            let cpp = translate_js(raw_expr, state_map);
            let cpp = cpp.trim().trim_end_matches(';').to_string();
            match val_type {
                "float" => {
                    lines.push(open_effect(id));
                    lines.push(format!("{indent}    {id}->interruptStateTransitions();"));
                    lines.push(format!("{indent}    {id}->style.{field_name} = (float)({cpp});"));
                    lines.push(format!("{indent}    {id}->markDirty(LayoutDirty);"));
                    lines.push(format!("{indent}    {id}->markDirty(PaintDirty);"));
                    lines.push(close_effect.to_string());
                }
                "string" => {
                    lines.push(open_effect(id));
                    lines.push(format!("{indent}    {id}->interruptStateTransitions();"));
                    lines.push(format!("{indent}    {id}->style.{field_name} = morph::str({cpp});"));
                    lines.push(format!("{indent}    {id}->markDirty(PaintDirty);"));
                    lines.push(close_effect.to_string());
                }
                "color" => {
                    lines.push(open_effect(id));
                    lines.push(format!("{indent}    {id}->interruptStateTransitions();"));
                    lines.push(format!("{indent}    morph::setColor({id}->style.{field_name}, morph::str({cpp}));"));
                    lines.push(format!("{indent}    {id}->markDirty(PaintDirty);"));
                    lines.push(close_effect.to_string());
                }
                "transform" => {
                    lines.push(open_effect(id));
                    lines.push(format!("{indent}    {id}->interruptStateTransitions();"));
                    lines.push(format!("{indent}    morph::setCssTransform({id}->style, morph::str({cpp}), {id}->w, {id}->h);"));
                    lines.push(format!("{indent}    {id}->markDirty(PaintDirty);"));
                    lines.push(close_effect.to_string());
                }
                _ => {}
            }
        }
    }

    // ── Propagate font-size to child TextNodes ──
    if node.reactive_style.contains_key("font-size") {
        let cpp = translate_js(node.reactive_style["font-size"].as_str(), state_map);
        let cpp = cpp.trim().trim_end_matches(';').to_string();
        for child in &node.children {
            if child.node_type != "__text__" { continue; }
            lines.push(open_effect(id));
            lines.push(format!("{indent}    {}->interruptStateTransitions();", child.node_id));
            lines.push(format!("{indent}    {}->style.fontSize = (float)({cpp});", child.node_id));
            lines.push(format!("{indent}    {}->markDirty(LayoutDirty);", child.node_id));
            lines.push(format!("{indent}    {}->markDirty(PaintDirty);", child.node_id));
            lines.push(close_effect.to_string());
        }
    }

    // ── Reactive attrs (src/alt on img, value on input) ──
    if !node.reactive_attrs.is_empty() {
        let mut attrs: Vec<(&String, &String)> = node.reactive_attrs.iter().collect();
        attrs.sort_by(|a, b| a.0.cmp(b.0));
        for (attr_key, raw_expr) in attrs {
            let cpp = translate_js(raw_expr, state_map);
            let cpp = cpp.trim().trim_end_matches(';').to_string();
            lines.push(open_effect(id));
            if attr_key == "src" && node.node_type == "img" {
                lines.push(format!("{indent}    auto* img = static_cast<ImageNode*>({id});"));
                lines.push(format!("{indent}    std::string _src = morph::str({cpp});"));
                lines.push(format!("{indent}    if (img->src != _src) {{"));
                lines.push(format!("{indent}        img->src = _src;"));
                lines.push(format!("{indent}        img->loaded = false;"));
                lines.push(format!("{indent}    }}"));
            } else if attr_key == "value" && node.node_type == "input" {
                lines.push(format!("{indent}    static_cast<InputNode*>({id})->setValue(morph::str({cpp}));"));
            } else {
                lines.push(format!("{indent}    {id}->{attr_key} = morph::str({cpp});"));
            }
            lines.push(format!("{indent}    {id}->markDirty(PaintDirty);"));
            lines.push(close_effect.to_string());
        }
    }

    // ── Reactive className (stores string on node) ──
    if !node.reactive_class.is_empty() {
        let cpp = translate_js(&node.reactive_class, state_map);
        let cpp = cpp.trim().trim_end_matches(';').to_string();
        lines.push(open_effect(id));
        lines.push(format!("{indent}    {id}->setClassName(\"{cpp}\");"));
        lines.push(close_effect.to_string());
    }

    // ── Conditional class style effects (direct condition → style) ──
    if !node.class_conditional_effects.is_empty() {
        for eff in &node.class_conditional_effects {
            let cond_cpp = translate_js(&eff.condition, state_map);
            let cond_cpp = cond_cpp.trim().trim_end_matches(';').to_string();
            lines.push(open_effect(id));
            lines.push(format!("{indent}    {id}->interruptStateTransitions();"));
            lines.push(format!("{indent}    if ({cond_cpp}) {{"));
            let mut on_props: Vec<(&String, &String)> = eff.on_styles.iter().collect();
            on_props.sort_by(|a, b| a.0.cmp(b.0));
            for (prop, val) in &on_props {
                for a in css_val_to_cpp(id, prop, val, indent) { lines.push(a); }
            }
            lines.push(format!("{indent}    }} else {{"));
            if eff.off_styles.is_empty() {
                let mut reset_fields: Vec<&str> = Vec::new();
                for prop in on_props.iter().map(|p| p.0.as_str()) {
                    if let Some((fname, _)) = css_to_style_field(prop) {
                        if !reset_fields.contains(&fname) { reset_fields.push(fname); }
                    }
                }
                reset_fields.sort();
                for fname in reset_fields {
                    for a in css_field_reset(id, fname, indent) { lines.push(a); }
                }
            } else {
                let mut off_props: Vec<(&String, &String)> = eff.off_styles.iter().collect();
                off_props.sort_by(|a, b| a.0.cmp(b.0));
                for (prop, val) in off_props {
                    for a in css_val_to_cpp(id, prop, val, indent) { lines.push(a); }
                }
            }
            lines.push(format!("{indent}    }}"));
            lines.push(format!("{indent}    {id}->markDirty(PaintDirty);"));
            lines.push(close_effect.to_string());
        }
    }

    lines.join("\n")
}

/// C++ registering the global @keyframes registry (feature: animation).
/// Port of Python `keyframe_registration_code`.
pub fn keyframe_registration_code(
    keyframes: &std::collections::HashMap<String, Vec<morph_ir::IRKeyframe>>,
    features: &std::collections::HashSet<String>,
) -> String {
    if !features.contains("animation") || keyframes.is_empty() { return String::new(); }
    let mut lines = vec!["// ── @keyframes registry (feature: animation) ──".to_string(), "morphClearKeyframes();".to_string()];
    let mut names: Vec<&String> = keyframes.keys().collect();
    names.sort();
    for name in names {
        for kf in &keyframes[name] {
            lines.push(format!("morphAddKeyframe(\"{}\", {}, {{", name, fmt(kf.offset)));
            for f in &kf.declared {
                if let Some((enum_name, value)) = keyframe_style_value(f, &kf.style) {
                    lines.push(format!("    {{KeyframeProperty::{enum_name}, {{{value}}}}},",));
                }
            }
            for (prop, css) in &kf.raw {
                if let Some(enum_name) = raw_prop_to_enum(prop.as_str()) {
                    let escaped = css.replace('\\', "\\\\").replace('"', "\\\"");
                    lines.push(format!("    {{KeyframeProperty::{enum_name}, {{}}, \"{escaped}\"}},"));
                }
            }
            lines.push("});".into());
        }
    }
    lines.join("\n")
}

fn keyframe_style_value(field: &str, s: &IRStyle) -> Option<(&'static str, String)> {
    match field {
        "opacity" => Some(("Opacity", fmt(s.opacity))),
        "bg_color" => Some(("BgColor", color4(&s.bg_color))),
        "color" => Some(("Color", color4(&s.color))),
        "border_radius" => Some(("BorderRadius", fmt(s.border_radius))),
        "font_size" => Some(("FontSize", fmt(s.font_size))),
        "width" => s.width.map(|w| ("Width", fmt(w))),
        "height" => s.height.map(|h| ("Height", fmt(h))),
        "left" => s.left.map(|v| ("Left", fmt(v))),
        "top" => s.top.map(|v| ("Top", fmt(v))),
        _ => None,
    }
}

fn raw_prop_to_enum(prop: &str) -> Option<&'static str> {
    match prop {
        "opacity" => Some("Opacity"),
        "background-color" => Some("BgColor"),
        "color" => Some("Color"),
        "border-radius" => Some("BorderRadius"),
        "font-size" => Some("FontSize"),
        "width" => Some("Width"),
        "height" => Some("Height"),
        "left" => Some("Left"),
        "top" => Some("Top"),
        "transform" => Some("Transform"),
        _ => None,
    }
}

