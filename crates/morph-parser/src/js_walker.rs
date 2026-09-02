use std::collections::HashMap;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};

use super::ast_types::*;

// ── Helpers ────────────────────────────────────────────────────────────────

fn camel_to_kebab(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('-');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }
    result
}

fn normalize_jsx_text(raw: &str) -> Option<String> {
    if !raw.contains('\n') {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }
        return Some(trimmed.to_string());
    }
    let lines: Vec<&str> = raw.lines().map(|l| l.trim()).collect();
    let mut first = -1i32;
    let mut last = -1i32;
    for (i, ln) in lines.iter().enumerate() {
        if !ln.is_empty() {
            if first < 0 {
                first = i as i32;
            }
            last = i as i32;
        }
    }
    if first < 0 {
        return Some(" ".to_string());
    }
    let parts: Vec<&str> = lines[(first as usize)..=(last as usize)]
        .iter()
        .filter(|ln| !ln.is_empty())
        .copied()
        .collect();
    Some(parts.join(" "))
}

fn member_expr_to_string(expr: &MemberExpression) -> String {
    match expr {
        MemberExpression::ComputedMemberExpression(c) => {
            format!("{}[...]", member_expr_obj_str(&c.object))
        }
        MemberExpression::StaticMemberExpression(s) => {
            format!("{}.{}", member_expr_obj_str(&s.object), s.property.name)
        }
        MemberExpression::PrivateFieldExpression(p) => {
            format!("{}#{}", member_expr_obj_str(&p.object), p.field.name)
        }
    }
}

fn member_expr_obj_str(expr: &Expression) -> String {
    match expr {
        Expression::Identifier(id) => id.name.to_string(),
        _ => expr.span().source_text("").to_string(),
    }
}

fn is_member_expr_callee(expr: &Expression, obj: &str, prop: &str) -> bool {
    // Check for `obj.prop` via StaticMemberExpression / Chain etc.
    if let Some(me) = expr.as_member_expression() {
        if let MemberExpression::StaticMemberExpression(s) = me {
            if let Expression::Identifier(id) = &s.object {
                return id.name.as_str() == obj && s.property.name.as_str() == prop;
            }
        }
    }
    false
}

fn extract_string_lit(expr: &Expression) -> Option<String> {
    if let Expression::StringLiteral(sl) = expr {
        Some(sl.value.to_string())
    } else {
        None
    }
}

fn binding_ident_name(bp: &BindingPattern) -> Option<String> {
    if let BindingPattern::BindingIdentifier(id) = bp {
        Some(id.name.to_string())
    } else {
        None
    }
}

fn binding_array_elements(bp: &BindingPattern) -> Option<Vec<Option<String>>> {
    if let BindingPattern::ArrayPattern(arr) = bp {
        Some(
            arr.elements
                .iter()
                .map(|e| {
                    e.as_ref()
                        .and_then(|bp| binding_ident_name(bp))
                })
                .collect(),
        )
    } else {
        None
    }
}

fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offs = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' { offs.push(i + 1); }
    }
    offs
}

fn offset_to_line_col(source: &str, offset: u32) -> (usize, usize) {
    let offset = offset as usize;
    let mut line = 1usize;
    let mut last = 0usize;
    for (i, ch) in source.char_indices() {
        if i >= offset { break; }
        if ch == '\n' { line += 1; last = i + 1; }
    }
    let col = offset.saturating_sub(last) + 1;
    (line, col)
}

#[inline]
fn offset_to_line_col_fast(offsets: &[usize], offset: u32) -> (usize, usize) {
    let off = offset as usize;
    let line = match offsets.binary_search(&off) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    };
    let line_start = offsets[line.saturating_sub(1)];
    (line, off.saturating_sub(line_start) + 1)
}

// ── Walker ─────────────────────────────────────────────────────────────────

pub struct MxWalker<'src> {
    source: &'src str,
    line_offsets: Vec<usize>,
    pub imports: Vec<MxImport>,
    pub window_config: Option<WindowConfig>,
    pub components: Vec<MxComponent>,
    pub state_vars: Vec<StateVar>,
    pub effects: Vec<MxEffect>,
    pub inner_functions: Vec<InnerFunction>,
    pub function_declarations: Vec<InnerFunction>,
    pub global_vars: Vec<String>,
    pub console_logs: Vec<String>,
    pub extra_headers: Vec<String>,
    pub cpp_imports: Vec<CppImport>,
}

impl<'src> MxWalker<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            source,
            line_offsets: build_line_offsets(source),
            imports: Vec::new(),
            window_config: None,
            components: Vec::new(),
            state_vars: Vec::new(),
            effects: Vec::new(),
            inner_functions: Vec::new(),
            function_declarations: Vec::new(),
            global_vars: Vec::new(),
            console_logs: Vec::new(),
            extra_headers: Vec::new(),
            cpp_imports: Vec::new(),
        }
    }

    fn span_text(&self, span: Span) -> &'src str {
        span.source_text(self.source)
    }

    #[inline]
    fn line_col(&self, offset: u32) -> (usize, usize) {
        offset_to_line_col_fast(&self.line_offsets, offset)
    }

    // ── JSX helpers ────────────────────────────────────────────────────────

    fn jsx_tag_name(name: &JSXElementName) -> String {
        match name {
            JSXElementName::Identifier(id) => id.name.to_string(),
            JSXElementName::IdentifierReference(id) => id.name.to_string(),
            JSXElementName::NamespacedName(n) => format!("{}:{}", n.namespace.name, n.name.name),
            JSXElementName::MemberExpression(m) => {
                let mut parts = Vec::new();
                Self::collect_jsx_member_parts(&m.object, &mut parts);
                parts.push(m.property.name.to_string());
                parts.join(".")
            }
            JSXElementName::ThisExpression(_) => "this".to_string(),
        }
    }

    fn collect_jsx_member_parts(obj: &JSXMemberExpressionObject, parts: &mut Vec<String>) {
        match obj {
            JSXMemberExpressionObject::IdentifierReference(id) => parts.push(id.name.to_string()),
            JSXMemberExpressionObject::MemberExpression(m) => {
                Self::collect_jsx_member_parts(&m.object, parts);
                parts.push(m.property.name.to_string());
            }
            JSXMemberExpressionObject::ThisExpression(_) => parts.push("this".to_string()),
        }
    }

    fn jsx_prop_value(&self, val: &JSXAttributeValue) -> JsxPropValue {
        match val {
            JSXAttributeValue::StringLiteral(sl) => JsxPropValue::String(sl.value.to_string()),
            JSXAttributeValue::ExpressionContainer(ec) => self.jsx_expr_to_prop(&ec.expression),
            JSXAttributeValue::Element(e) => {
                JsxPropValue::Expr(format!("<{}>", Self::jsx_tag_name(&e.opening_element.name)))
            }
            JSXAttributeValue::Fragment(_) => JsxPropValue::Expr("<></>".to_string()),
        }
    }

    fn jsx_expr_to_prop(&self, expr: &JSXExpression) -> JsxPropValue {
        // JSXExpression inherits Expression variants + EmptyExpression
        if let Some(e) = expr.as_expression() {
            return self.expr_to_prop(e);
        }
        JsxPropValue::Expr(String::new())
    }

    fn expr_to_prop(&self, expr: &Expression) -> JsxPropValue {
        match expr {
            Expression::StringLiteral(sl) => JsxPropValue::String(sl.value.to_string()),
            Expression::NumericLiteral(n) => JsxPropValue::String(n.value.to_string()),
            Expression::BooleanLiteral(b) => JsxPropValue::String(b.value.to_string()),
            Expression::Identifier(id) => JsxPropValue::Ref(id.name.to_string()),
            Expression::ArrowFunctionExpression(a) => {
                JsxPropValue::Fn(self.span_text(a.span).to_string())
            }
            Expression::FunctionExpression(f) => {
                JsxPropValue::Fn(self.span_text(f.span).to_string())
            }
            Expression::ObjectExpression(obj) => {
                let mut style = HashMap::new();
                for prop in &obj.properties {
                    let ObjectPropertyKind::ObjectProperty(p) = prop else { continue };
                    let key = match &p.key {
                        PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                        PropertyKey::StringLiteral(sl) => sl.value.to_string(),
                        _ => continue,
                    };
                    let kebab = camel_to_kebab(&key);
                    let val = match &p.value {
                        Expression::StringLiteral(sl) => StyleValue::Static(sl.value.to_string()),
                        Expression::NumericLiteral(n) => StyleValue::Static(format!("{}px", n.value)),
                        Expression::BooleanLiteral(b) => StyleValue::Static(b.value.to_string()),
                        Expression::UnaryExpression(ue) => {
                            if let Expression::NumericLiteral(n) = &ue.argument {
                                StyleValue::Static(format!("{}{}", ue.operator.as_str(), n.value))
                            } else {
                                StyleValue::Expr(self.span_text(ue.span).to_string())
                            }
                        }
                        other => StyleValue::Expr(self.span_text(other.span()).to_string()),
                    };
                    style.insert(kebab, val);
                }
                JsxPropValue::Style(style)
            }
            Expression::TemplateLiteral(tl) => {
                JsxPropValue::Template(self.span_text(tl.span).to_string())
            }
            other => JsxPropValue::Expr(self.span_text(other.span()).to_string()),
        }
    }

    fn jsx_props(&self, opening: &JSXOpeningElement) -> HashMap<String, JsxPropValue> {
        let mut props = HashMap::new();
        for item in &opening.attributes {
            if let Some(attr) = item.as_attribute() {
                let name = attr.name.get_identifier().name.to_string();
                let value = match &attr.value {
                    Some(v) => self.jsx_prop_value(v),
                    None => JsxPropValue::Bool,
                };
                props.insert(name, value);
            }
        }
        props
    }

    fn parse_jsx_element(&self, elem: &JSXElement) -> JsxNode {
        let tag = Self::jsx_tag_name(&elem.opening_element.name);
        let props = self.jsx_props(&elem.opening_element);
        let children = self.parse_jsx_children(&elem.children);
        let self_closing = elem.closing_element.is_none();
        let (line, col) = self.line_col(elem.span.start);
        JsxNode::Element { tag, props, children, self_closing, line, col }
    }

    fn parse_jsx_children(&self, children: &[JSXChild]) -> Vec<JsxNode> {
        let mut result = Vec::new();
        for child in children {
            match child {
                JSXChild::Text(txt) => {
                    if let Some(text) = normalize_jsx_text(txt.value.as_str()) {
                        result.push(JsxNode::Text(text));
                    }
                }
                JSXChild::Element(e) => result.push(self.parse_jsx_element(e)),
                JSXChild::Fragment(f) => {
                    let children = self.parse_jsx_children(&f.children);
                    let (line, col) = self.line_col(f.span.start);
                    result.push(JsxNode::Fragment { props: HashMap::new(), children, line, col });
                }
                JSXChild::ExpressionContainer(ec) => {
                    // Check JSXExpression variants
                    if let Some(expr) = ec.expression.as_expression() {
                        match expr {
                            Expression::LogicalExpression(logical) => {
                                if let Some(jsx) = self.try_jsx_from_expr(&logical.right) {
                                    let cond = self.span_text(logical.left.span()).to_string();
                                    if logical.operator == oxc_ast::ast::LogicalOperator::And {
                                        let (line, col) = self.line_col(ec.span.start);
                                        result.push(JsxNode::Conditional {
                                            condition: cond,
                                            then_branch: vec![jsx],
                                            else_branch: vec![],
                                            line, col
                                        });
                                        continue;
                                    }
                                }
                                result.push(JsxNode::Expression(self.span_text(ec.span).trim_matches(|c| c == '{' || c == '}').trim().to_string()));
                            }
                            Expression::ConditionalExpression(cond) => {
                                let condition = self.span_text(cond.test.span()).to_string();
                                let then_branch = self.try_jsx_from_expr(&cond.consequent).map(|n| vec![n]).unwrap_or_default();
                                let else_branch = self.try_jsx_from_expr(&cond.alternate).map(|n| vec![n]).unwrap_or_default();
                                if !then_branch.is_empty() || !else_branch.is_empty() {
                                    let (line, col) = self.line_col(ec.span.start);
                                    result.push(JsxNode::Conditional { condition, then_branch, else_branch, line, col });
                                } else {
                                    result.push(JsxNode::Expression(self.span_text(ec.span).trim_matches(|c| c == '{' || c == '}').trim().to_string()));
                                }
                            }
                            Expression::CallExpression(call) => {
                                if let Some(list) = self.try_list_from_map(call) {
                                    result.push(list);
                                } else {
                                    result.push(JsxNode::Expression(self.span_text(ec.span).trim_matches(|c| c == '{' || c == '}').trim().to_string()));
                                }
                            }
                            _ => {
                                if let Some(jsx) = self.try_jsx_from_expr(expr) {
                                    result.push(jsx);
                                } else {
                                    let raw = self.span_text(ec.span).trim_matches(|c| c == '{' || c == '}').trim().to_string();
                                    // Skip JSX comments like {/* ... */}
                                    if raw.starts_with("/*") { continue; }
                                    result.push(JsxNode::Expression(raw));
                                }
                            }
                        }
                    } else {
                        // EmptyExpression
                        continue;
                    }
                }
                JSXChild::Spread(sc) => {
                    result.push(JsxNode::Expression(format!("...{}", self.span_text(sc.span))));
                }
            }
        }
        result
    }

    fn try_jsx_from_expr(&self, expr: &Expression) -> Option<JsxNode> {
        match expr {
            Expression::JSXElement(e) => Some(self.parse_jsx_element(e)),
            Expression::JSXFragment(f) => {
                let children = self.parse_jsx_children(&f.children);
                let (line, col) = self.line_col(f.span.start);
                Some(JsxNode::Fragment { props: HashMap::new(), children, line, col })
            }
            Expression::ParenthesizedExpression(pe) => self.try_jsx_from_expr(&pe.expression),
            _ => None,
        }
    }

    fn try_list_from_map(&self, call: &CallExpression) -> Option<JsxNode> {
        // Must be `arr.map(...)`
        let me = call.callee.as_member_expression()?;
        let MemberExpression::StaticMemberExpression(s) = me else { return None };
        if s.property.name.as_str() != "map" { return None; }
        let array_src = self.span_text(s.object.span()).to_string();

        // First arg must be arrow function
        let arg = call.arguments.first()?;
        let expr = arg.as_expression()?;
        let Expression::ArrowFunctionExpression(arrow) = expr else { return None };

        // Params
        let params: Vec<String> = arrow.params.items.iter().filter_map(|p| {
            if let BindingPattern::BindingIdentifier(id) = &p.pattern {
                Some(id.name.to_string())
            } else { None }
        }).collect();

        // Body -> JSX
        let Some(body_expr) = arrow.body.as_expression() else { return None };
        // Unwrap parenthesized etc. inside expression
        let template = self.try_jsx_from_expr(body_expr)?;

        // Extract key
        let mut key_expr = String::new();
        if let JsxNode::Element { props, .. } = &template {
            if let Some(v) = props.get("key") {
                key_expr = match v {
                    JsxPropValue::Expr(s) | JsxPropValue::Ref(s) | JsxPropValue::String(s) => s.clone(),
                    JsxPropValue::Fn(s) | JsxPropValue::Template(s) => s.clone(),
                    _ => String::new(),
                };
            }
        }

        let (line, col) = self.line_col(call.span.start);
        Some(JsxNode::List {
            array_expr: array_src,
            item_param: params.first().cloned().unwrap_or_else(|| "item".to_string()),
            index_param: params.get(1).cloned().unwrap_or_default(),
            key_expr,
            item_template: Box::new(template),
            line, col
        })
    }

    // ── Import / windowConfig / component extraction ────────────────────────

    fn extract_import(&mut self, decl: &ImportDeclaration) {
        let source = decl.source.value.to_string();
        let specifiers: Vec<String> = decl.specifiers.as_ref().map(|v| {
            v.iter().filter_map(|s| {
                if let ImportDeclarationSpecifier::ImportSpecifier(spec) = s {
                    Some(spec.local.name.to_string())
                } else { None }
            }).collect()
        }).unwrap_or_default();

        if source.starts_with("http://") || source.starts_with("https://") {
            self.imports.push(MxImport { kind: MxImportKind::CssUrl { url: source }, style: "import".to_string() });
        } else if source.ends_with(".css") {
            self.imports.push(MxImport { kind: MxImportKind::CssLocal { path: source }, style: "import".to_string() });
        } else if source.ends_with(".cpp") || source.ends_with(".cc") || source.ends_with(".cxx") || source.ends_with(".h") || source.ends_with(".hpp") {
            self.cpp_imports.push(CppImport { path: source.clone(), specifiers: specifiers.clone() });
            self.imports.push(MxImport { kind: MxImportKind::CppLocal { path: source, specifiers }, style: "import".to_string() });
        } else {
            self.imports.push(MxImport { kind: MxImportKind::Component { path: source, specifiers }, style: "import".to_string() });
        }
    }

    fn extract_window_config_from_decl(&mut self, var_decl: &VariableDeclaration) {
        for decl in &var_decl.declarations {
            let Some(name) = binding_ident_name(&decl.id) else { continue };
            if name != "windowConfig" { continue; }
            let Some(Expression::ObjectExpression(obj)) = &decl.init else { continue };
            let mut config = WindowConfig {
                title: String::new(), width: 800, height: 600,
                max_width: None, max_height: None, min_width: None, min_height: None,
                visible: true, modal: false,
            };
            for prop in &obj.properties {
                let ObjectPropertyKind::ObjectProperty(p) = prop else { continue };
                let key = match &p.key {
                    PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                    _ => continue,
                };
                match key.as_str() {
                    "title" => if let Some(v) = extract_string_lit(&p.value) { config.title = v; }
                    "width" => if let Expression::NumericLiteral(n) = &p.value { config.width = n.value as u32; }
                    "height" => if let Expression::NumericLiteral(n) = &p.value { config.height = n.value as u32; }
                    "maxWidth" => if let Expression::NumericLiteral(n) = &p.value { config.max_width = Some(n.value as u32); }
                    "maxHeight" => if let Expression::NumericLiteral(n) = &p.value { config.max_height = Some(n.value as u32); }
                    "minWidth" => if let Expression::NumericLiteral(n) = &p.value { config.min_width = Some(n.value as u32); }
                    "minHeight" => if let Expression::NumericLiteral(n) = &p.value { config.min_height = Some(n.value as u32); }
                    "visible" => if let Expression::BooleanLiteral(b) = &p.value { config.visible = b.value; }
                    "modal" => if let Expression::BooleanLiteral(b) = &p.value { config.modal = b.value; }
                    _ => {}
                }
            }
            self.window_config = Some(config);
        }
    }

    fn extract_component(&mut self, func: &Function, exported: bool) {
        let Some(id) = &func.id else { return };
        let name = id.name.to_string();
        let Some(body) = &func.body else { return };

        let has_jsx = body.statements.iter().any(|stmt| {
            if let Statement::ReturnStatement(ret) = stmt {
                if let Some(arg) = &ret.argument {
                    return self.expr_is_jsx(arg);
                }
            }
            false
        });
        if !has_jsx { return; }

        // Params: look for object destructuring in first param
        let params: Vec<String> = func.params.items.iter().flat_map(|p| {
            if let BindingPattern::ObjectPattern(obj) = &p.pattern {
                obj.properties.iter().filter_map(|prop| {
                    if let BindingPattern::BindingIdentifier(id) = &prop.value {
                        Some(id.name.to_string())
                    } else { None }
                }).collect::<Vec<_>>()
            } else { vec![] }
        }).collect();

        let state_vars = self.state_vars_from_body(body);
        let effects = self.effects_from_body(body);
        let inner_functions = self.inner_funcs_from_body(body);
        let consts = self.consts_from_body(body);
        let console_logs = self.logs_from_body(body);
        let jsx = self.jsx_from_body(body).unwrap_or(JsxNode::Fragment { props: HashMap::new(), children: vec![], line: 0, col: 0 });

        self.components.push(MxComponent { name, exported, params, jsx, state_vars, effects, inner_functions, consts, console_logs });
    }

    fn expr_is_jsx(&self, expr: &Expression) -> bool {
        matches!(expr, Expression::JSXElement(_) | Expression::JSXFragment(_) | Expression::ParenthesizedExpression(_))
    }

    fn jsx_from_body(&self, body: &FunctionBody) -> Option<JsxNode> {
        for stmt in &body.statements {
            if let Statement::ReturnStatement(ret) = stmt {
                if let Some(arg) = &ret.argument {
                    if let Some(n) = self.try_jsx_from_expr(arg) {
                        return Some(n);
                    }
                    if let Expression::ParenthesizedExpression(pe) = arg {
                        if let Some(n) = self.try_jsx_from_expr(&pe.expression) {
                            return Some(n);
                        }
                    }
                }
            }
        }
        None
    }

    fn state_vars_from_body(&self, body: &FunctionBody) -> Vec<StateVar> {
        let mut vars = Vec::new();
        for stmt in &body.statements {
            let Statement::VariableDeclaration(vd) = stmt else { continue };
            for decl in &vd.declarations {
                let Some(elements) = binding_array_elements(&decl.id) else { continue };
                let Some(Expression::CallExpression(call)) = &decl.init else { continue };
                if !is_member_expr_callee(&call.callee, "morphState", "") && !matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphState") {
                    continue;
                }
                let names: Vec<String> = elements.into_iter().flatten().collect();
                if names.len() < 2 { continue; }
                let init = call.arguments.first()
                    .and_then(|a| a.as_expression())
                    .map(|e| self.span_text(e.span()).to_string())
                    .unwrap_or_else(|| "0".to_string());
                vars.push(StateVar { getter: names[0].clone(), setter: names[1].clone(), init });
            }
        }
        vars
    }

    fn effects_from_body(&self, body: &FunctionBody) -> Vec<MxEffect> {
        let mut effects = Vec::new();
        for stmt in &body.statements {
            self.find_effect_in_stmt(stmt, &mut effects);
        }
        effects
    }

    fn find_effect_in_stmt(&self, stmt: &Statement, out: &mut Vec<MxEffect>) {
        match stmt {
            Statement::ExpressionStatement(es) => self.find_effect_in_expr(&es.expression, out, true),
            Statement::VariableDeclaration(vd) => {
                for decl in &vd.declarations {
                    if let Some(init) = &decl.init { self.find_effect_in_expr(init, out, true); }
                }
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument { self.find_effect_in_expr(arg, out, false); }
            }
            _ => {}
        }
    }

    fn find_effect_in_expr(&self, expr: &Expression, out: &mut Vec<MxEffect>, recurse: bool) {
        if let Expression::CallExpression(call) = expr {
            let is_target = matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphEffect");
            if is_target {
                let callback = call.arguments.first().and_then(|a| a.as_expression()).map(|e| self.span_text(e.span()).to_string()).unwrap_or_default();
                let deps = call.arguments.get(1).and_then(|a| a.as_expression()).map(|e| self.span_text(e.span()).to_string()).unwrap_or_default();
                out.push(MxEffect { callback, deps });
            }
        }
        if let Expression::ArrowFunctionExpression(arrow) = expr {
            if recurse {
                if let Some(b) = arrow.body.as_function_body() {
                    for stmt in &b.statements { self.find_effect_in_stmt(stmt, out); }
                } else if let Some(e) = arrow.body.as_expression() {
                    self.find_effect_in_expr(e, out, true);
                }
            }
        }
        if let Expression::ParenthesizedExpression(pe) = expr {
            self.find_effect_in_expr(&pe.expression, out, recurse);
        }
    }

    fn inner_funcs_from_body(&self, body: &FunctionBody) -> Vec<InnerFunction> {
        let mut funcs = Vec::new();
        for stmt in &body.statements {
            match stmt {
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        funcs.push(InnerFunction { name: id.name.to_string(), source: self.span_text(f.span).to_string() });
                    }
                }
                Statement::VariableDeclaration(vd) => {
                    for decl in &vd.declarations {
                        let Some(name) = binding_ident_name(&decl.id) else { continue };
                        let is_fn = matches!(decl.init.as_ref(), Some(Expression::ArrowFunctionExpression(_)) | Some(Expression::FunctionExpression(_)));
                        if is_fn {
                            funcs.push(InnerFunction { name, source: self.span_text(decl.span).to_string() });
                        }
                    }
                }
                _ => {}
            }
        }
        funcs
    }

    fn consts_from_body(&self, body: &FunctionBody) -> Vec<ComponentConst> {
        let mut consts = Vec::new();
        for stmt in &body.statements {
            let Statement::VariableDeclaration(vd) = stmt else { continue };
            for decl in &vd.declarations {
                let Some(name) = binding_ident_name(&decl.id) else { continue };
                // skip morphState, arrow fns, fn exprs
                let skip = match decl.init.as_ref() {
                    Some(Expression::CallExpression(call)) => matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphState"),
                    Some(Expression::ArrowFunctionExpression(_)) | Some(Expression::FunctionExpression(_)) => true,
                    _ => false,
                };
                if skip || name.is_empty() { continue; }
                if let Some(init) = &decl.init {
                    consts.push(ComponentConst { name, rhs: self.span_text(init.span()).to_string() });
                }
            }
        }
        consts
    }

    fn logs_from_body(&self, body: &FunctionBody) -> Vec<String> {
        let mut logs = Vec::new();
        for stmt in &body.statements {
            self.find_logs_in_stmt(stmt, &mut logs, true);
        }
        logs
    }

    fn find_logs_in_stmt(&self, stmt: &Statement, logs: &mut Vec<String>, top: bool) {
        match stmt {
            Statement::ExpressionStatement(es) => self.find_logs_in_expr(&es.expression, logs, top),
            Statement::ReturnStatement(ret) => if let Some(arg) = &ret.argument { self.find_logs_in_expr(arg, logs, false); },
            Statement::VariableDeclaration(vd) => for decl in &vd.declarations { if let Some(init) = &decl.init { self.find_logs_in_expr(init, logs, false); } },
            _ => {}
        }
    }

    fn find_logs_in_expr(&self, expr: &Expression, logs: &mut Vec<String>, top: bool) {
        if let Expression::CallExpression(call) = expr {
            if let Some(me) = call.callee.as_member_expression() {
                if let MemberExpression::StaticMemberExpression(s) = me {
                    if let Expression::Identifier(obj) = &s.object {
                        if obj.name.as_str() == "console" && s.property.name.as_str() == "log" {
                            if let Some(arg) = call.arguments.first().and_then(|a| a.as_expression()) {
                                if let Some(s) = extract_string_lit(arg) { logs.push(s); }
                            }
                        }
                    }
                }
            }
            if !top { return; }
        }
        if matches!(expr, Expression::ArrowFunctionExpression(_)) { return; }
        if let Expression::ParenthesizedExpression(pe) = expr { self.find_logs_in_expr(&pe.expression, logs, top); }
    }

    // ── CSS.load detection ─────────────────────────────────────────────────

    fn check_css_load(&self, call: &CallExpression, out: &mut Vec<MxImport>) {
        let is_css = is_member_expr_callee(&call.callee, "CSS", "load");
        if !is_css { return; }
        if let Some(arg) = call.arguments.first().and_then(|a| a.as_expression()) {
            if let Some(url) = extract_string_lit(arg) {
                if url.starts_with("http://") || url.starts_with("https://") {
                    out.push(MxImport { kind: MxImportKind::CssUrl { url }, style: "css_load".to_string() });
                } else {
                    out.push(MxImport { kind: MxImportKind::CssLocal { path: url }, style: "css_load".to_string() });
                }
            }
        }
    }

    fn walk_css_load_stmts(&self, stmts: &[Statement], out: &mut Vec<MxImport>) {
        for stmt in stmts {
            match stmt {
                Statement::ExpressionStatement(es) => {
                    if let Expression::CallExpression(call) = &es.expression { self.check_css_load(call, out); }
                }
                _ => {}
            }
        }
    }
}

impl<'a, 'src> Visit<'a> for MxWalker<'src> {
    fn visit_program(&mut self, program: &Program<'a>) {
        // Phase 1: imports and windowConfig
        for stmt in &program.body {
            match stmt {
                Statement::ImportDeclaration(d) => self.extract_import(d),
                Statement::ExportDeclaration(d) => {
                    if let Declaration::VariableDeclaration(vd) = &d.declaration {
                        self.extract_window_config_from_decl(vd);
                    }
                }
                _ => {}
            }
        }
        // CSS.load calls
        let mut css = Vec::new();
        self.walk_css_load_stmts(&program.body, &mut css);
        self.imports.extend(css);

        // Phase 2: components
        for stmt in &program.body {
            match stmt {
                Statement::ExportDefaultDeclaration(d) => {
                    match &d.declaration {
                        ExportDefaultDeclarationKind::FunctionDeclaration(f) => self.extract_component(f, true),
                        _ => {}
                    }
                }
                Statement::ExportDeclaration(d) => {
                    if let Declaration::FunctionDeclaration(f) = &d.declaration { self.extract_component(f, true); }
                }
                Statement::FunctionDeclaration(f) => {
                    // A function that returns JSX is a component; otherwise it's a
                    // module-level helper function (e.g. `compute`), transpiled into
                    // premain like Python's `function_declarations`.
                    if f.body.as_ref().map(|b| b.statements.iter().any(|s| if let Statement::ReturnStatement(ret) = s {
                        ret.argument.as_ref().map(|a| self.expr_is_jsx(a)).unwrap_or(false)
                    } else { false })).unwrap_or(false) {
                        self.extract_component(f, false);
                    } else if let Some(id) = &f.id {
                        self.function_declarations.push(InnerFunction {
                            name: id.name.to_string(),
                            source: self.span_text(f.span).to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        // Phase 3: global vars (non-component)
        for stmt in &program.body {
            if matches!(stmt, Statement::ImportDeclaration(_) | Statement::ExportDeclaration(_) | Statement::ExportDefaultDeclaration(_) | Statement::FunctionDeclaration(_)) { continue; }
            if let Statement::VariableDeclaration(vd) = stmt {
                let is_state = vd.declarations.iter().any(|d| {
                    if let Some(Expression::CallExpression(call)) = &d.init {
                        matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphState")
                    } else { false }
                });
                if !is_state {
                    self.global_vars.push(self.span_text(vd.span).to_string());
                }
            }
        }
    }
}
