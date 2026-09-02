use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};
use std::collections::{HashMap, HashSet};

use morph_parser::LintError;

// Re-export LintError for convenience
pub use morph_parser::LintError as TsLintError;

fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offs = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' { offs.push(i+1); }
    }
    offs
}
fn offset_to_line_col(offsets: &[usize], offset: u32) -> (usize, usize) {
    let off = offset as usize;
    let line = match offsets.binary_search(&off) { Ok(idx) => idx+1, Err(idx) => idx };
    let start = offsets[line.saturating_sub(1)];
    (line, off.saturating_sub(start)+1)
}

#[derive(Clone, Copy, PartialEq)]
enum Strictness { Strict, Permissive } // Strict = .ts , Permissive = .js

pub fn check_str(content: &str, file_path: &str) -> Vec<LintError> {
    let is_js = file_path.ends_with(".js") || file_path.ends_with(".jsx") || file_path.ends_with(".mjs") || file_path.ends_with(".cjs");
    let strict = if is_js { Strictness::Permissive } else { Strictness::Strict };
    check_with_strictness(content, file_path, strict)
}

pub fn check_with_strictness(content: &str, file_path: &str, strictness: Strictness) -> Vec<LintError> {
    let offsets = build_line_offsets(content);
    let allocator = Allocator::default();
    let source_type = if file_path.ends_with(".tsx") || file_path.ends_with(".ts") {
        SourceType::from_path(file_path).unwrap_or_default().with_typescript(true)
    } else {
        SourceType::default().with_typescript(true)
    };
    let ret = Parser::new(&allocator, content, source_type).parse();
    let mut out = Vec::new();
    // Parse errors first — user friendly
    for diag in &ret.diagnostics {
        let span = diag.labels.first().map(|l| l.span()).unwrap_or(Span::new(0,0));
        let (line,col)= offset_to_line_col(&offsets, span.start);
        out.push(LintError {
            severity: "error".into(),
            code: "parse-error".into(),
            message: diag.message.to_string(),
            suggestion: diag.help.as_ref().map(|h| h.to_string()),
            file_path: file_path.into(),
            line, col,
        });
    }
    if ret.panicked {
        out.push(LintError {
            severity: "error".into(),
            code: "parse-panic".into(),
            message: "File has a syntax error and could not be read".into(),
            suggestion: Some("Check for missing `}` or `)` or an unfinished line".into()),
            file_path: file_path.into(),
            line: 1, col: 1
        });
        return out;
    }
    let mut l = Linter { source: content, file_path, offsets, strictness, errors: Vec::new(), declared: HashSet::new(), scope_stack: vec![HashSet::new()], in_strict: strictness == Strictness::Strict };
    l.visit_program(&ret.program);
    // Post-check: if no export and it's a .ts file being translated, warn
    out.extend(l.errors);
    out
}

struct Linter<'a> {
    source: &'a str,
    file_path: &'a str,
    offsets: Vec<usize>,
    strictness: Strictness,
    errors: Vec<LintError>,
    declared: HashSet<String>, // all top-level declared names
    scope_stack: Vec<HashSet<String>>,
    in_strict: bool,
}

impl<'a> Linter<'a> {
    fn line_col(&self, span: Span) -> (usize, usize) { offset_to_line_col(&self.offsets, span.start) }
    fn push(&mut self, span: Span, code: &str, severity: &str, message: String, suggestion: Option<String>) {
        let (line,col)= self.line_col(span);
        self.errors.push(LintError{ severity: severity.into(), code: code.into(), message, suggestion, file_path: self.file_path.into(), line, col });
    }
    fn is_declared(&self, name: &str) -> bool {
        self.scope_stack.iter().rev().any(|s| s.contains(name)) || self.declared.contains(name)
    }
    fn push_scope(&mut self){ self.scope_stack.push(HashSet::new()); }
    fn pop_scope(&mut self){ self.scope_stack.pop(); }
    fn declare(&mut self, name: String){
        if let Some(top)= self.scope_stack.last_mut(){ top.insert(name.clone()); }
        self.declared.insert(name);
    }

    fn type_str_of_expr(&self, expr: &Expression<'a>) -> Option<String> {
        match expr {
            Expression::StringLiteral(_) => Some("string".into()),
            Expression::TemplateLiteral(_) => Some("string".into()),
            Expression::NumericLiteral(_) => Some("number".into()),
            Expression::BooleanLiteral(_) => Some("boolean".into()),
            Expression::NullLiteral(_) => Some("null".into()),
            Expression::Identifier(id) => {
                // Try to find declaration type via var_types — we don't have it here, so fallback to unknown
                // We can look at the identifier name and guess? For now check if it's a known declared var via simple heuristic:
                // If we have seen `let x: string`, we could track. For now return None for unknown.
                let _ = id.name.as_str();
                None
            }
            Expression::CallExpression(c) => {
                // If callee is identifier with known return type? Hard without type info, skip
                let _ = c.callee.span();
                None
            }
            Expression::BinaryExpression(b) => {
                let lt = self.type_str_of_expr(&b.left);
                let rt = self.type_str_of_expr(&b.right);
                if lt.as_deref() == Some("string") || rt.as_deref() == Some("string") {
                    // string + anything in JS is string, but in strict TS we forbid it
                    // For linter, if either is string, the result is string
                    Some("string".into())
                } else if lt.as_deref() == Some("number") && rt.as_deref() == Some("number") {
                    Some("number".into())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn check_add(&mut self, left: &Expression<'a>, right: &Expression<'a>, span: Span, op: &str){
        if op != "+" { return; }
        // Only lint `+` — other ops are always numeric in user's mental model
        let lt = self.type_str_of_expr(left);
        let rt = self.type_str_of_expr(right);
        // If we can see that one side is string literal / template and the other is number/boolean, it's the bug
        let left_is_string = lt.as_deref() == Some("string") || matches!(left, Expression::StringLiteral(_) | Expression::TemplateLiteral(_));
        let right_is_string = rt.as_deref() == Some("string") || matches!(right, Expression::StringLiteral(_) | Expression::TemplateLiteral(_));
        let left_is_number = lt.as_deref() == Some("number") || matches!(left, Expression::NumericLiteral(_));
        let right_is_number = rt.as_deref() == Some("number") || matches!(right, Expression::NumericLiteral(_));
        let left_is_bool = lt.as_deref() == Some("boolean") || matches!(left, Expression::BooleanLiteral(_));
        let right_is_bool = rt.as_deref() == Some("boolean") || matches!(right, Expression::BooleanLiteral(_));

        // Also check Identifier that is a number literal via `let x = 2` — we don't have type info, so also check the source text for numeric vs quoted
        // Fallback: look at the raw source slice for the expression span to see if it looks like a number or string
        // This helps when type_str is None for identifiers
        let left_src = left.span().source_text(self.source).trim();
        let right_src = right.span().source_text(self.source).trim();
        let looks_like_string = |s: &str| (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) || s.starts_with('`');
        let looks_like_number = |s: &str| s.parse::<f64>().is_ok();

        let left_looks_str = left_is_string || looks_like_string(left_src);
        let right_looks_str = right_is_string || looks_like_string(right_src);
        let left_looks_num = left_is_number || looks_like_number(left_src);
        let right_looks_num = right_is_number || looks_like_number(right_src);
        let left_looks_bool = left_is_bool || left_src == "true" || left_src == "false";
        let right_looks_bool = right_is_bool || right_src == "true" || right_src == "false";

        let has_string = left_looks_str || right_looks_str;
        let has_number = left_looks_num || right_looks_num || left_looks_bool || right_looks_bool;

        // For the user's example: `2 + "Hello"` => left_looks_num && right_looks_str => has_string && has_number
        // `"This " + add(2,5)` where add returns int is also number+string but we can't see return type, so we also check the right_src for call expression
        // If either side is a string-looking and the other side is not also string-looking, it's a problem in strict mode
        let both_strings = left_looks_str && right_looks_str;
        // In strict TS, we want `+` to be ONLY for numbers. For strings, use template strings.
        // So: number + number => OK
        //      string + string => ERROR in strict (suggest template), OK in permissive
        //      string + number / number + string / string + boolean etc => ERROR in strict
        let is_number_plus_number = (left_looks_num || left_looks_bool) && (right_looks_num || right_looks_bool) && !has_string;
        // If we couldn't determine, be conservative: if has_string is true and not both_strings, it's likely a bug
        // Also if both_strings, it's also a bug in strict (should use template)
        let should_error = if self.strictness == Strictness::Strict {
            // Strict: no `+` with any string at all
            has_string
        } else {
            // Permissive (.js): only error if we are sure it's number+string (has_string && has_number) — but we will auto-wrap in codegen anyway, so don't error
            false
        };

        // However, for strict we also want to allow `+` for numbers only; so has_string => error
        if should_error {
            // But don't double-report for `+` where both are numbers — that's fine
            if is_number_plus_number { return; }
            // Don't error if both are unknown and no string look (conservative)
            if !has_string { return; }

            // Build user-friendly message
            let left_desc = if left_looks_str { "a string" } else if left_looks_num { "a number" } else { "a value" };
            let right_desc = if right_looks_str { "a string" } else if right_looks_num { "a number" } else { "a value" };
            let left_code = left_src.chars().take(20).collect::<String>();
            let right_code = right_src.chars().take(20).collect::<String>();
            let message = if both_strings {
                format!("You can't join two strings with `+` in TypeScript — it will do pointer math in C++")
            } else {
                format!("You can't add {} and {} with `+` — one is a string and the other is a number", left_desc, right_desc)
            };
            let suggestion = if both_strings {
                format!("Use a template string instead: `` `${{left}}${{right}}` `` or `String({}) + String({})` → in your code: \"`{}`\" + \"`{}`\" should be `` `${{{}}}${{{}}}` ``", left_code, right_code, left_code, right_code, left_code, right_code)
            } else {
                format!("Use a template string to be clear: `` `This ${{}}:` `` or convert the number first: `String({}) + \"{}\"`. In your code `{}` + `{}` → write `` `${{{}}}` + \"{}\" `` or `String({}) + \"{}\" `", left_code, right_code, left_code, right_code, left_code, right_code, left_code, right_code)
            };
            // More precise suggestion for the user's example: `"This 2 + 5 = " + add(2,5)` where left is string and right is call returning int
            // We can give a specific example: `"This 2 + 5 = " + add(2,5)` => ` `This 2 + 5 = ${add(2,5)}` `
            let final_suggestion = if left_looks_str && (right_src.contains('(') || right_looks_num) {
                format!("Change `\"{}\" + {}` to `` `{}` + \"${{ {} }}\" `` or `` `${{ {} }}` ``. Example: `\"This 2 + 5 = \" + add(2,5)` → `` `This 2 + 5 = ${{add(2,5)}}` ``", left_code, right_code, left_code, right_code, right_code)
            } else if right_looks_str && left_looks_num {
                format!("Change `{} + \"{}\"` to `` `${{ {} }} {}` `` or `String({}) + \"{}\" `", left_code, right_code, left_code, right_code, left_code, right_code)
            } else {
                suggestion
            };
            self.push(span, "ts-strict-add", "error", message, Some(final_suggestion));
        }
    }
}

impl<'a> Linter<'a> {
    fn visit_program(&mut self, program: &Program<'a>) {
        for stmt in &program.body {
            self.visit_statement(stmt);
        }
    }
    fn visit_statement(&mut self, stmt: &Statement<'a>) {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                // Rule: no `var`, only `let`/`const`
                if decl.kind == VariableDeclarationKind::Var {
                    let (line,col)= self.line_col(decl.span);
                    self.push(decl.span, "ts-no-var", "error",
                        "Don't use `var` — it behaves differently in C++. Use `let` for values that change or `const` for values that don't.".into(),
                        Some("Change `var x = ...` to `let x = ...` or `const x = ...`".into()));
                }
                for d in &decl.declarations {
                    let (name, _) = Self::binding_name(&d.id);
                    if let Some(n)=name { self.declare(n); }
                    // Rule: must have type annotation in strict mode
                    if self.in_strict && d.type_annotation.is_none() {
                        // Try to infer if initializer is simple and we could suggest a type
                        let inferred = d.init.as_ref().and_then(|e| self.type_str_of_expr(e));
                        let (line,col)= self.line_col(d.id.span());
                        let init_src = d.init.as_ref().map(|e| e.span().source_text(self.source).chars().take(20).collect::<String>()).unwrap_or_default();
                        let msg = if let Some(n)= &name {
                            format!("`{n}` needs a type. In TypeScript every variable must say what it is.")
                        } else {
                            "This variable needs a type.".into()
                        };
                        let sugg = if let Some(t)= inferred {
                            format!("Add a type like `: {t}` — example: `let {}: {} = {}`", name.clone().unwrap_or("x".into()), t, init_src)
                        } else if let Some(n)= &name {
                            format!("Add a type — is `{n}` a string, a number, or something else? Example: `let {n}: number = 1` or `let {n}: string = \"hello\"`")
                        } else {
                            "Add a type like `: number` or `: string`".into()
                        };
                        self.push(d.span, "ts-require-type", "error", msg, Some(sugg));
                    }
                    if let Some(init)= &d.init {
                        self.visit_expression(init);
                    }
                    // Check for `any`
                    if let Some(ta)= &d.type_annotation {
                        if ta.type_annotation.span().source_text(self.source).trim() == "any" {
                            self.push(ta.span, "ts-no-any", "error",
                                "Don't use `any` — it hides mistakes that will crash in C++.".into(),
                                Some("Use a real type like `number`, `string`, or `unknown` and check it.".into()));
                        }
                    }
                }
            }
            Statement::FunctionDeclaration(f) => {
                if let Some(id)= &f.id {
                    self.declare(id.name.to_string());
                    // Must have return type in strict
                    if self.in_strict && f.return_type.is_none() {
                        // Allow `void` if no return? Check if function has return with value
                        let has_return_val = f.body.as_ref().map(|b| b.statements.iter().any(|s| matches!(s, Statement::ReturnStatement(r) if r.argument.is_some()))).unwrap_or(false);
                        if has_return_val {
                            self.push(f.span, "ts-require-return-type", "error",
                                format!("Function `{}` returns a value, so you must say what type it returns.", id.name),
                                Some(format!("Add a return type: `function {}(...): number {{` or `: string`", id.name)));
                        } else if f.body.is_some() {
                            // Even for void, suggest explicit void in strict for clarity, but make it warning
                            self.push(f.span, "ts-require-return-type", "warning",
                                format!("Function `{}` should say what it returns, even if it's nothing.", id.name),
                                Some(format!("Add `: void` if it returns nothing: `function {}(...): void {{`", id.name)));
                        }
                    }
                    // Check param types
                    for p in &f.params.items {
                        let (n,_) = Self::binding_name(&p.pattern);
                        if let Some(name)= n { self.declare(name); }
                        if self.in_strict && p.type_annotation.is_none() && p.pattern.type_annotation().is_none() {
                            // p.pattern.type_annotation() is via BindingPattern? Check via FormalParameter's type_annotation? Actually FormalParameter doesn't have type_annotation directly, need to check p.pattern
                            // For now, check if pattern has type annotation via source text `:` 
                            let param_src = p.span.source_text(self.source);
                            if !param_src.contains(':') {
                                self.push(p.span, "ts-require-param-type", "error",
                                    format!("Parameter `{}` needs a type.", n.clone().unwrap_or("param".into())),
                                    Some(format!("Write it like `{}: number` or `{}: string`", n.clone().unwrap_or("x".into()), n.unwrap_or("x".into()))));
                            }
                        }
                    }
                }
                self.push_scope();
                if let Some(body)=&f.body { for s in &body.statements { self.visit_statement(s); } }
                self.pop_scope();
            }
            Statement::BlockStatement(b) => { self.push_scope(); for s in &b.body { self.visit_statement(s); } self.pop_scope(); }
            Statement::ExpressionStatement(es) => self.visit_expression(&es.expression),
            Statement::ReturnStatement(r) => if let Some(e)=&r.argument { self.visit_expression(e); },
            Statement::IfStatement(i) => { self.visit_expression(&i.test); self.visit_statement(&i.consequent); if let Some(alt)=&i.alternate { self.visit_statement(alt); } },
            Statement::WhileStatement(w) => { self.visit_expression(&w.test); self.visit_statement(&w.body); },
            Statement::DoWhileStatement(d) => { self.visit_statement(&d.body); self.visit_expression(&d.test); },
            Statement::ForStatement(f) => {
                self.push_scope();
                if let Some(init)=&f.init { match init { ForStatementInit::VariableDeclaration(v) => self.visit_statement(&Statement::VariableDeclaration(v.clone())), _ => if let Some(e)=init.as_expression(){ self.visit_expression(e); } } }
                if let Some(t)=&f.test { self.visit_expression(t); }
                if let Some(u)=&f.update { self.visit_expression(u); }
                self.visit_statement(&f.body);
                self.pop_scope();
            }
            Statement::ForInStatement(f) => { self.visit_expression(&f.right); self.push_scope(); self.visit_statement(&f.body); self.pop_scope(); }
            Statement::ForOfStatement(f) => { self.visit_expression(&f.right); self.push_scope(); self.visit_statement(&f.body); self.pop_scope(); }
            Statement::SwitchStatement(s) => { self.visit_expression(&s.discriminant); for c in &s.cases { if let Some(t)=&c.test { self.visit_expression(t); } for st in &c.consequent { self.visit_statement(st); } } }
            Statement::TryStatement(t) => { self.visit_statement(&Statement::BlockStatement(t.block.clone())); if let Some(h)=&t.handler { self.push_scope(); for st in &h.body.body { self.visit_statement(st); } self.pop_scope(); } if let Some(f)=&t.finalizer { self.visit_statement(&Statement::BlockStatement(f.clone())); } }
            Statement::ThrowStatement(t) => self.visit_expression(&t.argument),
            Statement::ClassDeclaration(c) => {
                if let Some(id)=&c.id { self.declare(id.name.to_string()); }
                self.push_scope();
                for el in &c.body.body {
                    match el {
                        ClassElement::MethodDefinition(m) => {
                            self.push_scope();
                            for p in &m.value.params.items { if let Some((n,_))=Self::binding_name_opt(&p.pattern) { self.declare(n); } }
                            if let Some(b)=&m.value.body { for s in &b.statements { self.visit_statement(s); } }
                            self.pop_scope();
                        }
                        ClassElement::PropertyDefinition(p) => if let Some(v)=&p.value { self.visit_expression(v); },
                        _ => {}
                    }
                }
                self.pop_scope();
            }
            Statement::TSInterfaceDeclaration(_) => {},
            Statement::TSTypeAliasDeclaration(t) => {
                if t.type_annotation.span().source_text(self.source).trim() == "any" {
                    self.push(t.span, "ts-no-any", "error", "Don't use `any` for a type alias.".into(), Some("Use `unknown` or a specific type.".into()));
                }
            }
            Statement::TSEnumDeclaration(e) => {
                for m in &e.body.members { if let Some(init)=&m.initializer { self.visit_expression(init); } }
            }
            Statement::BreakStatement(_) | Statement::ContinueStatement(_) | Statement::EmptyStatement(_) | Statement::DebuggerStatement(_) => {},
            Statement::LabeledStatement(l) => self.visit_statement(&l.body),
            Statement::WithStatement(w) => { self.visit_expression(&w.object); self.visit_statement(&w.body); }
            Statement::ImportDeclaration(_) | Statement::ExportNamedDeclaration(_) | Statement::ExportDefaultDeclaration(_) | Statement::ExportAllDeclaration(_) | Statement::ExportDeclaration(_) => {},
            _ => {}
        }
    }
    fn visit_expression(&mut self, expr: &Expression<'a>) {
        match expr {
            Expression::BinaryExpression(b) => {
                self.check_add(&b.left, &b.right, b.span, b.operator.as_str());
                self.visit_expression(&b.left);
                self.visit_expression(&b.right);
                // Also check == vs === in strict
                if b.operator == BinaryOperator::Equality || b.operator == BinaryOperator::Inequality {
                    self.push(b.span, "ts-strict-equality", "error",
                        "Use `===` and `!==` instead of `==` and `!=` — they don't do the same thing.".into(),
                        Some("Change `a == b` to `a === b` and `a != b` to `a !== b`".into()));
                }
            }
            Expression::LogicalExpression(l) => { self.visit_expression(&l.left); self.visit_expression(&l.right); }
            Expression::UnaryExpression(u) => self.visit_expression(&u.argument),
            Expression::UpdateExpression(u) => { let _ = &u.argument; }
            Expression::AssignmentExpression(a) => {
                // Check if assigning to undeclared variable
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    if !self.is_declared(id.name.as_str()) {
                        self.push(a.span, "ts-undeclared", "error",
                            format!("`{}` hasn't been declared. Did you forget `let` or `const`?", id.name),
                            Some(format!("Write `let {} = ...` before using it", id.name)));
                    }
                }
                self.visit_expression(&a.right);
            }
            Expression::CallExpression(c) => {
                // Check callee exists if it's an identifier
                if let Expression::Identifier(id) = &c.callee {
                    if !self.is_declared(id.name.as_str()) {
                        // Allow globals like console, fetch, etc.
                        let allowed = ["console","fetch","setTimeout","setInterval","clearTimeout","clearInterval","Math","Number","String","Boolean","Array","Object","JSON","Promise","setTimeout"];
                        if !allowed.contains(&id.name.as_str()) {
                            self.push(c.span, "ts-undefined-function", "error",
                                format!("Function `{}` doesn't exist — are you missing an import or did you misspell it?", id.name),
                                Some(format!("Check the spelling of `{}` or add `function {}() {{}}`", id.name, id.name)));
                        }
                    }
                }
                for arg in &c.arguments { if let Some(e)= arg.as_expression() { self.visit_expression(e); } }
                self.visit_expression(&c.callee);
            }
            Expression::MemberExpression(m) => {
                match m {
                    MemberExpression::StaticMemberExpression(s) => self.visit_expression(&s.object),
                    MemberExpression::ComputedMemberExpression(c) => { self.visit_expression(&c.object); self.visit_expression(&c.expression); }
                    MemberExpression::PrivateFieldExpression(p) => self.visit_expression(&p.object),
                }
            }
            Expression::ConditionalExpression(c) => { self.visit_expression(&c.test); self.visit_expression(&c.consequent); self.visit_expression(&c.alternate); }
            Expression::ArrayExpression(a) => for el in &a.elements { if let Some(e)= el.as_expression() { self.visit_expression(e); } },
            Expression::ObjectExpression(o) => for p in &o.properties { if let ObjectPropertyKind::ObjectProperty(prop)= p { self.visit_expression(&prop.value); } },
            Expression::TemplateLiteral(t) => for e in &t.expressions { self.visit_expression(e); },
            Expression::TaggedTemplateExpression(t) => { self.visit_expression(&t.tag); for e in &t.quasi.expressions { self.visit_expression(e); } },
            Expression::SequenceExpression(s) => for e in &s.expressions { self.visit_expression(e); },
            Expression::ParenthesizedExpression(p) => self.visit_expression(&p.expression),
            Expression::AwaitExpression(a) => self.visit_expression(&a.argument),
            Expression::YieldExpression(y) => if let Some(a)=&y.argument { self.visit_expression(a); },
            Expression::ChainExpression(c) => match &c.expression {
                ChainElement::CallExpression(call) => self.visit_expression(&Expression::CallExpression(call.clone())),
                ChainElement::StaticMemberExpression(s) => self.visit_expression(&Expression::StaticMemberExpression(s.clone())),
                ChainElement::ComputedMemberExpression(c) => self.visit_expression(&Expression::ComputedMemberExpression(c.clone())),
                _ => {}
            },
            Expression::ArrowFunctionExpression(f) => {
                self.push_scope();
                for p in &f.params.items { if let Some((n,_))=Self::binding_name_opt(&p.pattern) { self.declare(n); } }
                match &f.body {
                    ArrowFunctionBody::FunctionBody(b) => for s in &b.statements { self.visit_statement(s); },
                    _ => if let Some(e)=f.body.as_expression() { self.visit_expression(e); },
                }
                self.pop_scope();
            }
            Expression::FunctionExpression(f) => {
                self.push_scope();
                for p in &f.params.items { if let Some((n,_))=Self::binding_name_opt(&p.pattern) { self.declare(n); } }
                if let Some(b)=&f.body { for s in &b.statements { self.visit_statement(s); } }
                self.pop_scope();
            }
            Expression::Identifier(id) => {
                // Use of undeclared variable
                if !self.is_declared(id.name.as_str()) {
                    // Allow console, Math, etc.
                    let allowed = ["console","Math","Number","String","Boolean","Array","Object","JSON","Promise","undefined","null","true","false","NaN","Infinity"];
                    if !allowed.contains(&id.name.as_str()) {
                        // Only warn if it's likely a real variable, not a property
                        // For now, don't error on bare identifiers that could be globals — just check if it's in a call like `add(2,5)` where add is not declared
                        // We already check calls above, so skip here to avoid double
                    }
                }
            }
            Expression::Super(_) | Expression::ThisExpression(_) | Expression::NullLiteral(_) | Expression::BooleanLiteral(_) | Expression::NumericLiteral(_) | Expression::StringLiteral(_) | Expression::BigIntLiteral(_) | Expression::RegExpLiteral(_) | Expression::TemplateLiteral(_) | Expression::JSXElement(_) | Expression::JSXFragment(_) => {},
            _ => {},
        }
    }

    fn binding_name(pat: &BindingPattern<'a>) -> (Option<String>, Option<String>) {
        Self::binding_name_opt(pat).unwrap_or((None,None))
    }
    fn binding_name_opt(pat: &BindingPattern<'a>) -> Option<(String, Option<String>)> {
        match pat {
            BindingPattern::BindingIdentifier(id) => Some((id.name.to_string(), None)),
            BindingPattern::AssignmentPattern(a) => Self::binding_name_opt(&a.left),
            BindingPattern::ArrayPattern(ap) => ap.elements.iter().find_map(|e| e.as_ref().and_then(|p| Self::binding_name_opt(p))),
            BindingPattern::ObjectPattern(op) => op.properties.iter().find_map(|prop| Self::binding_name_opt(&prop.value)),
        }
    }
}
