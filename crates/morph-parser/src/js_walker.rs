use std::collections::HashMap;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;

use super::ast_types::*;

// ── Helpers ────────────────────────────────────────────────────────────────

fn camel_to_kebab(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('-');
        }
        result.push(c.to_ascii_lowercase());
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
    let lines: Vec<&str> = raw.lines().map(str::trim).collect();
    let mut first = -1i32;
    let mut last = -1i32;
    for (i, ln) in lines.iter().enumerate() {
        if !ln.is_empty() {
            if first < 0 {
                first = i32::try_from(i).unwrap_or(i32::MAX);
            }
            last = i32::try_from(i).unwrap_or(i32::MAX);
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn member_expr_obj_str(expr: &Expression) -> String {
    match expr {
        Expression::Identifier(id) => id.name.to_string(),
        _ => expr.span().source_text("").to_string(),
    }
}

fn is_member_expr_callee(expr: &Expression, obj: &str, prop: &str) -> bool {
    // Check for `obj.prop` via StaticMemberExpression / Chain etc.
    if let Some(MemberExpression::StaticMemberExpression(s)) = expr.as_member_expression() {
        if let Expression::Identifier(id) = &s.object {
            return id.name.as_str() == obj && s.property.name.as_str() == prop;
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

impl<'src> MxWalker<'src> {
    /// Name behind a `ModuleExportName` (`foo`, `foo` in `as foo`, or a
    /// string-literal export name).
    fn export_name(name: &ModuleExportName) -> String {
        match name {
            ModuleExportName::IdentifierName(id) => id.name.to_string(),
            ModuleExportName::IdentifierReference(r) => r.name.to_string(),
            ModuleExportName::StringLiteral(lit) => lit.value.to_string(),
        }
    }
}

fn binding_array_elements(bp: &BindingPattern) -> Option<Vec<Option<String>>> {
    if let BindingPattern::ArrayPattern(arr) = bp {
        Some(
            arr.elements.iter().map(|e| e.as_ref().and_then(|bp| binding_ident_name(bp))).collect(),
        )
    } else {
        None
    }
}

fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offs = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            offs.push(i + 1);
        }
    }
    offs
}

#[allow(dead_code)]
fn offset_to_line_col(source: &str, offset: u32) -> (usize, usize) {
    let offset = offset as usize;
    let mut line = 1usize;
    let mut last = 0usize;
    for (i, ch) in source.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            last = i + 1;
        }
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

/// Parse a TS object-type annotation into declared props.
/// Purely textual; nested regions are respected, unparseable input is
/// skipped.
fn parse_props_annotation(ann: &str) -> Vec<ComponentProp> {
    let mut s = ann.trim();
    if let Some(rest) = s.strip_prefix(':') {
        s = rest.trim();
    }
    if s.starts_with('{') {
        let bytes = s.as_bytes();
        let mut depth = 0i32;
        let mut i = 0usize;
        let mut in_str: Option<u8> = None;
        let mut end = None;
        while i < bytes.len() {
            let b = bytes[i];
            if let Some(q) = in_str {
                if b == b'\\' {
                    i += 2;
                    continue;
                }
                if b == q {
                    in_str = None;
                }
                i += 1;
                continue;
            }
            match b {
                b'"' | b'\'' | b'`' => in_str = Some(b),
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i);
                        break;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        if let Some(e) = end {
            s = s[1..e].trim();
        } else {
            return Vec::new();
        }
    }
    let mut props = Vec::new();
    for member in split_top_level_props(s) {
        let m = member.trim();
        if m.is_empty() {
            continue;
        }
        let m = m.strip_prefix("readonly ").map_or(m, str::trim);
        if m.starts_with('[') || m.starts_with("...") {
            continue;
        }
        let name_end = m
            .char_indices()
            // Identifier chars: `_`, `$` or alphanumeric at any position.
            // (The old `*i == 0 && ...` prefix was dead code: the trailing
            // alternatives already accepted `_`/`$` everywhere, so the
            // condition was equivalent to this for every index.)
            .take_while(|(_i, c)| *c == '_' || *c == '$' || c.is_alphanumeric())
            .map(|(i, c)| i + c.len_utf8())
            .last()
            .unwrap_or(0);
        if name_end == 0 {
            continue;
        }
        let name = m[..name_end].to_string();
        let rest = m[name_end..].trim_start();
        let (optional, rest) =
            rest.strip_prefix('?').map_or((false, rest), |r| (true, r.trim_start()));
        let Some(ty) = rest.strip_prefix(':') else { continue };
        props.push(ComponentProp { name, prop_type: ty.trim().to_string(), optional });
    }
    props
}

fn split_top_level_props(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut in_str: Option<u8> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = in_str {
            if b == b'\\' {
                i += 2;
                continue;
            }
            if b == q {
                in_str = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'"' | b'\'' | b'`' => in_str = Some(b),
            b'=' if i + 1 < bytes.len() && bytes[i + 1] == b'>' => {
                i += 1;
            }
            b'<' | b'(' | b'[' | b'{' => depth += 1,
            b'>' | b')' | b']' | b'}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b',' | b';' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&s[start..]);
    out
}

// ── Walker ─────────────────────────────────────────────────────────────────

pub struct MxWalker<'src> {
    source: &'src str,
    line_offsets: Vec<usize>,
    pub imports: Vec<MxImport>,
    pub window_config: Option<WindowConfig>,
    pub components: Vec<MxComponent>,
    pub shared_bindings: Vec<SharedBinding>,
    pub event_bindings: Vec<EventBinding>,
    pub state_vars: Vec<StateVar>,
    pub effects: Vec<MxEffect>,
    pub inner_functions: Vec<InnerFunction>,
    pub function_declarations: Vec<InnerFunction>,
    pub class_declarations: Vec<ClassDecl>,
    pub exported_vars: Vec<ExportedVar>,
    pub named_exports: Vec<(String, String)>,
    pub default_export: Option<String>,
    pub re_exports: Vec<ReExport>,
    pub global_vars: Vec<String>,
    pub console_logs: Vec<String>,
    pub extra_headers: Vec<String>,
    pub cpp_imports: Vec<CppImport>,
}

impl<'src> MxWalker<'src> {
    pub(crate) fn new(source: &'src str) -> Self {
        Self {
            source,
            line_offsets: build_line_offsets(source),
            imports: Vec::new(),
            window_config: None,
            components: Vec::new(),
            shared_bindings: Vec::new(),
            event_bindings: Vec::new(),
            state_vars: Vec::new(),
            effects: Vec::new(),
            inner_functions: Vec::new(),
            function_declarations: Vec::new(),
            class_declarations: Vec::new(),
            exported_vars: Vec::new(),
            named_exports: Vec::new(),
            default_export: None,
            re_exports: Vec::new(),
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
                        Expression::NumericLiteral(n) => {
                            StyleValue::Static(format!("{}px", n.value))
                        }
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
                let value =
                    attr.value.as_ref().map_or(JsxPropValue::Bool, |v| self.jsx_prop_value(v));
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
                    // (An `EmptyExpression` yields nothing, so there is no
                    // else branch — falling through continues the loop.)
                    if let Some(expr) = ec.expression.as_expression() {
                        match expr {
                            Expression::LogicalExpression(logical) => {
                                if let Some(jsx) = self.try_jsx_from_expr(&logical.right) {
                                    let cond = self.span_text(logical.left.span()).to_string();
                                    if logical.operator == LogicalOperator::And {
                                        let (line, col) = self.line_col(ec.span.start);
                                        result.push(JsxNode::Conditional {
                                            condition: cond,
                                            then_branch: vec![jsx],
                                            else_branch: vec![],
                                            line,
                                            col,
                                        });
                                        continue;
                                    }
                                }
                                result.push(JsxNode::Expression(
                                    self.span_text(ec.span)
                                        .trim_matches(|c| c == '{' || c == '}')
                                        .trim()
                                        .to_string(),
                                ));
                            }
                            Expression::ConditionalExpression(cond) => {
                                let condition = self.span_text(cond.test.span()).to_string();
                                let then_branch = self
                                    .try_jsx_from_expr(&cond.consequent)
                                    .map(|n| vec![n])
                                    .unwrap_or_default();
                                let else_branch = self
                                    .try_jsx_from_expr(&cond.alternate)
                                    .map(|n| vec![n])
                                    .unwrap_or_default();
                                if !then_branch.is_empty() || !else_branch.is_empty() {
                                    let (line, col) = self.line_col(ec.span.start);
                                    result.push(JsxNode::Conditional {
                                        condition,
                                        then_branch,
                                        else_branch,
                                        line,
                                        col,
                                    });
                                } else {
                                    result.push(JsxNode::Expression(
                                        self.span_text(ec.span)
                                            .trim_matches(|c| c == '{' || c == '}')
                                            .trim()
                                            .to_string(),
                                    ));
                                }
                            }
                            Expression::CallExpression(call) => {
                                if let Some(list) = self.try_list_from_map(call) {
                                    result.push(list);
                                } else {
                                    result.push(JsxNode::Expression(
                                        self.span_text(ec.span)
                                            .trim_matches(|c| c == '{' || c == '}')
                                            .trim()
                                            .to_string(),
                                    ));
                                }
                            }
                            _ => {
                                if let Some(jsx) = self.try_jsx_from_expr(expr) {
                                    result.push(jsx);
                                } else {
                                    let raw = self
                                        .span_text(ec.span)
                                        .trim_matches(|c| c == '{' || c == '}')
                                        .trim()
                                        .to_string();
                                    // Skip JSX comments like {/* ... */}
                                    if raw.starts_with("/*") {
                                        continue;
                                    }
                                    result.push(JsxNode::Expression(raw));
                                }
                            }
                        }
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
        if s.property.name.as_str() != "map" {
            return None;
        }
        let array_src = self.span_text(s.object.span()).to_string();

        // First arg must be arrow function
        let arg = call.arguments.first()?;
        let expr = arg.as_expression()?;
        let Expression::ArrowFunctionExpression(arrow) = expr else { return None };

        // Params
        let params: Vec<String> = arrow
            .params
            .items
            .iter()
            .filter_map(|p| {
                if let BindingPattern::BindingIdentifier(id) = &p.pattern {
                    Some(id.name.to_string())
                } else {
                    None
                }
            })
            .collect();

        // Body -> JSX
        let body_expr = arrow.body.as_expression()?;
        // Unwrap parenthesized etc. inside expression
        let template = self.try_jsx_from_expr(body_expr)?;

        // Extract key
        let mut key_expr = String::new();
        if let JsxNode::Element { props, .. } = &template {
            if let Some(v) = props.get("key") {
                key_expr = match v {
                    JsxPropValue::Expr(s) | JsxPropValue::Ref(s) | JsxPropValue::String(s) => {
                        s.clone()
                    }
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
            line,
            col,
        })
    }

    // ── Import / windowConfig / component extraction ────────────────────────

    fn extract_import(&mut self, decl: &ImportDeclaration) {
        let source = decl.source.value.to_string();
        let mut default: Option<String> = None;
        let mut specifiers: Vec<String> = Vec::new();
        if let Some(specs) = decl.specifiers.as_ref() {
            for s in specs {
                match s {
                    ImportDeclarationSpecifier::ImportSpecifier(spec) => {
                        specifiers.push(spec.local.name.to_string());
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(spec) => {
                        default = Some(spec.local.name.to_string());
                    }
                    // Member tags (`<NS.Hero/>`) are unsupported; keep the
                    // binding visible so the linter can point at it.
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(spec) => {
                        default = Some(spec.local.name.to_string());
                    }
                }
            }
        }

        // Case-insensitive extension check (import specifiers may use any
        // case, e.g. `./style.CSS`); `Path::extension` also keeps the
        // `.ts`-vs-`.tsx`-style distinctions exact here.
        let ext_is = |ext: &str| {
            std::path::Path::new(&source)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        };
        if source.starts_with("http://") || source.starts_with("https://") {
            self.imports.push(MxImport {
                kind: MxImportKind::CssUrl { url: source },
                style: "import".to_string(),
            });
        } else if ext_is("css") {
            self.imports.push(MxImport {
                kind: MxImportKind::CssLocal { path: source },
                style: "import".to_string(),
            });
        } else if ext_is("cpp") || ext_is("cc") || ext_is("cxx") || ext_is("h") || ext_is("hpp") {
            self.cpp_imports
                .push(CppImport { path: source.clone(), specifiers: specifiers.clone() });
            self.imports.push(MxImport {
                kind: MxImportKind::CppLocal { path: source, specifiers },
                style: "import".to_string(),
            });
        } else {
            self.imports.push(MxImport {
                kind: MxImportKind::Component { path: source, default, specifiers },
                style: "import".to_string(),
            });
        }
    }

    fn extract_window_config_from_decl(&mut self, var_decl: &VariableDeclaration) {
        for decl in &var_decl.declarations {
            let Some(name) = binding_ident_name(&decl.id) else { continue };
            if name != "windowConfig" {
                continue;
            }
            let Some(Expression::ObjectExpression(obj)) = &decl.init else { continue };
            let mut config = WindowConfig {
                title: String::new(),
                width: 800,
                height: 600,
                max_width: None,
                max_height: None,
                min_width: None,
                min_height: None,
                visible: true,
                modal: false,
            };
            for prop in &obj.properties {
                let ObjectPropertyKind::ObjectProperty(p) = prop else { continue };
                let key = match &p.key {
                    PropertyKey::StaticIdentifier(id) => id.name.to_string(),
                    _ => continue,
                };
                match key.as_str() {
                    "title" => {
                        if let Some(v) = extract_string_lit(&p.value) {
                            config.title = v;
                        }
                    }
                    "width" => {
                        if let Expression::NumericLiteral(n) = &p.value {
                            config.width = n.value as u32;
                        }
                    }
                    "height" => {
                        if let Expression::NumericLiteral(n) = &p.value {
                            config.height = n.value as u32;
                        }
                    }
                    "maxWidth" => {
                        if let Expression::NumericLiteral(n) = &p.value {
                            config.max_width = Some(n.value as u32);
                        }
                    }
                    "maxHeight" => {
                        if let Expression::NumericLiteral(n) = &p.value {
                            config.max_height = Some(n.value as u32);
                        }
                    }
                    "minWidth" => {
                        if let Expression::NumericLiteral(n) = &p.value {
                            config.min_width = Some(n.value as u32);
                        }
                    }
                    "minHeight" => {
                        if let Expression::NumericLiteral(n) = &p.value {
                            config.min_height = Some(n.value as u32);
                        }
                    }
                    "visible" => {
                        if let Expression::BooleanLiteral(b) = &p.value {
                            config.visible = b.value;
                        }
                    }
                    "modal" => {
                        if let Expression::BooleanLiteral(b) = &p.value {
                            config.modal = b.value;
                        }
                    }
                    _ => {}
                }
            }
            self.window_config = Some(config);
        }
    }

    fn extract_component(&mut self, func: &Function, exported: bool, is_default: bool) {
        let name = func.id.as_ref().map_or_else(
            || {
                if is_default {
                    "default".to_string()
                } else {
                    String::new()
                }
            },
            |id| id.name.to_string(),
        );
        if name.is_empty() {
            return;
        }
        let Some(body) = &func.body else { return };

        let has_jsx = body.statements.iter().any(|stmt| {
            if let Statement::ReturnStatement(ret) = stmt {
                if let Some(arg) = &ret.argument {
                    return self.expr_is_jsx(arg);
                }
            }
            false
        });
        if !has_jsx {
            return;
        }

        let (props_param, props) = self.component_props(&func.params);
        let params: Vec<String> = props.iter().map(|p| p.name.clone()).collect();

        let state_vars = self.state_vars_from_body(body);
        let effects = self.effects_from_body(body);
        let inner_functions = self.inner_funcs_from_body(body);
        let consts = self.consts_from_body(body);
        let console_logs = self.logs_from_body(body);
        let jsx = self.jsx_from_body(body).unwrap_or_else(|| JsxNode::Fragment {
            props: HashMap::new(),
            children: vec![],
            line: 0,
            col: 0,
        });

        let event_subs = self.event_subs_from_body(body);

        self.components.push(MxComponent {
            name,
            exported,
            event_subs,
            is_default,
            props_param,
            props,
            params,
            jsx,
            state_vars,
            effects,
            inner_functions,
            consts,
            console_logs,
        });
    }

    /// Claim a top-level declarator as an exported shared/event binding when
    /// its initializer calls `morphShared` / `morphEvent`.
    fn extract_binding_declarator(&mut self, decl: &VariableDeclarator) {
        let Some(Expression::CallExpression(call)) = &decl.init else { return };
        let callee_is =
            |n: &str| matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == n);
        if callee_is("morphShared") {
            let Some(elements) = binding_array_elements(&decl.id) else { return };
            let names: Vec<String> = elements.into_iter().flatten().collect();
            if names.len() < 2 {
                return;
            }
            let init = call
                .arguments
                .first()
                .and_then(|a| a.as_expression())
                .map(|e| self.span_text(e.span()).to_string())
                .unwrap_or_default();
            let (line, col) = self.line_col(call.span.start);
            self.shared_bindings.push(SharedBinding {
                getter: names[0].clone(),
                setter: names[1].clone(),
                type_arg: self.call_type_arg(call),
                init,
                line,
                col,
            });
        } else if callee_is("morphEvent") {
            let Some(name) = binding_ident_name(&decl.id) else { return };
            let (line, col) = self.line_col(call.span.start);
            self.event_bindings.push(EventBinding {
                name,
                type_arg: self.call_type_arg(call),
                line,
                col,
            });
        }
    }

    /// Extract a component from an arrow function. Non-JSX arrows are left
    /// alone for the global-vars pass.
    fn extract_arrow_component(
        &mut self,
        name: String,
        arrow: &ArrowFunctionExpression,
        exported: bool,
        is_default: bool,
    ) {
        if name.is_empty() {
            return;
        }
        let jsx = if let Some(expr) = arrow.body.as_expression() {
            match self.try_jsx_from_expr(expr) {
                Some(n) => n,
                None => return,
            }
        } else if let Some(body) = arrow.body.as_function_body() {
            let has_jsx = body.statements.iter().any(|stmt| {
                if let Statement::ReturnStatement(ret) = stmt {
                    if let Some(arg) = &ret.argument {
                        return self.expr_is_jsx(arg);
                    }
                }
                false
            });
            if !has_jsx {
                return;
            }
            match self.jsx_from_stmts(&body.statements) {
                Some(n) => n,
                None => return,
            }
        } else {
            return;
        };

        let (props_param, props) = self.component_props(&arrow.params);
        let params: Vec<String> = props.iter().map(|p| p.name.clone()).collect();

        let (state_vars, event_subs, effects, inner_functions, consts, console_logs) =
            arrow.body.as_function_body().map_or_else(
                || (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new()),
                |body| {
                    (
                        self.state_vars_from_body(body),
                        self.event_subs_from_body(body),
                        self.effects_from_body(body),
                        self.inner_funcs_from_body(body),
                        self.consts_from_body(body),
                        self.logs_from_body(body),
                    )
                },
            );

        self.components.push(MxComponent {
            name,
            exported,
            event_subs,
            is_default,
            props_param,
            props,
            params,
            jsx,
            state_vars,
            effects,
            inner_functions,
            consts,
            console_logs,
        });
    }

    /// Claim a variable declarator as an arrow-function component when it
    /// yields JSX.
    fn try_extract_arrow_declarator(
        &mut self,
        decl: &VariableDeclarator,
        exported: bool,
        is_default: bool,
    ) {
        let Some(name) = binding_ident_name(&decl.id) else { return };
        let Some(Expression::ArrowFunctionExpression(arrow)) = &decl.init else { return };
        self.extract_arrow_component(name, arrow, exported, is_default);
    }

    /// Declared props from a component's first parameter.
    fn component_props(&self, params: &FormalParameters) -> (String, Vec<ComponentProp>) {
        let Some(first) = params.items.first() else {
            return (String::new(), Vec::new());
        };
        let ann_src = first.type_annotation.as_ref().map(|t| self.span_text(t.span).to_string());
        match &first.pattern {
            BindingPattern::BindingIdentifier(id) => {
                let name = id.name.to_string();
                let props = ann_src.map(|s| parse_props_annotation(&s)).unwrap_or_default();
                (name, props)
            }
            BindingPattern::ObjectPattern(obj) => {
                let pattern_names: Vec<String> = obj
                    .properties
                    .iter()
                    .filter_map(|prop| {
                        if let BindingPattern::BindingIdentifier(id) = &prop.value {
                            Some(id.name.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();
                let ann_props = ann_src.map(|s| parse_props_annotation(&s)).unwrap_or_default();
                let mut props: Vec<ComponentProp> = pattern_names
                    .iter()
                    .map(|n| {
                        ann_props.iter().find(|a| &a.name == n).cloned().unwrap_or(ComponentProp {
                            name: n.clone(),
                            prop_type: String::new(),
                            optional: false,
                        })
                    })
                    .collect();
                for a in ann_props {
                    if !props.iter().any(|p| p.name == a.name) {
                        props.push(a);
                    }
                }
                (String::new(), props)
            }
            _ => (String::new(), Vec::new()),
        }
    }

    const fn expr_is_jsx(&self, expr: &Expression) -> bool {
        matches!(
            expr,
            Expression::JSXElement(_)
                | Expression::JSXFragment(_)
                | Expression::ParenthesizedExpression(_)
        )
    }

    fn jsx_from_body(&self, body: &FunctionBody) -> Option<JsxNode> {
        self.jsx_from_stmts(&body.statements)
    }

    fn jsx_from_stmts(&self, statements: &[Statement]) -> Option<JsxNode> {
        for stmt in statements {
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

    /// Raw text of the first type argument on a call like `morphState<number>`.
    fn call_type_arg(&self, call: &CallExpression) -> Option<String> {
        let ta = call.type_arguments.as_ref()?;
        let t = ta.params.first()?;
        Some(self.span_text(t.span()).to_string())
    }

    /// `const [value, setValue] = morphState<T>(init)` declarations inside a
    /// component body.
    fn state_vars_from_body(&self, body: &FunctionBody) -> Vec<StateVar> {
        let mut vars = Vec::new();
        for stmt in &body.statements {
            let Statement::VariableDeclaration(vd) = stmt else { continue };
            for decl in &vd.declarations {
                let Some(elements) = binding_array_elements(&decl.id) else { continue };
                let Some(Expression::CallExpression(call)) = &decl.init else { continue };
                if !is_member_expr_callee(&call.callee, "morphState", "")
                    && !matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphState")
                {
                    continue;
                }
                let names: Vec<String> = elements.into_iter().flatten().collect();
                if names.len() < 2 {
                    continue;
                }
                let init = call
                    .arguments
                    .first()
                    .and_then(|a| a.as_expression())
                    .map_or_else(|| "0".to_string(), |e| self.span_text(e.span()).to_string());
                vars.push(StateVar {
                    getter: names[0].clone(),
                    setter: names[1].clone(),
                    init,
                    type_arg: self.call_type_arg(call),
                });
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
            Statement::ExpressionStatement(es) => {
                self.find_effect_in_expr(&es.expression, out, true);
            }
            Statement::VariableDeclaration(vd) => {
                for decl in &vd.declarations {
                    if let Some(init) = &decl.init {
                        self.find_effect_in_expr(init, out, true);
                    }
                }
            }
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.find_effect_in_expr(arg, out, false);
                }
            }
            _ => {}
        }
    }

    fn find_effect_in_expr(&self, expr: &Expression, out: &mut Vec<MxEffect>, recurse: bool) {
        if let Expression::CallExpression(call) = expr {
            let is_target = matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphEffect");
            if is_target {
                let callback = call
                    .arguments
                    .first()
                    .and_then(|a| a.as_expression())
                    .map(|e| self.span_text(e.span()).to_string())
                    .unwrap_or_default();
                let deps = call
                    .arguments
                    .get(1)
                    .and_then(|a| a.as_expression())
                    .map(|e| self.span_text(e.span()).to_string())
                    .unwrap_or_default();
                out.push(MxEffect { callback, deps });
            }
        }
        if let Expression::ArrowFunctionExpression(arrow) = expr {
            if recurse {
                if let Some(b) = arrow.body.as_function_body() {
                    for stmt in &b.statements {
                        self.find_effect_in_stmt(stmt, out);
                    }
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
                        funcs.push(InnerFunction {
                            name: id.name.to_string(),
                            source: self.span_text(f.span).to_string(),
                        });
                    }
                }
                Statement::VariableDeclaration(vd) => {
                    for decl in &vd.declarations {
                        let Some(name) = binding_ident_name(&decl.id) else { continue };
                        let is_fn = matches!(
                            decl.init.as_ref(),
                            Some(
                                Expression::ArrowFunctionExpression(_)
                                    | Expression::FunctionExpression(_)
                            )
                        );
                        if is_fn {
                            funcs.push(InnerFunction {
                                name,
                                source: self.span_text(decl.span).to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        funcs
    }

    /// Statement-level `<event>.on(handler)` subscriptions inside a component
    /// body. The receiver must be a bare identifier (an event binding name).
    fn event_subs_from_body(&self, body: &FunctionBody) -> Vec<EventSub> {
        let mut subs = Vec::new();
        for stmt in &body.statements {
            let Statement::ExpressionStatement(es) = stmt else { continue };
            let Expression::CallExpression(call) = &es.expression else { continue };
            // Match `<event>.on(handler)` where object is a bare identifier.
            let (obj_name, prop_name) = match call.callee.as_member_expression() {
                Some(MemberExpression::StaticMemberExpression(s)) => {
                    let name = match &s.object {
                        Expression::Identifier(id) => id.name.to_string(),
                        _ => continue,
                    };
                    (name, s.property.name.to_string())
                }
                _ => continue,
            };
            if prop_name != "on" || obj_name.is_empty() {
                continue;
            }
            let handler = call
                .arguments
                .first()
                .and_then(|a| a.as_expression())
                .map(|e| self.span_text(e.span()).to_string());
            let Some(handler) = handler else { continue };
            let (line, col) = self.line_col(call.span.start);
            subs.push(EventSub { event: obj_name, handler, line, col });
        }
        subs
    }

    fn consts_from_body(&self, body: &FunctionBody) -> Vec<ComponentConst> {
        let mut consts = Vec::new();
        for stmt in &body.statements {
            let Statement::VariableDeclaration(vd) = stmt else { continue };
            for decl in &vd.declarations {
                let Some(name) = binding_ident_name(&decl.id) else { continue };
                let skip = match decl.init.as_ref() {
                    Some(Expression::CallExpression(call)) => {
                        matches!(&call.callee, Expression::Identifier(id) if {
                            let n = id.name.as_str();
                            n == "morphState"
                                || n == "morphShared"
                                || n == "morphEvent"
                                || n == "morphOn"
                                || n == "morphEmit"
                        })
                    }
                    Some(
                        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_),
                    ) => true,
                    _ => false,
                };
                if skip || name.is_empty() {
                    continue;
                }
                if let Some(init) = &decl.init {
                    consts.push(ComponentConst {
                        name,
                        rhs: self.span_text(init.span()).to_string(),
                    });
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
            Statement::ReturnStatement(ret) => {
                if let Some(arg) = &ret.argument {
                    self.find_logs_in_expr(arg, logs, false);
                }
            }
            Statement::VariableDeclaration(vd) => {
                for decl in &vd.declarations {
                    if let Some(init) = &decl.init {
                        self.find_logs_in_expr(init, logs, false);
                    }
                }
            }
            _ => {}
        }
    }

    fn find_logs_in_expr(&self, expr: &Expression, logs: &mut Vec<String>, top: bool) {
        if let Expression::CallExpression(call) = expr {
            if let Some(me) = call.callee.as_member_expression() {
                if let MemberExpression::StaticMemberExpression(s) = me {
                    if let Expression::Identifier(obj) = &s.object {
                        if obj.name.as_str() == "console" && s.property.name.as_str() == "log" {
                            if let Some(arg) =
                                call.arguments.first().and_then(|a| a.as_expression())
                            {
                                if let Some(s) = extract_string_lit(arg) {
                                    logs.push(s);
                                }
                            }
                        }
                    }
                }
            }
            if !top {
                return;
            }
        }
        if matches!(expr, Expression::ArrowFunctionExpression(_)) {
            return;
        }
        if let Expression::ParenthesizedExpression(pe) = expr {
            self.find_logs_in_expr(&pe.expression, logs, top);
        }
    }

    // ── CSS.load detection ─────────────────────────────────────────────────

    fn check_css_load(&self, call: &CallExpression, out: &mut Vec<MxImport>) {
        let is_css = is_member_expr_callee(&call.callee, "CSS", "load");
        if !is_css {
            return;
        }
        if let Some(arg) = call.arguments.first().and_then(|a| a.as_expression()) {
            if let Some(url) = extract_string_lit(arg) {
                if url.starts_with("http://") || url.starts_with("https://") {
                    out.push(MxImport {
                        kind: MxImportKind::CssUrl { url },
                        style: "css_load".to_string(),
                    });
                } else {
                    out.push(MxImport {
                        kind: MxImportKind::CssLocal { path: url },
                        style: "css_load".to_string(),
                    });
                }
            }
        }
    }

    fn walk_css_load_stmts(&self, stmts: &[Statement], out: &mut Vec<MxImport>) {
        for stmt in stmts {
            if let Statement::ExpressionStatement(es) = stmt {
                if let Expression::CallExpression(call) = &es.expression {
                    self.check_css_load(call, out);
                }
            }
        }
    }
}

impl<'a> Visit<'a> for MxWalker<'_> {
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

        // Top-level `morphShared` / `morphEvent` bindings (exported or not —
        // the linter enforces the export requirement).
        for stmt in &program.body {
            match stmt {
                Statement::ExportDeclaration(d) => {
                    if let Declaration::VariableDeclaration(vd) = &d.declaration {
                        for decl in &vd.declarations {
                            self.extract_binding_declarator(decl);
                        }
                    }
                }
                Statement::VariableDeclaration(vd) => {
                    for decl in &vd.declarations {
                        self.extract_binding_declarator(decl);
                    }
                }
                _ => {}
            }
        }

        // Phase 2: components
        for stmt in &program.body {
            match stmt {
                Statement::ExportDefaultDeclaration(d) => match &d.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                        let before = self.components.len();
                        self.extract_component(f, true, true);
                        if self.components.len() == before {
                            // Not JSX: a plain module helper with a default
                            // export (previously dropped entirely).
                            if let Some(id) = &f.id {
                                let name = id.name.to_string();
                                self.function_declarations.push(InnerFunction {
                                    name: name.clone(),
                                    source: self.span_text(f.span).to_string(),
                                });
                                self.default_export = Some(name);
                            }
                        } else if let Some(last) = self.components.last() {
                            if !last.name.is_empty() {
                                self.default_export = Some(last.name.clone());
                            }
                        }
                    }
                    ExportDefaultDeclarationKind::ArrowFunctionExpression(arrow) => {
                        let before = self.components.len();
                        self.extract_arrow_component("default".to_string(), arrow, true, true);
                        if self.components.len() != before {
                            if let Some(last) = self.components.last() {
                                if !last.name.is_empty() {
                                    self.default_export = Some(last.name.clone());
                                }
                            }
                        }
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                        let name = c
                            .id
                            .as_ref()
                            .map_or_else(|| "default_export".to_string(), |id| id.name.to_string());
                        self.class_declarations.push(ClassDecl {
                            name: name.clone(),
                            source: self.span_text(c.span).to_string(),
                            exported: true,
                        });
                        self.default_export = Some(name);
                    }
                    ExportDefaultDeclarationKind::Identifier(id) => {
                        self.default_export = Some(id.name.to_string());
                    }
                    _ => {}
                },
                Statement::ExportDeclaration(d) => match &d.declaration {
                    Declaration::FunctionDeclaration(f) => {
                        let before = self.components.len();
                        self.extract_component(f, true, false);
                        if self.components.len() == before {
                            // Exported non-component helper (previously
                            // dropped: only JSX registered as a component).
                            if let Some(id) = &f.id {
                                self.function_declarations.push(InnerFunction {
                                    name: id.name.to_string(),
                                    source: self.span_text(f.span).to_string(),
                                });
                            }
                        }
                    }
                    Declaration::VariableDeclaration(vd) => {
                        for decl in &vd.declarations {
                            let before = self.components.len();
                            self.try_extract_arrow_declarator(decl, true, false);
                            if self.components.len() == before {
                                if let Some(name) = binding_ident_name(&decl.id) {
                                    self.exported_vars.push(ExportedVar {
                                        name,
                                        source: self.span_text(decl.span).to_string(),
                                    });
                                }
                            }
                        }
                    }
                    Declaration::ClassDeclaration(c) => {
                        if let Some(id) = &c.id {
                            self.class_declarations.push(ClassDecl {
                                name: id.name.to_string(),
                                source: self.span_text(c.span).to_string(),
                                exported: true,
                            });
                        }
                    }
                    _ => {}
                },
                Statement::ExportNamedDeclaration(d) => {
                    // `export { a, b as c }`: mark locals exported.
                    for spec in &d.specifiers {
                        self.named_exports.push((
                            Self::export_name(&spec.local),
                            Self::export_name(&spec.exported),
                        ));
                    }
                }
                Statement::ExportFromDeclaration(d) => {
                    // `export { a, b as c } from './path'` re-export.
                    let mut names = Vec::new();
                    for spec in &d.specifiers {
                        names.push((
                            Self::export_name(&spec.local),
                            Self::export_name(&spec.exported),
                        ));
                    }
                    self.re_exports.push(ReExport {
                        path: d.source.value.to_string(),
                        names,
                        star: false,
                        star_as: None,
                    });
                }
                Statement::ExportAllDeclaration(d) => {
                    // `export * [as ns] from './path'`.
                    let star_as = d.exported.as_ref().and_then(|e| match e {
                        ModuleExportName::IdentifierName(id) => Some(id.name.to_string()),
                        ModuleExportName::IdentifierReference(r) => Some(r.name.to_string()),
                        ModuleExportName::StringLiteral(_) => None,
                    });
                    self.re_exports.push(ReExport {
                        path: d.source.value.to_string(),
                        names: Vec::new(),
                        star: true,
                        star_as,
                    });
                }
                Statement::ClassDeclaration(c) => {
                    // Non-exported module class: inventoried for own-module
                    // rewriting (same as plain helper functions below).
                    if let Some(id) = &c.id {
                        self.class_declarations.push(ClassDecl {
                            name: id.name.to_string(),
                            source: self.span_text(c.span).to_string(),
                            exported: false,
                        });
                    }
                }
                Statement::FunctionDeclaration(f) => {
                    // A function that returns JSX is a component; otherwise it's a
                    // module-level helper function (e.g. `compute`), transpiled into
                    // premain like Python's `function_declarations`.
                    if f.body.as_ref().is_some_and(|b| {
                        b.statements.iter().any(|s| {
                            if let Statement::ReturnStatement(ret) = s {
                                ret.argument.as_ref().is_some_and(|a| self.expr_is_jsx(a))
                            } else {
                                false
                            }
                        })
                    }) {
                        self.extract_component(f, false, false);
                    } else if let Some(id) = &f.id {
                        self.function_declarations.push(InnerFunction {
                            name: id.name.to_string(),
                            source: self.span_text(f.span).to_string(),
                        });
                    }
                }
                Statement::VariableDeclaration(vd) => {
                    for decl in &vd.declarations {
                        self.try_extract_arrow_declarator(decl, false, false);
                    }
                }
                _ => {}
            }
        }

        // Phase 3: global vars (non-component)
        let component_names: std::collections::HashSet<String> =
            self.components.iter().map(|c| c.name.clone()).collect();
        for stmt in &program.body {
            if matches!(
                stmt,
                Statement::ImportDeclaration(_)
                    | Statement::ExportDeclaration(_)
                    | Statement::ExportDefaultDeclaration(_)
                    | Statement::FunctionDeclaration(_)
            ) {
                continue;
            }
            if let Statement::VariableDeclaration(vd) = stmt {
                let has_component = vd.declarations.iter().any(|d| {
                    binding_ident_name(&d.id).is_some_and(|n| component_names.contains(&n))
                });
                if has_component {
                    continue;
                }
                let is_framework_call = vd.declarations.iter().any(|d| {
                    if let Some(Expression::CallExpression(call)) = &d.init {
                        matches!(&call.callee, Expression::Identifier(id) if {
                            let n = id.name.as_str();
                            n == "morphState"
                                || n == "morphShared"
                                || n == "morphEvent"
                                || n == "morphOn"
                                || n == "morphEmit"
                        })
                    } else {
                        false
                    }
                });
                if !is_framework_call {
                    self.global_vars.push(self.span_text(vd.span).to_string());
                }
            }
        }
    }
}

#[cfg(test)]
mod walker_tests {
    use crate::ast_types::MxImportKind;
    use crate::{parse_mx_str, ComponentProp, MxSource};

    fn parse(src: &str) -> MxSource {
        parse_mx_str(src, "test.mx").expect("parse")
    }

    #[test]
    fn default_import_binds_local_name() {
        let s =
            parse("import Hero from './Hero.mx'\nexport default function App() { return <div/> }");
        assert_eq!(s.imports.len(), 1);
        match &s.imports[0].kind {
            MxImportKind::Component { path, default, specifiers } => {
                assert_eq!(path, "./Hero.mx");
                assert_eq!(default.as_deref(), Some("Hero"));
                assert!(specifiers.is_empty());
                assert!(s.imports[0].kind.is_mx());
            }
            k => panic!("unexpected kind: {k:?}"),
        }
    }

    #[test]
    fn named_imports_collect_specifiers() {
        let s = parse(
            "import { Card, Btn } from './ui.mx'\nexport default function App() { return <div/> }",
        );
        match &s.imports[0].kind {
            MxImportKind::Component { default, specifiers, .. } => {
                assert_eq!(default, &None);
                assert_eq!(specifiers, &vec!["Card".to_string(), "Btn".to_string()]);
            }
            k => panic!("unexpected kind: {k:?}"),
        }
    }

    #[test]
    fn exported_helpers_classes_vars_are_inventoried() {
        let s = parse(
            "export function loadData(): int { return 1 }\nexport const token = \"abc\"\nexport class User {}\nclass Helper {}\nexport { loadData as fetch }\nexport * from './other.ts'\n",
        );
        assert!(s.function_declarations.iter().any(|f| f.name == "loadData"), "{s:?}");
        assert!(s.exported_vars.iter().any(|v| v.name == "token"), "{s:?}");
        assert!(s.class_declarations.iter().any(|c| c.name == "User" && c.exported), "{s:?}");
        assert!(s.class_declarations.iter().any(|c| c.name == "Helper" && !c.exported), "{s:?}");
        assert!(s.named_exports.contains(&("loadData".to_string(), "fetch".to_string())), "{s:?}");
        assert_eq!(s.re_exports.len(), 1);
        assert_eq!(s.re_exports[0].path, "./other.ts");
        assert!(s.re_exports[0].star);
    }

    #[test]
    fn default_exports_map_to_declared_names() {
        let s = parse("export default function loadData(): int { return 1 }");
        assert!(s.function_declarations.iter().any(|f| f.name == "loadData"), "{s:?}");
        assert_eq!(s.default_export.as_deref(), Some("loadData"));
        let s = parse("export default class Store {}");
        assert!(s.class_declarations.iter().any(|c| c.name == "Store" && c.exported), "{s:?}");
        assert_eq!(s.default_export.as_deref(), Some("Store"));
        let s = parse("const helper = 1\nexport default helper");
        assert_eq!(s.default_export.as_deref(), Some("helper"));
    }

    #[test]
    fn named_reexports_record_paths_and_pairs() {
        let s = parse("export { loadData, token as auth } from './utility.ts'");
        assert_eq!(s.re_exports.len(), 1);
        assert_eq!(s.re_exports[0].path, "./utility.ts");
        assert!(!s.re_exports[0].star);
        assert!(s.re_exports[0].names.contains(&("loadData".to_string(), "loadData".to_string())));
        assert!(s.re_exports[0].names.contains(&("token".to_string(), "auth".to_string())));
    }

    #[test]
    fn typed_identifier_props() {
        let s = parse(
            "export function C(props: { label: string, step?: number, onStep: (v: number) => void }) { return (<div/>) }",
        );
        assert_eq!(s.components.len(), 1);
        let c = &s.components[0];
        assert_eq!(c.name, "C");
        assert!(c.exported && !c.is_default);
        assert_eq!(c.props_param, "props");
        assert_eq!(c.params, vec!["label", "step", "onStep"]);
        assert_eq!(c.props[0].prop_type, "string");
        assert!(!c.props[0].optional);
        assert_eq!(c.props[1].prop_type, "number");
        assert!(c.props[1].optional);
        assert_eq!(c.props[2].prop_type, "(v: number) => void");
        assert!(c.props[2].is_function());
        assert!(!c.props[0].is_function());
    }

    #[test]
    fn destructured_props_merge_with_annotation() {
        let s = parse(
            "export function C({ title, count }: { title: string, count?: number }) { return (<div/>) }",
        );
        let c = &s.components[0];
        assert_eq!(c.props_param, "");
        assert_eq!(c.params, vec!["title", "count"]);
        assert_eq!(c.props[0].prop_type, "string");
        assert!(c.props[1].optional);
    }

    #[test]
    fn untyped_and_empty_params() {
        let s = parse(
            "export function A(props) { return (<div/>) }\nexport function B() { return (<span/>) }",
        );
        assert_eq!(s.components.len(), 2);
        assert_eq!(s.components[0].props_param, "props");
        assert!(s.components[0].props.is_empty());
        assert_eq!(s.components[1].props_param, "");
    }

    #[test]
    fn export_kinds_are_tracked() {
        let s = parse(
            "export function Named() { return (<div/>) }\nexport default function Root() { return (<span/>) }\nfunction Local() { return (<p/>) }\nfunction helper() { return 1; }",
        );
        assert_eq!(s.components.len(), 3);
        let named = s.components.iter().find(|c| c.name == "Named").unwrap();
        assert!(named.exported && !named.is_default);
        let root = s.components.iter().find(|c| c.name == "Root").unwrap();
        assert!(root.exported && root.is_default);
        let local = s.components.iter().find(|c| c.name == "Local").unwrap();
        assert!(!local.exported && !local.is_default);
        assert_eq!(s.function_declarations.len(), 1);
        assert_eq!(s.function_declarations[0].name, "helper");
    }

    #[test]
    fn arrow_const_components() {
        let s = parse(
            "export const Card = (props: { t: string }) => (<div>{props.t}</div>)\nconst Plain = () => 42\nconst Hidden = () => { const [x, setX] = morphState(0); return (<b/>); }",
        );
        assert_eq!(s.components.len(), 2);
        let card = s.components.iter().find(|c| c.name == "Card").unwrap();
        assert!(card.exported && !card.is_default);
        assert_eq!(card.props_param, "props");
        assert_eq!(card.props.len(), 1);
        let hidden = s.components.iter().find(|c| c.name == "Hidden").unwrap();
        assert!(!hidden.exported);
        assert_eq!(hidden.state_vars.len(), 1);
        // Non-JSX arrows stay plain globals.
        assert!(s.global_vars.iter().any(|g| g.contains("Plain")));
        assert!(!s.global_vars.iter().any(|g| g.contains("Hidden")));
    }

    #[test]
    fn prop_fn_type_detection() {
        let f = |t: &str| {
            ComponentProp { name: "p".into(), prop_type: t.into(), optional: false }.is_function()
        };
        assert!(f("(v: number) => void"));
        assert!(f("() => void"));
        assert!(f("Function"));
        assert!(!f("string"));
        assert!(!f("string[]"));
        assert!(!f(""));
        assert!(!f("{ cb: () => void }"));
    }

    #[test]
    fn annotation_parser_handles_nesting() {
        let s = parse(
            "export function C(props: { user: { name: string, tags: string[] }, cb: (a: string, b: number) => void, mode?: \"a\" | \"b\" }) { return (<div/>) }",
        );
        let c = &s.components[0];
        assert_eq!(c.props.len(), 3);
        assert_eq!(c.props[0].prop_type, "{ name: string, tags: string[] }");
        assert!(c.props[1].is_function());
        assert!(c.props[2].optional);
    }
}

#[cfg(test)]
mod shared_tests {
    use crate::parse_mx_str;

    #[test]
    fn exported_top_level_shared_binding() {
        let src = r#"
export const [theme, setTheme] = morphShared("light")
export default function App() {
  const [count, setCount] = morphState(0)
  return (<div/>)
}
"#;
        let s = parse_mx_str(src, "test.mx").unwrap();
        assert_eq!(s.shared_bindings.len(), 1);
        let b = &s.shared_bindings[0];
        assert_eq!(b.getter, "theme");
        assert_eq!(b.setter, "setTheme");
        assert_eq!(b.init, "\"light\"");
        assert_eq!(s.components[0].state_vars.len(), 1);
        assert!(s.components[0].event_subs.is_empty());
    }

    #[test]
    fn exported_top_level_event_binding() {
        let src = r"
export const toastEvent = morphEvent<{ message: string }>()
export default function App() {
  return (<div/>)
}
";
        let s = parse_mx_str(src, "test.mx").unwrap();
        assert_eq!(s.event_bindings.len(), 1);
        assert_eq!(s.event_bindings[0].name, "toastEvent");
        assert_eq!(s.event_bindings[0].type_arg.as_deref(), Some("{ message: string }"));
    }

    #[test]
    fn event_subs_from_dot_on() {
        let src = r"
export const toastEvent = morphEvent<{ message: string }>()
export default function App() {
  toastEvent.on((payload) => { console.log(payload) })
  return (<div/>)
}
";
        let s = parse_mx_str(src, "test.mx").unwrap();
        let c = &s.components[0];
        assert_eq!(c.event_subs.len(), 1);
        assert_eq!(c.event_subs[0].event, "toastEvent");
        assert!(c.event_subs[0].handler.contains("payload"));
    }

    #[test]
    fn state_var_type_arg() {
        let src = r"
export default function App() {
  const [count, setCount] = morphState<number>(0)
  return (<div>{count}</div>)
}
";
        let s = parse_mx_str(src, "test.mx").unwrap();
        assert_eq!(s.components[0].state_vars[0].type_arg.as_deref(), Some("number"));
    }

    #[test]
    fn non_exported_shared_binding_extracted() {
        let src = r#"
const [a, setA] = morphShared("k", 0)
export default function App() { return (<div/>) }
"#;
        let s = parse_mx_str(src, "test.mx").unwrap();
        assert_eq!(s.shared_bindings.len(), 1);
        assert_eq!(s.shared_bindings[0].getter, "a");
    }

    #[test]
    fn morph_shared_not_in_global_vars() {
        let src = r#"
const [a, setA] = morphShared("k", 0)
export default function App() { return (<div/>) }
"#;
        let s = parse_mx_str(src, "test.mx").unwrap();
        assert!(!s.global_vars.iter().any(|g| g.contains("morphShared")));
    }
}
