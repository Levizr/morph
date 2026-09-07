use oxc_ast::ast::*;
use oxc_span::GetSpan;

use super::context::{Ctx, INDENT};

pub struct RustTranslator<'a> {
    source: &'a str,
    ctx: Ctx,
}

impl<'a> RustTranslator<'a> {
    pub fn new(source: &'a str, indent_level: usize) -> Self {
        let mut ctx = Ctx::default();
        ctx.indent_level = indent_level;
        Self { source, ctx }
    }

    fn span_text(&self, span: oxc_span::Span) -> &'a str {
        span.source_text(self.source)
    }

    fn indent(&self) -> String {
        INDENT.repeat(self.ctx.indent_level)
    }

    pub fn translate_program(&mut self, program: &Program<'a>) -> String {
        let mut lines = Vec::new();
        lines.push("// Auto-generated Rust from TypeScript via morph-js".to_string());
        lines.push("use std::collections::HashMap;".to_string());
        lines.push("use serde_json::Value;".to_string());
        lines.push(String::new());

        let mut decls = Vec::new();
        let mut mains = Vec::new();
        for stmt in &program.body {
            let is_decl = matches!(
                stmt,
                Statement::FunctionDeclaration(_)
                    | Statement::ClassDeclaration(_)
                    | Statement::TSInterfaceDeclaration(_)
                    | Statement::TSTypeAliasDeclaration(_)
                    | Statement::TSEnumDeclaration(_)
                    | Statement::ImportDeclaration(_)
                    | Statement::ExportDeclaration(_)
                    | Statement::ExportDefaultDeclaration(_)
                    | Statement::ExportNamedDeclaration(_)
            );
            if let Some(code) = self.emit_statement(stmt) {
                if code.trim().is_empty() { continue; }
                if is_decl {
                    decls.push(code);
                } else {
                    // Strip leading indent for inside main (will re-indent)
                    mains.push(code.trim_start().to_string());
                }
            }
        }
        let decls_is_empty = decls.is_empty();
        for d in decls {
            lines.push(d);
            lines.push(String::new());
        }
        if !mains.is_empty() {
            lines.push("fn main() {".to_string());
            for m in mains {
                // m is like `let x: f64 = 42;` or `println!(...)` — indent inside main
                for line in m.lines() {
                    if line.trim().is_empty() {
                        lines.push(String::new());
                    } else {
                        lines.push(format!("    {}", line));
                    }
                }
            }
            lines.push("}".to_string());
        } else if decls_is_empty {
            lines.push("fn main() {}".to_string());
        }
        lines.join("\n")
    }

    fn emit_statement(&mut self, stmt: &Statement<'a>) -> Option<String> {
        match stmt {
            Statement::VariableDeclaration(d) => self.emit_variable_declaration(d),
            Statement::FunctionDeclaration(f) => self.emit_function_declaration(f),
            Statement::ClassDeclaration(c) => Some(self.emit_class(c)),
            Statement::TSInterfaceDeclaration(i) => Some(self.emit_interface(i)),
            Statement::BlockStatement(b) => Some(self.emit_block(b)),
            Statement::ExpressionStatement(e) => Some(format!("{}{};", self.indent(), self.emit_expression(&e.expression))),
            Statement::ReturnStatement(r) => Some(self.emit_return(r)),
            Statement::IfStatement(i) => Some(self.emit_if(i)),
            Statement::WhileStatement(w) => Some(format!("{}while {} {}", self.indent(), self.emit_expression(&w.test), self.emit_statement(&w.body).unwrap_or_else(|| "{}".to_string()))),
            Statement::ForStatement(f) => Some(self.emit_for(f)),
            Statement::BreakStatement(_) => Some(format!("{}break;", self.indent())),
            Statement::ContinueStatement(_) => Some(format!("{}continue;", self.indent())),
            Statement::TryStatement(t) => Some(self.emit_try(t)),
            Statement::ThrowStatement(t) => Some(format!("{}panic!(\"{}\");", self.indent(), self.emit_expression(&t.argument).replace('"', "\\\""))),
            Statement::EmptyStatement(_) => None,
            Statement::ImportDeclaration(_) => None,
            Statement::ExportDeclaration(e) => self.emit_declaration(&e.declaration),
            Statement::ExportDefaultDeclaration(e) => self.emit_export_default(&e.declaration),
            _ => Some(format!("{}// unhandled {:?}", self.indent(), stmt.span())),
        }
    }

    fn emit_declaration(&mut self, decl: &Declaration<'a>) -> Option<String> {
        match decl {
            Declaration::VariableDeclaration(d) => self.emit_variable_declaration(d),
            Declaration::FunctionDeclaration(f) => self.emit_function_declaration(f),
            Declaration::ClassDeclaration(c) => Some(self.emit_class(c)),
            _ => None,
        }
    }

    fn emit_export_default(&mut self, decl: &ExportDefaultDeclarationKind<'a>) -> Option<String> {
        match decl {
            ExportDefaultDeclarationKind::FunctionDeclaration(f) => self.emit_function_declaration(f),
            ExportDefaultDeclarationKind::ClassDeclaration(c) => Some(self.emit_class(c)),
            _ => decl.as_expression().map(|e| format!("{}{};", self.indent(), self.emit_expression(e))),
        }
    }

    fn emit_variable_declaration(&mut self, decl: &VariableDeclaration<'a>) -> Option<String> {
        let mut parts = Vec::new();
        for d in &decl.declarations {
            let (name, _) = self.binding_to_identifier(&d.id);
            if name.starts_with("/*") { continue; }
            let rust_type = d.type_annotation.as_ref().map(|ta| self.ts_type_to_rust(&ta.type_annotation)).unwrap_or_else(|| "auto".to_string());
            let rust_type = if rust_type == "auto" {
                if let Some(init) = &d.init {
                    self.infer_rust_type(init).unwrap_or("auto".to_string())
                } else { "auto".to_string() }
            } else { rust_type };
            let ty_str = if rust_type == "auto" { "".to_string() } else { format!(": {}", rust_type) };
            if let Some(init) = &d.init {
                let init_code = self.emit_expression(init);
                parts.push(format!("{}let {}{} = {};", self.indent(), name, ty_str, init_code));
            } else {
                parts.push(format!("{}let {}{};", self.indent(), name, ty_str));
            }
        }
        if parts.is_empty() { None } else { Some(parts.join("\n")) }
    }

    fn binding_to_identifier(&self, pat: &BindingPattern<'a>) -> (String, Option<String>) {
        match pat {
            BindingPattern::BindingIdentifier(id) => (id.name.to_string(), None),
            BindingPattern::AssignmentPattern(a) => self.binding_to_identifier(&a.left),
            _ => ("/*destruct*/".to_string(), None),
        }
    }

    fn ts_type_to_rust(&self, ty: &TSType<'a>) -> String {
        match ty {
            TSType::TSStringKeyword(_) => "String".to_string(),
            TSType::TSNumberKeyword(_) => "f64".to_string(),
            TSType::TSBooleanKeyword(_) => "bool".to_string(),
            TSType::TSVoidKeyword(_) => "()".to_string(),
            TSType::TSAnyKeyword(_) => "serde_json::Value".to_string(),
            TSType::TSArrayType(a) => format!("Vec<{}>", self.ts_type_to_rust(&a.element_type)),
            TSType::TSTypeReference(r) => {
                let name = match &r.type_name {
                    TSTypeName::IdentifierReference(id) => id.name.to_string(),
                    _ => "Value".to_string(),
                };
                if name == "Array" {
                    if let Some(args) = &r.type_arguments {
                        if let Some(first) = args.params.first() {
                            return format!("Vec<{}>", self.ts_type_to_rust(first));
                        }
                    }
                    return "Vec<Value>".to_string();
                }
                if name == "Promise" {
                    if let Some(args) = &r.type_arguments {
                        if let Some(first) = args.params.first() {
                            return self.ts_type_to_rust(first);
                        }
                    }
                    return "()".to_string();
                }
                name
            }
            _ => "String".to_string(),
        }
    }

    fn infer_rust_type(&self, expr: &Expression<'a>) -> Option<String> {
        match expr {
            Expression::StringLiteral(_) => Some("String".to_string()),
            Expression::NumericLiteral(_) => Some("f64".to_string()),
            Expression::BooleanLiteral(_) => Some("bool".to_string()),
            Expression::ArrayExpression(_) => Some("Vec<Value>".to_string()),
            Expression::ObjectExpression(_) => Some("HashMap<String, Value>".to_string()),
            _ => None,
        }
    }

    fn emit_block(&mut self, block: &BlockStatement<'a>) -> String {
        if block.body.is_empty() { return "{}".to_string(); }
        let mut lines = vec!["{".to_string()];
        let old = self.ctx.indent_level;
        self.ctx.indent_level = old + 1;
        for stmt in &block.body {
            if let Some(code) = self.emit_statement(stmt) { lines.push(code); }
        }
        self.ctx.indent_level = old;
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_return(&mut self, r: &ReturnStatement<'a>) -> String {
        if let Some(arg) = &r.argument {
            format!("{}return {};", self.indent(), self.emit_expression(arg))
        } else {
            format!("{}return;", self.indent())
        }
    }

    fn emit_if(&mut self, i: &IfStatement<'a>) -> String {
        let cond = self.emit_expression(&i.test);
        let cons = self.emit_statement(&i.consequent).unwrap_or_else(|| "{}".to_string());
        let mut s = format!("{}if {} {}", self.indent(), cond, cons);
        if let Some(alt) = &i.alternate {
            s.push_str(&format!(" else {}", self.emit_statement(alt).unwrap_or_default()));
        }
        s
    }

    fn emit_for(&mut self, f: &ForStatement<'a>) -> String {
        let init = f.init.as_ref().map(|init| match init {
            ForStatementInit::VariableDeclaration(d) => {
                if let Some(first) = d.declarations.first() {
                    let (name, _) = self.binding_to_identifier(&first.id);
                    let init_code = first.init.as_ref().map(|e| self.emit_expression(e)).unwrap_or_else(|| "0".to_string());
                    format!("let mut {} = {}", name, init_code)
                } else { "".to_string() }
            },
            _ => self.span_text(init.span()).to_string(),
        }).unwrap_or_default();
        let cond = f.test.as_ref().map(|e| self.emit_expression(e)).unwrap_or_else(|| "true".to_string());
        let update = f.update.as_ref().map(|e| self.emit_expression(e)).unwrap_or_default();
        let body = self.emit_statement(&f.body).unwrap_or_else(|| "{}".to_string());
        // Rust for is different; we emit as `for` with `while` for now
        if init.is_empty() && cond == "true" && update.is_empty() {
            format!("{}loop {}", self.indent(), body)
        } else {
            format!("{}{}; while {} {{ {} {}; }}", self.indent(), init, cond, body, update)
        }
    }

    fn emit_try(&mut self, t: &TryStatement<'a>) -> String {
        let mut lines = vec![format!("{}// try", self.indent()), self.emit_block(&t.block)];
        if let Some(h) = &t.handler {
            let param = h.param.as_ref().map(|p| self.binding_to_identifier(&p.pattern).0).unwrap_or_else(|| "e".to_string());
            lines.push(format!("{}// catch {}", self.indent(), param));
            lines.push(self.emit_block(&h.body));
        }
        if let Some(f) = &t.finalizer {
            lines.push(format!("{}// finally", self.indent()));
            lines.push(self.emit_block(f));
        }
        lines.join("\n")
    }

    fn emit_function_declaration(&mut self, f: &Function<'a>) -> Option<String> {
        let Some(id) = &f.id else { return None; };
        let name = id.name.to_string();
        let is_async = f.r#async;
        let params: Vec<String> = f.params.items.iter().map(|p| {
            let (n, _) = self.binding_to_identifier(&p.pattern);
            let ty = p.type_annotation.as_ref().map(|ta| self.ts_type_to_rust(&ta.type_annotation)).unwrap_or_else(|| "String".to_string());
            format!("{}: {}", n, ty)
        }).collect();
        let ret = f.return_type.as_ref().map(|rt| self.ts_type_to_rust(&rt.type_annotation)).unwrap_or_else(|| "()".to_string());
        let ret_str = if ret == "()" { "".to_string() } else { format!(" -> {}", ret) };
        let async_str = if is_async { "async " } else { "" };
        let body = f.body.as_ref().map(|b| self.emit_function_body(b)).unwrap_or_else(|| "{}".to_string());
        Some(format!("{}{}fn {}({}){} {}", self.indent(), async_str, name, params.join(", "), ret_str, body))
    }

    fn emit_function_body(&mut self, body: &FunctionBody<'a>) -> String {
        if body.statements.is_empty() { return "{}".to_string(); }
        let mut lines = vec!["{".to_string()];
        let old = self.ctx.indent_level;
        self.ctx.indent_level = old + 1;
        for stmt in &body.statements {
            if let Some(code) = self.emit_statement(stmt) { lines.push(code); }
        }
        self.ctx.indent_level = old;
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_class(&mut self, class: &Class<'a>) -> String {
        let name = class.id.as_ref().map(|id| id.name.to_string()).unwrap_or_else(|| "Unnamed".to_string());
        let mut lines = vec![format!("{}struct {} {{", self.indent(), name)];
        for el in &class.body.body {
            if let ClassElement::PropertyDefinition(p) = el {
                if let Some(key) = self.property_key_to_string(&p.key) {
                    let ty = p.type_annotation.as_ref().map(|ta| self.ts_type_to_rust(&ta.type_annotation)).unwrap_or_else(|| "String".to_string());
                    lines.push(format!("{}    {}: {},", self.indent(), key, ty));
                }
            }
        }
        lines.push(format!("{}}}", self.indent()));
        lines.push(String::new());
        lines.push(format!("{}impl {} {{", self.indent(), name));
        for el in &class.body.body {
            if let ClassElement::MethodDefinition(m) = el {
                if m.kind == MethodDefinitionKind::Constructor {
                    let params: Vec<String> = m.value.params.items.iter().map(|p| {
                        let (n, _) = self.binding_to_identifier(&p.pattern);
                        let ty = p.type_annotation.as_ref().map(|ta| self.ts_type_to_rust(&ta.type_annotation)).unwrap_or_else(|| "String".to_string());
                        format!("{}: {}", n, ty)
                    }).collect();
                    let body = m.value.body.as_ref().map(|b| self.emit_function_body(b)).unwrap_or_else(|| "{}".to_string());
                    lines.push(format!("{}    pub fn new({}) -> Self {}", self.indent(), params.join(", "), body));
                } else {
                    let method_name = self.property_key_to_string(&m.key).unwrap_or_else(|| "method".to_string());
                    let params: Vec<String> = m.value.params.items.iter().map(|p| {
                        let (n, _) = self.binding_to_identifier(&p.pattern);
                        let ty = p.type_annotation.as_ref().map(|ta| self.ts_type_to_rust(&ta.type_annotation)).unwrap_or_else(|| "String".to_string());
                        format!("{}: {}", n, ty)
                    }).collect();
                    let ret = m.value.return_type.as_ref().map(|rt| self.ts_type_to_rust(&rt.type_annotation)).unwrap_or_else(|| "()".to_string());
                    let ret_str = if ret == "()" { "".to_string() } else { format!(" -> {}", ret) };
                    let body = m.value.body.as_ref().map(|b| self.emit_function_body(b)).unwrap_or_else(|| "{}".to_string());
                    // Heuristic: if method uses `this`, make it &self (kept for future use)
                    let _self_param = if body.contains("self.") || body.contains("this") { " &self, " } else { "" };
                    let _params_str = if _self_param.is_empty() { params.join(", ") } else { format!("{} {}", _self_param.trim_end_matches(", "), params.join(", ")).trim().to_string().trim_start_matches(',').trim().to_string() };
                    // Simpler: always include &self for methods not static
                    let prefix = if m.r#static { "" } else { "&self, " };
                    let all_params = if params.is_empty() { if m.r#static { "".to_string() } else { "&self".to_string() } } else { format!("{}{}", prefix, params.join(", ")) };
                    lines.push(format!("{}    pub fn {}({}){} {}", self.indent(), method_name, all_params, ret_str, body));
                }
            }
        }
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_interface(&mut self, iface: &TSInterfaceDeclaration<'a>) -> String {
        let name = iface.id.name.to_string();
        let mut lines = vec![format!("{}trait {} {{", self.indent(), name)];
        for sig in &iface.body.body {
            match sig {
                TSSignature::TSMethodSignature(m) => {
                    if let Some(key) = self.property_key_to_string(&m.key) {
                        let params: Vec<String> = m.params.items.iter().map(|p| {
                            let (n, _) = self.binding_to_identifier(&p.pattern);
                            format!("{}: String", n)
                        }).collect();
                        lines.push(format!("{}    fn {}({});", self.indent(), key, params.join(", ")));
                    }
                }
                TSSignature::TSPropertySignature(p) => {
                    if let Some(key) = self.property_key_to_string(&p.key) {
                        lines.push(format!("{}    fn get_{}(&self) -> String;", self.indent(), key));
                    }
                }
                _ => {}
            }
        }
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn property_key_to_string(&self, key: &PropertyKey<'a>) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
            PropertyKey::StringLiteral(s) => Some(s.value.to_string()),
            _ => None,
        }
    }

    fn emit_expression(&mut self, expr: &Expression<'a>) -> String {
        match expr {
            Expression::Identifier(id) => id.name.to_string(),
            Expression::NumericLiteral(n) => n.raw.map(|r| r.to_string()).unwrap_or_else(|| n.value.to_string()),
            Expression::StringLiteral(s) => format!("\"{}\"", s.value.escape_default()),
            Expression::BooleanLiteral(b) => b.value.to_string(),
            Expression::NullLiteral(_) => "None".to_string(),
            Expression::TemplateLiteral(t) => {
                let mut s = String::from("format!(\"");
                for (i, q) in t.quasis.iter().enumerate() {
                    s.push_str(&q.value.raw.as_str().replace('"', "\\\""));
                    if let Some(e) = t.expressions.get(i) {
                        s.push_str("{}");
                        let _ = self.emit_expression(e);
                    }
                }
                s.push_str("\")");
                s
            }
            Expression::CallExpression(c) => {
                let callee = self.emit_expression(&c.callee);
                let args: Vec<String> = c.arguments.iter().filter_map(|a| a.as_expression().map(|e| self.emit_expression(e))).collect();
                if callee == "console.log" || callee.ends_with(".log") {
                    return format!("println!({})", args.join(", "));
                }
                format!("{}({})", callee, args.join(", "))
            }
            Expression::StaticMemberExpression(m) => format!("{}.{}", self.emit_expression(&m.object), m.property.name),
            Expression::ComputedMemberExpression(m) => format!("{}[{}]", self.emit_expression(&m.object), self.emit_expression(&m.expression)),
            Expression::BinaryExpression(b) => format!("{} {} {}", self.emit_expression(&b.left), b.operator.as_str(), self.emit_expression(&b.right)),
            Expression::AssignmentExpression(a) => format!("{} {} {}", self.span_text(a.left.span()).to_string(), a.operator.as_str(), self.emit_expression(&a.right)),
            Expression::ArrayExpression(arr) => {
                let elems: Vec<String> = arr.elements.iter().filter_map(|e| e.as_expression().map(|ex| self.emit_expression(ex))).collect();
                format!("vec![{}]", elems.join(", "))
            }
            Expression::ObjectExpression(obj) => {
                let mut pairs = Vec::new();
                for prop in &obj.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = prop {
                        if let Some(k) = self.property_key_to_string(&p.key) {
                            pairs.push(format!("(\"{}\".to_string(), {})", k, self.emit_expression(&p.value)));
                        }
                    }
                }
                format!("HashMap::from([{}])", pairs.join(", "))
            }
            _ => self.span_text(expr.span()).to_string(),
        }
    }
}
