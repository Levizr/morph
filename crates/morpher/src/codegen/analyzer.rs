// Semantic analyzer for intent-based codegen
// Performs escape analysis, type widening, async boundary detection, and closure capture detection

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;
use std::collections::{HashMap, HashSet};

use super::context::TypeMode;
use super::js_comparison::{ComparisonKind, ComparisonSignature, OperandClass};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EscapeKind {
    None,           // Stack allocation - doesn't escape
    Return,         // Returned from function - unique_ptr + move
    Global,         // Stored in global/container - unique_ptr + move
    ClosureCapture, // Captured by closure - shared_ptr
    MultipleRefs,   // Assigned to multiple vars - shared_ptr
    AsyncBoundary,  // Crosses await/co_return - shared_ptr
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidenedType {
    None,       // Keep native type
    ToJsNumber, // int/float -> JsNumber (handles bigint/overflow)
    ToJsString, // string -> JsString (if .toString() called)
    ToJsValue,  // any -> JsValue (completely dynamic)
    ToJsArray,  // array -> JsArray (if array methods like push, length used)
}

#[derive(Debug, Clone)]
pub struct VarInfo {
    pub name: String,
    pub annotated_type: Option<String>,
    pub escape_kind: EscapeKind,
    pub widened_type: WidenedType,
    pub is_mutable: bool,
    pub usages: Vec<UsageKind>,
    pub init_operand_class: OperandClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UsageKind {
    ArithmeticOp,         // +, -, *, /, %, etc.
    ComparisonOp,         // ==, !=, <, >, etc.
    PropertyRead,         // obj.prop
    PropertyWrite,        // obj.prop = val
    MethodCall,           // obj.method()
    ToStringCall,         // .toString(), .toFixed(), etc.
    DynamicAssign,        // Assigned from await, fetch, unknown
    AssignedToVar,        // other = this_var
    AssignedFromVar,      // this_var = other
    Returned,             // return this_var
    CapturedInClosure,    // Used in nested function
    Awaited,              // await this_var
    CoReturned,           // co_return this_var
    StoredInGlobal,       // global.push(this_var)
    Indexed,              // arr[i]
    Iterated,             // for (x of arr)
    Spread,               // [...arr]
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
    comparison_signatures: HashSet<ComparisonSignature>,
    pending_identifier_inits: Vec<(String, String)>,
    type_mode: TypeMode,
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
            comparison_signatures: HashSet::new(),
            pending_identifier_inits: Vec::new(),
            type_mode: TypeMode::Infer,
        }
    }

    /// Set the type resolution mode so parameter classification matches codegen.
    pub fn set_type_mode(&mut self, type_mode: TypeMode) {
        self.type_mode = type_mode;
    }

    pub fn analyze_program(&mut self, program: &Program) -> AnalysisResult {
        self.collect_signatures(program);

        for stmt in &program.body {
            self.analyze_statement(stmt);
        }

        // Detect chaining patterns and mark variables for JsString
        self.detect_chaining(program);

        self.resolve_cross_function_escapes();
        self.resolve_identifier_init_classes();

        AnalysisResult {
            escapes: self.escapes.clone(),
            widens: self.widens.clone(),
            var_infos: self.var_infos.clone(),
            async_functions: self.async_functions.clone(),
            comparison_signatures: self.comparison_signatures.iter().cloned().collect(),
        }
    }

    /// Detect chaining patterns (e.g., `s.toUpperCase().toLowerCase()`)
    /// and mark the base variables to use JsString instead of native types
    fn detect_chaining(&mut self, program: &Program) {
        for stmt in &program.body {
            self.detect_chaining_in_stmt(stmt);
        }
    }

    fn detect_chaining_in_stmt(&mut self, stmt: &Statement) {
        match stmt {
            Statement::ExpressionStatement(es) => {
                self.detect_chaining_in_expr(&es.expression);
            }
            Statement::VariableDeclaration(d) => {
                for decl in &d.declarations {
                    if let Some(init) = &decl.init {
                        self.detect_chaining_in_expr(init);
                    }
                }
            }
            Statement::ReturnStatement(r) => {
                if let Some(arg) = &r.argument {
                    self.detect_chaining_in_expr(arg);
                }
            }
            Statement::IfStatement(i) => {
                self.detect_chaining_in_expr(&i.test);
                self.detect_chaining_in_stmt(&i.consequent);
                if let Some(alt) = &i.alternate {
                    self.detect_chaining_in_stmt(alt);
                }
            }
            Statement::BlockStatement(b) => {
                for s in &b.body {
                    self.detect_chaining_in_stmt(s);
                }
            }
            Statement::WhileStatement(w) => {
                self.detect_chaining_in_expr(&w.test);
                self.detect_chaining_in_stmt(&w.body);
            }
            Statement::ForStatement(f) => {
                if let Some(init) = &f.init {
                    self.detect_chaining_in_for_init(init);
                }
                if let Some(test) = &f.test {
                    self.detect_chaining_in_expr(test);
                }
                if let Some(update) = &f.update {
                    self.detect_chaining_in_expr(update);
                }
                self.detect_chaining_in_stmt(&f.body);
            }
            Statement::ForOfStatement(f) => {
                self.detect_chaining_in_expr(&f.right);
                self.detect_chaining_in_stmt(&f.body);
            }
            Statement::ForInStatement(f) => {
                self.detect_chaining_in_expr(&f.right);
                self.detect_chaining_in_stmt(&f.body);
            }
            Statement::TryStatement(t) => {
                for s in &t.block.body {
                    self.detect_chaining_in_stmt(s);
                }
                if let Some(h) = &t.handler {
                    for s in &h.body.body {
                        self.detect_chaining_in_stmt(s);
                    }
                }
                if let Some(fin) = &t.finalizer {
                    for s in &fin.body {
                        self.detect_chaining_in_stmt(s);
                    }
                }
            }
            _ => {}
        }
    }

    fn detect_chaining_in_for_init(&mut self, init: &ForStatementInit) {
        if let ForStatementInit::VariableDeclaration(d) = init {
            for decl in &d.declarations {
                if let Some(init_expr) = &decl.init {
                    self.detect_chaining_in_expr(init_expr);
                }
            }
        }
    }

    /// Check if an expression contains a chaining pattern:
    /// A CallExpression (method call) whose result is used as the object of another StaticMemberExpression
    fn detect_chaining_in_expr(&mut self, expr: &Expression) {
        // Look for StaticMemberExpression where the object is a CallExpression
        // on an Identifier - this indicates chaining
        match expr {
            Expression::StaticMemberExpression(m) => {
                if let Expression::CallExpression(inner_call) = &m.object {
                    if let Expression::StaticMemberExpression(inner_method) = &inner_call.callee {
                        if let Expression::Identifier(base_id) = &inner_method.object {
                            let base_name = base_id.name.to_string();
                            // Mark the base variable as needing JsString for chaining
                            if self.var_infos.contains_key(&base_name) {
                                self.widens.insert(base_name, WidenedType::ToJsString);
                            }
                        }
                    }
                }
                // Recurse into object
                self.detect_chaining_in_expr(&m.object);
            }
            Expression::CallExpression(c) => {
                // Recurse into callee and arguments
                // (chaining itself is detected via the StaticMemberExpression case above)
                self.detect_chaining_in_expr(&c.callee);
                for arg in &c.arguments {
                    self.detect_chaining_in_arg(arg);
                }
            }
            Expression::AssignmentExpression(a) => {
                self.detect_chaining_in_expr(&a.right);
                // Assignment target doesn't chain
            }
            Expression::BinaryExpression(b) => {
                self.detect_chaining_in_expr(&b.left);
                self.detect_chaining_in_expr(&b.right);
            }
            Expression::LogicalExpression(b) => {
                self.detect_chaining_in_expr(&b.left);
                self.detect_chaining_in_expr(&b.right);
            }
            Expression::ConditionalExpression(c) => {
                self.detect_chaining_in_expr(&c.test);
                self.detect_chaining_in_expr(&c.consequent);
                self.detect_chaining_in_expr(&c.alternate);
            }
            Expression::AwaitExpression(a) => {
                self.detect_chaining_in_expr(&a.argument);
            }
            Expression::ArrowFunctionExpression(f) => {
                if let Some(body) = f.body.as_expression() {
                    self.detect_chaining_in_expr(body);
                } else if let Some(body) = f.body.as_function_body() {
                    for stmt in &body.statements {
                        self.detect_chaining_in_stmt(stmt);
                    }
                }
            }
            Expression::FunctionExpression(f) => {
                if let Some(body) = &f.body {
                    for stmt in &body.statements {
                        self.detect_chaining_in_stmt(stmt);
                    }
                }
            }
            Expression::ObjectExpression(o) => {
                for prop in &o.properties {
                    if let ObjectPropertyKind::ObjectProperty(p) = prop {
                        self.detect_chaining_in_expr(&p.value);
                    }
                }
            }
            Expression::ArrayExpression(a) => {
                for el in &a.elements {
                    if let Some(expr) = el.as_expression() {
                        self.detect_chaining_in_expr(expr);
                    }
                }
            }
            Expression::ParenthesizedExpression(p) => {
                self.detect_chaining_in_expr(&p.expression);
            }
            Expression::TSAsExpression(a) => {
                self.detect_chaining_in_expr(&a.expression);
            }
            Expression::TSSatisfiesExpression(s) => {
                self.detect_chaining_in_expr(&s.expression);
            }
            Expression::TSNonNullExpression(n) => {
                self.detect_chaining_in_expr(&n.expression);
            }
            Expression::TSInstantiationExpression(x) => {
                self.detect_chaining_in_expr(&x.expression);
            }
            _ => {}
        }
    }

    fn detect_chaining_in_arg(&mut self, arg: &Argument) {
        if let Some(expr) = arg.as_expression() {
            self.detect_chaining_in_expr(expr);
        }
    }

    fn collect_signatures(&mut self, program: &Program) {
        for stmt in &program.body {
            match stmt {
                Statement::FunctionDeclaration(f) => {
                    if let Some(id) = &f.id {
                        let name = id.name.to_string();
                        let is_async = f.r#async;
                        let params = f
                            .params
                            .items
                            .iter()
                            .filter_map(|p| {
                                let (name, _) = self.binding_to_identifier(&p.pattern);
                                let type_ann = p
                                    .type_annotation
                                    .as_ref()
                                    .map(|ta| self.type_annotation_to_string(ta));
                                Some((name, type_ann))
                            })
                            .collect();
                        let return_type =
                            f.return_type.as_ref().map(|rt| self.type_annotation_to_string(rt));
                        self.function_signatures.insert(
                            name.clone(),
                            FunctionSignature { name: name.clone(), is_async, params, return_type },
                        );
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
                self.record_truthy_test(&i.test);
                self.analyze_expression(&i.test);
                self.analyze_statement(&i.consequent);
                if let Some(alt) = &i.alternate {
                    self.analyze_statement(alt);
                }
            }
            Statement::WhileStatement(w) => {
                self.record_truthy_test(&w.test);
                self.analyze_expression(&w.test);
                self.analyze_statement(&w.body);
            }
            Statement::DoWhileStatement(d) => {
                self.record_truthy_test(&d.test);
                self.analyze_expression(&d.test);
                self.analyze_statement(&d.body);
            }
            Statement::ForStatement(f) => {
                if let Some(init) = &f.init {
                    self.analyze_for_init(init);
                }
                if let Some(test) = &f.test {
                    self.record_truthy_test(test);
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
            let type_ann =
                param.type_annotation.as_ref().map(|ta| self.type_annotation_to_string(ta));
            let param_class = match self.type_mode {
                TypeMode::Strict => type_ann
                    .as_deref()
                    .map(super::js_comparison::ts_annotation_to_class)
                    .unwrap_or(OperandClass::Other),
                TypeMode::Infer => OperandClass::JsValue,
            };
            self.record_var(VarInfo {
                name: param_name.clone(),
                annotated_type: type_ann.clone(),
                escape_kind: EscapeKind::None,
                widened_type: WidenedType::None,
                is_mutable: false,
                usages: vec![],
                init_operand_class: param_class,
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
            let type_ann =
                decl.type_annotation.as_ref().map(|ta| self.type_annotation_to_string(ta));

            let is_global = self.current_scope_depth == 0;

            let mut var_info = VarInfo {
                name: name.clone(),
                annotated_type: type_ann.clone(),
                escape_kind: if is_global { EscapeKind::Global } else { EscapeKind::None },
                widened_type: WidenedType::None,
                is_mutable: d.kind == VariableDeclarationKind::Let,
                usages: vec![],
                init_operand_class: OperandClass::JsValue,
            };

            if let Some(init) = &decl.init {
                self.analyze_expression(init);
                var_info.init_operand_class = self.infer_expression_class(init);
                if let Expression::Identifier(target_id) = init {
                    if target_id.name.as_str() != name {
                        self.pending_identifier_inits
                            .push((name.clone(), target_id.name.to_string()));
                    }
                }
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
                if let Some(kind) = ComparisonKind::from_binary_operator(b.operator.as_str()) {
                    let left_class = self.infer_expression_class(&b.left);
                    let right_class = self.infer_expression_class(&b.right);
                    self.record_comparison(kind, left_class, right_class);
                    if let Expression::Identifier(id) = &b.left {
                        self.record_usage(&id.name, UsageKind::ComparisonOp);
                    }
                    if let Expression::Identifier(id) = &b.right {
                        self.record_usage(&id.name, UsageKind::ComparisonOp);
                    }
                }
            }
            Expression::LogicalExpression(l) => {
                self.analyze_expression(&l.left);
                self.analyze_expression(&l.right);
                if matches!(l.operator.as_str(), "&&" | "||") {
                    self.record_truthy_test(&l.left);
                    self.record_truthy_test(&l.right);
                }
            }
            Expression::UnaryExpression(u) => {
                self.analyze_expression(&u.argument);
                if u.operator.as_str() == "!" {
                    self.record_truthy_test(&u.argument);
                }
            }
            Expression::ConditionalExpression(c) => {
                self.record_truthy_test(&c.test);
                self.analyze_expression(&c.test);
                self.analyze_expression(&c.consequent);
                self.analyze_expression(&c.alternate);
            }
            Expression::AssignmentExpression(a) => {
                self.analyze_assignment(a);
            }
            Expression::StaticMemberExpression(m) => {
                self.analyze_expression(&m.object);
                if let Expression::Identifier(id) = &m.object {
                    let obj_name = id.name.to_string();
                    self.record_usage(&obj_name, UsageKind::PropertyRead);
                    if m.property.name.as_str() == "length" {
                        self.widens.insert(obj_name, WidenedType::ToJsArray);
                    }
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
                if matches!(
                    method,
                    "toString"
                        | "toFixed"
                        | "toPrecision"
                        | "toLocaleString"
                        | "toUpperCase"
                        | "toLowerCase"
                        | "charAt"
                        | "indexOf"
                        | "lastIndexOf"
                        | "substring"
                        | "substr"
                        | "slice"
                        | "trim"
                        | "trimStart"
                        | "trimEnd"
                        | "replace"
                        | "replaceAll"
                        | "split"
                        | "match"
                        | "matchAll"
                        | "search"
                        | "padStart"
                        | "padEnd"
                        | "repeat"
                        | "startsWith"
                        | "endsWith"
                        | "includes"
                        | "localeCompare"
                        | "normalize"
                        | "toLocaleUpperCase"
                        | "toLocaleLowerCase"
                ) {
                    self.record_usage(&obj_name, UsageKind::ToStringCall);
                    self.widens.insert(obj_name.clone(), WidenedType::ToJsString);
                }
                // Array methods that require JsArray
                if matches!(
                    method,
                    "push"
                        | "pop"
                        | "shift"
                        | "unshift"
                        | "splice"
                        | "slice"
                        | "map"
                        | "filter"
                        | "forEach"
                        | "reduce"
                        | "find"
                        | "indexOf"
                        | "includes"
                        | "join"
                        | "reverse"
                        | "sort"
                        | "fill"
                        | "copyWithin"
                        | "flat"
                        | "flatMap"
                ) {
                    self.widens.insert(obj_name.clone(), WidenedType::ToJsArray);
                }
            }
        }

        // Also handle .length property access on arrays
        if let Expression::StaticMemberExpression(m) = &c.callee {
            if let Expression::Identifier(obj) = &m.object {
                let obj_name = obj.name.to_string();
                if m.property.name.as_str() == "length" {
                    self.widens.insert(obj_name, WidenedType::ToJsArray);
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
            self.closure_vars
                .entry(self.current_function.clone().unwrap_or_default())
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

    fn find_captured_vars_in_assignment_target(
        &self,
        target: &AssignmentTarget,
        captured: &mut HashSet<String>,
    ) {
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

    fn find_captured_vars_in_binding_pattern(
        &self,
        pattern: &BindingPattern,
        captured: &mut HashSet<String>,
    ) {
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
            Statement::ExpressionStatement(e) => {
                self.find_captured_vars_in_expr(&e.expression, captured)
            }
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

    /// Resolve `let alias = target` chains to a fixpoint so forward and chained
    /// identifier initializers pick up the target's operand class.
    fn resolve_identifier_init_classes(&mut self) {
        for _ in 0..self.pending_identifier_inits.len().saturating_add(1) {
            let mut progressed = false;
            let pending = std::mem::take(&mut self.pending_identifier_inits);
            let mut remaining = Vec::with_capacity(pending.len());
            for (var_name, target_name) in pending {
                let target_class = self
                    .var_infos
                    .get(&target_name)
                    .map(|info| self.apply_widening(&target_name, info.init_operand_class))
                    .unwrap_or(OperandClass::JsValue);
                if target_class == OperandClass::JsValue {
                    remaining.push((var_name, target_name));
                    continue;
                }
                if let Some(var_info) = self.var_infos.get_mut(&var_name) {
                    if var_info.init_operand_class == OperandClass::JsValue {
                        var_info.init_operand_class = target_class;
                        progressed = true;
                    }
                }
            }
            self.pending_identifier_inits = remaining;
            if !progressed {
                break;
            }
        }
    }

    fn apply_widening(&self, var_name: &str, base_class: OperandClass) -> OperandClass {
        match self.widens.get(var_name) {
            Some(WidenedType::ToJsNumber) => OperandClass::JsNumber,
            Some(WidenedType::ToJsString) => OperandClass::JsString,
            Some(WidenedType::ToJsValue) => OperandClass::JsValue,
            Some(WidenedType::ToJsArray) => OperandClass::JsArray,
            _ => base_class,
        }
    }

    /// Classify an expression into the operand class its emitted C++ value has.
    fn infer_expression_class(&self, expr: &Expression) -> OperandClass {
        match expr {
            Expression::BooleanLiteral(_) => OperandClass::Boolean,
            Expression::NullLiteral(_) => OperandClass::Null,
            Expression::StringLiteral(_) => OperandClass::Text,
            Expression::TemplateLiteral(_) => OperandClass::Text,
            Expression::NumericLiteral(literal) => {
                if literal.value.fract() == 0.0 {
                    OperandClass::Integer
                } else {
                    OperandClass::Float
                }
            }
            Expression::BigIntLiteral(_) => OperandClass::Integer,
            Expression::Identifier(id) => match id.name.as_str() {
                "undefined" => OperandClass::Undefined,
                "null" => OperandClass::Null,
                "NaN" | "Infinity" => OperandClass::Float,
                name => self
                    .var_infos
                    .get(name)
                    .map(|info| self.apply_widening(name, info.init_operand_class))
                    .unwrap_or(OperandClass::JsValue),
            },
            Expression::ArrayExpression(_) => OperandClass::JsArray,
            Expression::ObjectExpression(_) => OperandClass::JsObject,
            Expression::CallExpression(_)
            | Expression::AwaitExpression(_)
            | Expression::YieldExpression(_) => OperandClass::JsValue,
            Expression::UnaryExpression(unary) => match unary.operator.as_str() {
                "!" | "delete" => OperandClass::Boolean,
                "typeof" => OperandClass::Text,
                "void" => OperandClass::Undefined,
                "-" | "+" => OperandClass::Float,
                "~" => OperandClass::Integer,
                _ => OperandClass::JsValue,
            },
            Expression::BinaryExpression(binary) => {
                if ComparisonKind::from_binary_operator(binary.operator.as_str()).is_some() {
                    OperandClass::Boolean
                } else {
                    let left_class = self.infer_expression_class(&binary.left);
                    let right_class = self.infer_expression_class(&binary.right);
                    if left_class == OperandClass::Float || right_class == OperandClass::Float {
                        OperandClass::Float
                    } else if left_class.is_textual() || right_class.is_textual() {
                        OperandClass::Text
                    } else {
                        OperandClass::Integer
                    }
                }
            }
            Expression::LogicalExpression(_) => OperandClass::Boolean,
            Expression::ConditionalExpression(conditional) => {
                let consequent_class = self.infer_expression_class(&conditional.consequent);
                let alternate_class = self.infer_expression_class(&conditional.alternate);
                if consequent_class == alternate_class {
                    consequent_class
                } else {
                    OperandClass::Other
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.infer_expression_class(&parenthesized.expression)
            }
            Expression::TSAsExpression(cast) => self.infer_expression_class(&cast.expression),
            Expression::TSSatisfiesExpression(cast) => {
                self.infer_expression_class(&cast.expression)
            }
            Expression::TSNonNullExpression(cast) => self.infer_expression_class(&cast.expression),
            _ => OperandClass::JsValue,
        }
    }

    fn record_comparison(&mut self, kind: ComparisonKind, left: OperandClass, right: OperandClass) {
        self.comparison_signatures.insert(ComparisonSignature::binary(kind, left, right));
    }

    fn record_truthy_test(&mut self, expr: &Expression) {
        let operand_class = self.infer_expression_class(expr);
        self.comparison_signatures.insert(ComparisonSignature::truthy(operand_class));
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
            Expression::Identifier(id) => self.widens.get(id.name.as_str()).is_some(),
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
                                if info.usages.iter().any(|u| {
                                    matches!(u, UsageKind::Awaited | UsageKind::CoReturned)
                                }) {
                                    self.escapes
                                        .insert(param_name.clone(), EscapeKind::AsyncBoundary);
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
    pub comparison_signatures: Vec<ComparisonSignature>,
}

impl Default for EscapeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
