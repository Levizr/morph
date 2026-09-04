// Semantic analyzer for intent-based codegen
// Performs escape analysis, type widening, async boundary detection, and closure capture detection

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EscapeKind {
    None,              // Stack allocation - doesn't escape
    Return,            // Returned from function - unique_ptr + move
    Global,            // Stored in global/container - unique_ptr + move
    ClosureCapture,    // Captured by closure - shared_ptr
    MultipleRefs,      // Assigned to multiple vars - shared_ptr
    AsyncBoundary,     // Crosses await/co_return - shared_ptr
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidenedType {
    None,              // Keep native type
    ToJsNumber,        // int/float -> JsNumber (handles bigint/overflow)
    ToJsString,        // string -> JsString (if .toString() called)
    ToJsValue,         // any -> JsValue (completely dynamic)
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub annotated_type: Option<String>,
    pub escape_kind: EscapeKind,
    pub widened_type: WidenedType,
    pub is_mutable: bool,
    pub usages: Vec<UsageKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UsageKind {
    ArithmeticOp,        // +, -, *, /, %, etc.
    ComparisonOp,        // ==, !=, <, >, etc.
    PropertyRead,        // obj.prop
    PropertyWrite,       // obj.prop = val
    MethodCall,          // obj.method()
    ToStringCall,        // .toString(), .toFixed(), etc.
    DynamicAssign,       // Assigned from await, fetch, unknown
    AssignedToVar,       // other = this_var
    AssignedFromVar,     // this_var = other
    Returned,            // return this_var
    CapturedInClosure,   // Used in nested function
    Awaited,             // await this_var
    CoReturned,          // co_return this_var
    StoredInGlobal,      // global.push(this_var)
    Indexed,             // arr[i]
    Iterated,            // for (x of arr)
    Spread,              // [...arr]
    NumericLiteralAssign, // Assigned numeric literal
    FloatLiteralAssign,   // Assigned float literal
    IntLiteralAssign,     // Assigned int literal
    StringLiteralAssign,  // Assigned string literal
    BoolLiteralAssign,    // Assigned bool literal
    NullAssign,           // Assigned null
    UndefinedAssign,      // Assigned undefined
}

pub struct EscapeAnalyzer {
    escapes: HashMap<String, EscapeKind>,
    widens: HashMap<String, WidenedType>,
    var_infos: HashMap<String, VarInfo>,
    async_functions: HashSet<String>,
    closure_vars: HashMap<String, HashSet<String>>,
    current_function: Option<String>,
    current_scope_depth: usize,
    global_vars: HashSet<String>,
    function_signatures: HashMap<String, FunctionSignature>,
}

#[derive(Debug, Clone)]
pub struct FunctionSignature {
    pub name: String,
    pub is_async: bool,
    pub params: Vec<(String, Option<String>)>,
    pub return_type: Option<String>,
}

impl EscapeAnalyzer {
    pub fn new() -> Self {
        Self {
            escapes: HashMap::new(),
            widens: HashMap::new(),
            var_infos: HashMap::new(),
            async_functions: HashSet::new(),
            closure_vars: HashMap::new(),
            current_function: None,
            current_scope_depth: 0,
            global_vars: HashSet::new(),
            function_signatures: HashMap::new(),
        }
    }

    pub fn analyze_program(&mut self, program: &Program) -> AnalysisResult {
        self.collect_signatures(program);
        
        for stmt in &program.body {
            self.analyze_statement(stmt);
        }
        
        self.resolve_cross_function_escapes();
        
        AnalysisResult {
            escapes: self.escapes.clone(),
            widens: self.widens.clone(),
            var_infos: self.var_infos.clone(),
            async_functions: self.async_functions.clone(),
        }
    }

    fn collect_signatures(&mut self, program: &Program) {
        for stmt in &program.body {
            match stmt {
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        let name = id.name.to_string();
                        let is_async = f.r#async;
                        let params = f.params.items.iter().filter_map(|p| {
                            let (name, _) = self.binding_to_identifier(&p.pattern);
                            let type_ann = p.type_annotation.as_ref().map(|ta| {
                                self.type_annotation_to_string(ta)
                            });
                            Some((name, type_ann))
                        }).collect();
                        let return_type = f.return_type.as_ref().map(|rt| {
                            self.type_annotation_to_string(rt)
                        });
                        self.function_signatures.insert(name.clone(), FunctionSignature {
                            name: name.clone(),
                            is_async,
                            params,
                            return_type,
                        });
                        if is_async {
                            self.async_functions.insert(name);
                        }
                    }
                }
                Statement::VariableDeclaration(d) if self.current_scope_depth == 0 => {
                    for decl in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &decl.id {
                            self.global_vars.insert(id.name.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn analyze_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::FunctionDeclaration(f) => self.analyze_function(f),
            Statement::VariableDeclaration(d) => self.analyze_variable_declaration(d),
            Statement::ExpressionStatement(e) => self.analyze_expression(&e.expression),
            Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    self.analyze_expression(arg);
                    if let Expression::Identifier(id) = arg {
                        self.mark_escape(&id.name, EscapeKind::Return);
                    }
                }
            }
            Statement::BlockStatement(b) => {
                self.current_scope_depth += 1;
                for s in &b.body {
                    self.analyze_statement(s);
                }
                self.current_scope_depth -= 1;
            }
            Statement::IfStatement(i) => {
                self.analyze_expression(&i.test);
                self.analyze_statement(&i.consequent);
                if let Some(alt) = &i.alternate {
                    self.analyze_statement(alt);
                }
            }
            Statement::WhileStatement(w) => {
                self.analyze_expression(&w.test);
                self.analyze_statement(&w.body);
            }
            Statement::ForStatement(f) => {
                if let Some(init) = &f.init {
                    self.analyze_for_init(init);
                }
                if let Some(test) = &f.test {
                    self.analyze_expression(test);
                }
                if let Some(update) = &f.update {
                    self.analyze_expression(update);
                }
                self.analyze_statement(&f.body);
            }
            Statement::ForOfStatement(f) => {
                self.analyze_expression(&f.right);
                if let ForStatementLeft::VariableDeclaration(d) = &f.left {
                    for decl in &d.declarations {
                        if let BindingPattern::BindingIdentifier(id) = &decl.id {
                            self.record_usage(&id.name, UsageKind::Iterated);
                        }
                    }
                }
                self.analyze_statement(&f.body);
            }
            Statement::ForInStatement(f) => {
                self.analyze_expression(&f.right);
                self.analyze_statement(&f.body);
            }
            Statement::TryStatement(t) => {
                self.analyze_block(&t.block);
                if let Some(handler) = &t.handler {
                    self.analyze_block(&handler.body);
                }
                if let Some(finalizer) = &t.finalizer {
                    self.analyze_block(finalizer);
                }
            }
            _ => {}
        }
    }

    fn analyze_for_init(&mut self, init: &ForStatementInit) {
        match init {
            ForStatementInit::VariableDeclaration(d) => {
                for decl in &d.declarations {
                    if let Some(init_expr) = &decl.init {
                        self.analyze_expression(init_expr);
                    }
                }
            }
            _ => {}
        }
    }

    fn analyze_function(&mut self, f: &Function) {
        let name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or_default();
        let is_async = f.r#async;
        let old_function = self.current_function.clone();
        self.current_function = Some(name.clone());
        
        if is_async {
            self.async_functions.insert(name.clone());
        }

        for param in &f.params.items {
            let (param_name, _) = self.binding_to_identifier(&param.pattern);
            let type_ann = param.type_annotation.as_ref().map(|ta| {
                self.type_annotation_to_string(ta)
            });
            self.record_var(VarInfo {
                name: param_name.clone(),
                annotated_type: type_ann.clone(),
                escape_kind: EscapeKind::None,
                widened_type: WidenedType::None,
                is_mutable: false,
                usages: vec![],
            });
        }

        if let Some(body) = &f.body {
            self.analyze_function_body(body);
        }

        self.current_function = old_function;
    }

    fn analyze_function_body(&mut self, body: &FunctionBody) {
        self.current_scope_depth += 1;
        for stmt in &body.statements {
            self.analyze_statement(stmt);
        }
        self.current_scope_depth -= 1;
    }

    fn analyze_block(&mut self, block: &BlockStatement) {
        self.current_scope_depth += 1;
        for stmt in &block.body {
            self.analyze_statement(stmt);
        }
        self.current_scope_depth -= 1;
    }

    fn analyze_variable_declaration(&mut self, d: &VariableDeclaration) {
        for decl in &d.declarations {
            let (name, _) = self.binding_to_identifier(&decl.id);
            let type_ann = decl.type_annotation.as_ref().map(|ta| {
                self.type_annotation_to_string(ta)
            });
            
            let is_global = self.current_scope_depth == 0;
            
            let mut var_info = VarInfo {
                name: name.clone(),
                annotated_type: type_ann.clone(),
                escape_kind: if is_global { EscapeKind::Global } else { EscapeKind::None },
                widened_type: WidenedType::None,
                is_mutable: d.kind == VariableDeclarationKind::Let,
                usages: vec![],
            };

            if let Some(init) = &decl.init {
                self.analyze_expression(init);
                if self.is_dynamic_source(init) {
                    var_info.widened_type = WidenedType::ToJsNumber;
                    self.widens.insert(name.clone(), WidenedType::ToJsNumber);
                }
                // Track async arrow assigned to var: let f = async (...) => ...
                if let Expression::ArrowFunctionExpression(arrow) = init {
                    if arrow.r#async {
                        self.async_functions.insert(name.clone());
                    }
                }
                // Track async function expression assigned to var
                if let Expression::FunctionExpression(func) = init {
                    if func.r#async {
                        self.async_functions.insert(name.clone());
                    }
                }
            }

            self.var_infos.insert(name.clone(), var_info);
        }
    }

    fn analyze_expression(&mut self, expr: &Expression) {
        match expr {
            Expression::Identifier(id) => {
                self.record_usage(&id.name, UsageKind::PropertyRead);
            }
            Expression::CallExpression(c) => {
                self.analyze_call_expression(c);
            }
            Expression::AwaitExpression(a) => {
                self.analyze_expression(&a.argument);
                if let Expression::Identifier(id) = &a.argument {
                    self.mark_escape(&id.name, EscapeKind::AsyncBoundary);
                    self.record_usage(&id.name, UsageKind::Awaited);
                }
            }
            Expression::BinaryExpression(b) => {
                self.analyze_expression(&b.left);
                self.analyze_expression(&b.right);
                if let Expression::Identifier(id) = &b.left {
                    self.record_usage(&id.name, UsageKind::ArithmeticOp);
                }
                if let Expression::Identifier(id) = &b.right {
                    self.record_usage(&id.name, UsageKind::ArithmeticOp);
                }
            }
            Expression::AssignmentExpression(a) => {
                self.analyze_assignment(a);
            }
            Expression::StaticMemberExpression(m) => {
                self.analyze_expression(&m.object);
                if let Expression::Identifier(id) = &m.object {
                    self.record_usage(&id.name, UsageKind::PropertyRead);
                }
            }
            Expression::ComputedMemberExpression(m) => {
                self.analyze_expression(&m.object);
                self.analyze_expression(&m.expression);
                if let Expression::Identifier(id) = &m.object {
                    self.record_usage(&id.name, UsageKind::Indexed);
                }
            }
            Expression::ArrowFunctionExpression(f) => {
                self.analyze_arrow_function(f);
            }
            Expression::FunctionExpression(f) => {
                self.analyze_function_expression(f);
            }
            Expression::NewExpression(n) => {
                for arg in &n.arguments {
                    self.analyze_argument(arg);
                }
            }
            Expression::ObjectExpression(o) => {
                for prop in &o.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = prop {
                        self.analyze_expression(&p.value);
                    }
                }
            }
            Expression::ArrayExpression(a) => {
                for el in &a.elements {
                    if let Some(expr) = el.as_expression() {
                        self.analyze_expression(expr);
                    }
                }
            }
            _ => {}
        }
    }

    fn analyze_argument(&mut self, arg: &Argument) {
        if let Some(expr) = arg.as_expression() {
            self.analyze_expression(expr);
        }
    }

    fn analyze_call_expression(&mut self, c: &CallExpression) {
        if let Expression::StaticMemberExpression(m) = &c.callee {
            if let Expression::Identifier(obj) = &m.object {
                let obj_name = obj.name.to_string();
                self.record_usage(&obj_name, UsageKind::MethodCall);
                let method = m.property.name.as_str();
                if matches!(method, "toString" | "toFixed" | "toPrecision" | "toLocaleString") {
                    self.record_usage(&obj_name, UsageKind::ToStringCall);
                    self.widens.insert(obj_name, WidenedType::ToJsString);
                }
            }
        }

        if let Expression::Identifier(id) = &c.callee {
            if id.name.as_str() == "fetch" {
                // fetch() return is dynamic - handled in assignment
            }
        }

        for arg in &c.arguments {
            self.analyze_argument(arg);
        }
    }

    fn analyze_assignment(&mut self, a: &AssignmentExpression) {
        if let AssignmentTarget::AssignmentTargetIdentifier(left_id) = &a.left {
            let left_name = left_id.name.to_string();
            self.record_usage(&left_name, UsageKind::PropertyWrite);
            
            if self.is_dynamic_source(&a.right) {
                self.widens.insert(left_name.clone(), WidenedType::ToJsNumber);
                self.record_usage(&left_name, UsageKind::DynamicAssign);
            }
        }

        self.analyze_expression(&a.right);

        if let AssignmentTarget::AssignmentTargetIdentifier(left_id) = &a.left {
            if let Expression::Identifier(right_id) = &a.right {
                self.mark_escape(&left_id.name, EscapeKind::MultipleRefs);
                self.mark_escape(&right_id.name, EscapeKind::MultipleRefs);
                self.record_usage(&left_id.name, UsageKind::AssignedFromVar);
                self.record_usage(&right_id.name, UsageKind::AssignedToVar);
            }
        }
    }

    fn analyze_arrow_function(&mut self, f: &ArrowFunctionExpression) {
        // Find captured variables
        let mut captured = HashSet::new();
        
        // ArrowFunctionExpressionBody can be either an expression or a block
        if let Some(expr) = f.body.as_expression() {
            self.find_captured_vars_in_expr(expr, &mut captured);
        } else if let Some(block) = f.body.as_function_body() {
            for stmt in &block.statements {
                self.find_captured_vars_in_stmt(stmt, &mut captured);
            }
        }
        
        for var in &captured {
            self.mark_escape(var, EscapeKind::ClosureCapture);
            self.closure_vars.entry(self.current_function.clone().unwrap_or_default())
                .or_default()
                .insert(var.clone());
        }

        if let Some(body) = f.body.as_expression() {
            self.analyze_expression(body);
        } else if let Some(body) = f.body.as_function_body() {
            self.analyze_function_body(body);
        }
    }

    fn analyze_function_expression(&mut self, f: &Function) {
        self.analyze_function(f);
    }

    fn find_captured_vars_in_expr(&self, expr: &Expression, captured: &mut HashSet<String>) {
        match expr {
            Expression::Identifier(id) => {
                if self.var_infos.contains_key(id.name.as_str()) {
                    captured.insert(id.name.to_string());
                }
            }
            Expression::CallExpression(c) => {
                for arg in &c.arguments {
                    self.find_captured_vars_in_arg(arg, captured);
                }
            }
            Expression::ArrowFunctionExpression(f) => {
                if let Some(body) = f.body.as_expression() {
                    self.find_captured_vars_in_expr(body, captured);
                }
            }
            Expression::FunctionExpression(f) => {
                if let Some(body) = &f.body {
                    for stmt in &body.statements {
                        self.find_captured_vars_in_stmt(stmt, captured);
                    }
                }
            }
            Expression::BinaryExpression(b) => {
                self.find_captured_vars_in_expr(&b.left, captured);
                self.find_captured_vars_in_expr(&b.right, captured);
            }
            Expression::AssignmentExpression(a) => {
                self.find_captured_vars_in_assignment_target(&a.left, captured);
                self.find_captured_vars_in_expr(&a.right, captured);
            }
            Expression::StaticMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
            }
            Expression::ComputedMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
                self.find_captured_vars_in_expr(&m.expression, captured);
            }
            Expression::ObjectExpression(o) => {
                for prop in &o.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = prop {
                        self.find_captured_vars_in_expr(&p.value, captured);
                    }
                }
            }
            Expression::ArrayExpression(a) => {
                for el in &a.elements {
                    if let Some(expr) = el.as_expression() {
                        self.find_captured_vars_in_expr(expr, captured);
                    }
                }
            }
            Expression::AwaitExpression(a) => {
                self.find_captured_vars_in_expr(&a.argument, captured);
            }
            Expression::ConditionalExpression(c) => {
                self.find_captured_vars_in_expr(&c.test, captured);
                self.find_captured_vars_in_expr(&c.consequent, captured);
                self.find_captured_vars_in_expr(&c.alternate, captured);
            }
            Expression::ParenthesizedExpression(p) => {
                self.find_captured_vars_in_expr(&p.expression, captured);
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_arg(&self, arg: &Argument, captured: &mut HashSet<String>) {
        if let Some(expr) = arg.as_expression() {
            self.find_captured_vars_in_expr(expr, captured);
        }
    }

    fn find_captured_vars_in_assignment_target(&self, target: &AssignmentTarget, captured: &mut HashSet<String>) {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                if self.var_infos.contains_key(id.name.as_str()) {
                    captured.insert(id.name.to_string());
                }
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
                self.find_captured_vars_in_expr(&m.expression, captured);
            }
            AssignmentTarget::ArrayAssignmentTarget(a) => {
                for el in &a.elements {
                    if let Some(pattern) = el {
                        // pattern is a BindingPattern or similar
                    }
                }
            }
            AssignmentTarget::ObjectAssignmentTarget(o) => {
                for prop in &o.properties {
                    // prop is AssignmentTargetProperty
                }
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_binding_pattern(&self, pattern: &BindingPattern, captured: &mut HashSet<String>) {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                if self.var_infos.contains_key(id.name.as_str()) {
                    captured.insert(id.name.to_string());
                }
            }
            BindingPattern::AssignmentPattern(a) => {
                self.find_captured_vars_in_binding_pattern(&a.left, captured);
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_stmt(&self, stmt: &Statement, captured: &mut HashSet<String>) {
        match stmt {
            Statement::ExpressionStatement(e) => self.find_captured_vars_in_expr(&e.expression, captured),
            Statement::VariableDeclaration(d) => {
                for decl in &d.declarations {
                    if let Some(init) = &decl.init {
                        self.find_captured_vars_in_expr(init, captured);
                    }
                }
            }
            Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    self.find_captured_vars_in_expr(arg, captured);
                }
            }
            Statement::IfStatement(i) => {
                self.find_captured_vars_in_expr(&i.test, captured);
                self.find_captured_vars_in_stmt(&i.consequent, captured);
                if let Some(alt) = &i.alternate {
                    self.find_captured_vars_in_stmt(alt, captured);
                }
            }
            Statement::BlockStatement(b) => {
                for s in &b.body {
                    self.find_captured_vars_in_stmt(s, captured);
                }
            }
            Statement::WhileStatement(w) => {
                self.find_captured_vars_in_expr(&w.test, captured);
                self.find_captured_vars_in_stmt(&w.body, captured);
            }
            Statement::ForStatement(f) => {
                if let Some(init) = &f.init {
                    if let ForStatementInit::VariableDeclaration(d) = init {
                        for decl in &d.declarations {
                            if let Some(init_expr) = &decl.init {
                                self.find_captured_vars_in_expr(init_expr, captured);
                            }
                        }
                    }
                }
                if let Some(test) = &f.test {
                    self.find_captured_vars_in_expr(test, captured);
                }
                if let Some(update) = &f.update {
                    self.find_captured_vars_in_expr(update, captured);
                }
                self.find_captured_vars_in_stmt(&f.body, captured);
            }
            Statement::ForOfStatement(f) => {
                self.find_captured_vars_in_expr(&f.right, captured);
                self.find_captured_vars_in_stmt(&f.body, captured);
            }
            Statement::ForInStatement(f) => {
                self.find_captured_vars_in_expr(&f.right, captured);
                self.find_captured_vars_in_stmt(&f.body, captured);
            }
            Statement::TryStatement(t) => {
                self.find_captured_vars_in_block(&t.block, captured);
                if let Some(handler) = &t.handler {
                    self.find_captured_vars_in_block(&handler.body, captured);
                }
                if let Some(finalizer) = &t.finalizer {
                    self.find_captured_vars_in_block(finalizer, captured);
                }
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_block(&self, block: &BlockStatement, captured: &mut HashSet<String>) {
        for stmt in &block.body {
            self.find_captured_vars_in_stmt(stmt, captured);
        }
    }

    fn get_outer_scope_vars(&self) -> Vec<String> {
        self.var_infos.keys().cloned().collect()
    }

    fn is_dynamic_source(&self, expr: &Expression) -> bool {
        match expr {
            Expression::CallExpression(c) => {
                if let Expression::Identifier(id) = &c.callee {
                    matches!(id.name.as_str(), "fetch" | "JSON.parse" | "parseInt" | "parseFloat")
                } else {
                    false
                }
            }
            Expression::AwaitExpression(_) => true,
            Expression::Identifier(id) => {
                self.widens.get(id.name.as_str()).is_some()
            }
            _ => false,
        }
    }

    fn mark_escape(&mut self, name: &str, kind: EscapeKind) {
        let current = self.escapes.get(name).cloned().unwrap_or(EscapeKind::None);
        self.escapes.insert(name.to_string(), EscapeKind::max(current, kind));
    }

    fn record_usage(&mut self, name: &str, usage: UsageKind) {
        if let Some(info) = self.var_infos.get_mut(name) {
            info.usages.push(usage);
        }
    }

    fn record_var(&mut self, info: VarInfo) {
        self.var_infos.insert(info.name.clone(), info);
    }

    fn binding_to_identifier(&self, pat: &BindingPattern) -> (String, Option<String>) {
        match pat {
            BindingPattern::BindingIdentifier(id) => (id.name.to_string(), None),
            BindingPattern::AssignmentPattern(a) => self.binding_to_identifier(&a.left),
            _ => ("/* pattern */".to_string(), None),
        }
    }

    fn type_annotation_to_string(&self, _ta: &TSTypeAnnotation) -> String {
        // Simplified - would need full type resolution
        // For now, return empty string
        String::new()
    }

    fn resolve_cross_function_escapes(&mut self) {
        let mut changed = true;
        while changed {
            changed = false;
            for (_, sig) in &self.function_signatures {
                if sig.is_async {
                    for (param_name, _) in &sig.params {
                        if self.escapes.get(param_name) == Some(&EscapeKind::None) {
                            if let Some(info) = self.var_infos.get(param_name) {
                                if info.usages.iter().any(|u| matches!(u, UsageKind::Awaited | UsageKind::CoReturned)) {
                                    self.escapes.insert(param_name.clone(), EscapeKind::AsyncBoundary);
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl EscapeKind {
    fn max(a: EscapeKind, b: EscapeKind) -> EscapeKind {
        use EscapeKind::*;
        match (a, b) {
            (AsyncBoundary, _) | (_, AsyncBoundary) => AsyncBoundary,
            (ClosureCapture, _) | (_, ClosureCapture) => ClosureCapture,
            (MultipleRefs, _) | (_, MultipleRefs) => MultipleRefs,
            (Global, _) | (_, Global) => Global,
            (Return, _) | (_, Return) => Return,
            (None, None) => None,
        }
    }
}

pub struct AnalysisResult {
    pub escapes: HashMap<String, EscapeKind>,
    pub widens: HashMap<String, WidenedType>,
    pub var_infos: HashMap<String, VarInfo>,
    pub async_functions: HashSet<String>,
}

impl Default for EscapeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}