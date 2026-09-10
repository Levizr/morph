// Semantic analyzer for intent-based codegen
// Performs escape analysis, type widening, async boundary detection, and closure capture detection

use oxc_ast::ast::*;
use oxc_span::GetSpan;
use std::collections::{HashMap, HashSet};

use super::context::TypeMode;
use super::js_comparison::{ComparisonKind, ComparisonSignature, OperandClass};
use super::string_methods::StringMethodHandler;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EscapeKind {
    None,           // Stack allocation - doesn't escape
    Return,         // Returned from function - unique_ptr + move
    Global,         // Stored in global/container - unique_ptr + move
    ClosureCapture, // Captured by closure - shared_ptr
    MultipleRefs,   // Assigned to multiple vars - shared_ptr
    AsyncBoundary,  // Crosses await/co_return - shared_ptr
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WidenedType {
    None,       // Keep native type
    ToJsNumber, // int/float -> JsNumber (handles bigint/overflow)
    ToJsString, // string -> JsString (if .toString() called)
    ToJsValue,  // any -> JsValue (completely dynamic)
    ToJsArray,  // array -> JsArray (if array methods like push, length used)
}

/// One program point: statement order plus the nesting that scopes it.
/// Branch and function depth make region reasoning sound without a CFG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StmtSite {
    pub index: usize,
    pub branch_depth: usize,
    pub func_depth: usize,
}

/// A definition: where it happened, in which region, and what it assigned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefSite {
    pub site: StmtSite,
    pub value_class: OperandClass,
    pub int_literal: Option<i64>,
    pub span_start: Option<u32>,
}

/// One narrowed region: redeclare from this assignment onward under a fresh
/// name. Keyed by (variable, assignment span start) at emission.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NarrowSplit {
    pub var_name: String,
    pub def_span_start: u32,
    pub narrowed_name: String,
    pub narrowed_type: String,
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
    pub int_range: Option<(i64, i64)>,
    pub int_range_exact: bool,
    pub def_sites: Vec<DefSite>,
    pub use_sites: Vec<StmtSite>,
    pub has_loop_use: bool,
    pub decl_branch_depth: usize,
    pub decl_func_depth: usize,
}

impl VarInfo {
    /// Index of the first statement that defines this variable, if any.
    pub fn first_defined_at(&self) -> Option<usize> {
        self.def_sites.first().map(|def| def.site.index)
    }

    /// Index of the last statement that reads this variable, if any.
    pub fn last_used_at(&self) -> Option<usize> {
        self.use_sites.last().map(|site| site.index)
    }

    /// True when some recorded read executes after the given statement.
    /// Conservative: any later-indexed read counts, including loop bodies.
    pub fn is_live_after(&self, index: usize) -> bool {
        self.use_sites.iter().any(|site| site.index > index)
    }

    /// Statement indices of definitions, in walk order.
    pub fn def_indices(&self) -> Vec<usize> {
        self.def_sites.iter().map(|def| def.site.index).collect()
    }

    /// Statement indices of reads, in walk order.
    pub fn use_indices(&self) -> Vec<usize> {
        self.use_sites.iter().map(|site| site.index).collect()
    }

    /// Number of recorded reads. Writes, paired call tags, and dynamic-assign
    /// markers are excluded so a single read counts exactly once.
    pub fn read_use_count(&self) -> usize {
        self.usages
            .iter()
            .filter(|usage| {
                !matches!(
                    usage,
                    UsageKind::PropertyWrite
                        | UsageKind::AssignedFromVar
                        | UsageKind::DynamicAssign
                        | UsageKind::ToStringCall
                )
            })
            .count()
    }
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
    statement_index: usize,
    loop_depth: usize,
    branch_depth: usize,
    func_depth: usize,
    narrow_splits: Vec<NarrowSplit>,
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
            statement_index: 0,
            loop_depth: 0,
            branch_depth: 0,
            func_depth: 0,
            narrow_splits: Vec::new(),
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
        self.compute_narrow_splits();

        AnalysisResult {
            escapes: self.escapes.clone(),
            widens: self.widens.clone(),
            var_infos: self.var_infos.clone(),
            async_functions: self.async_functions.clone(),
            comparison_signatures: self.comparison_signatures.iter().cloned().collect(),
            closure_captures: self.closure_vars.clone(),
            narrow_splits: self.narrow_splits.clone(),
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

    /// A method chain needs a JsString base only when some link lacks a
    /// native `morph::str` helper. Known string methods nest natively, and
    /// `.length` lowers to `.size()` on any native receiver.
    fn chain_needs_js_string(inner_method: &str, outer_method: &str) -> bool {
        if inner_method == "length" || outer_method == "length" {
            return false;
        }
        !StringMethodHandler::is_string_method(inner_method)
            || !StringMethodHandler::is_string_method(outer_method)
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
                            let inner_name = inner_method.property.name.as_str();
                            let outer_name = m.property.name.as_str();
                            if Self::chain_needs_js_string(inner_name, outer_name)
                                && self.var_infos.contains_key(&base_name)
                            {
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
        self.statement_index += 1;
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
                self.branch_depth += 1;
                self.analyze_statement(&i.consequent);
                if let Some(alt) = &i.alternate {
                    self.analyze_statement(alt);
                }
                self.branch_depth -= 1;
            }
            Statement::WhileStatement(w) => {
                self.record_truthy_test(&w.test);
                self.loop_depth += 1;
                self.analyze_expression(&w.test);
                self.analyze_statement(&w.body);
                self.loop_depth -= 1;
            }
            Statement::DoWhileStatement(d) => {
                self.record_truthy_test(&d.test);
                self.loop_depth += 1;
                self.analyze_expression(&d.test);
                self.analyze_statement(&d.body);
                self.loop_depth -= 1;
            }
            Statement::ForStatement(f) => {
                if let Some(init) = &f.init {
                    self.analyze_for_init(init);
                }
                self.loop_depth += 1;
                if let Some(test) = &f.test {
                    self.record_truthy_test(test);
                    self.analyze_expression(test);
                }
                if let Some(update) = &f.update {
                    self.analyze_expression(update);
                }
                self.detect_bounded_loop_counter(f);
                self.analyze_statement(&f.body);
                self.loop_depth -= 1;
            }
            Statement::ForOfStatement(f) => {
                self.analyze_expression(&f.right);
                self.record_for_target(&f.left);
                self.loop_depth += 1;
                self.analyze_statement(&f.body);
                self.loop_depth -= 1;
            }
            Statement::ForInStatement(f) => {
                self.analyze_expression(&f.right);
                self.record_for_target(&f.left);
                self.loop_depth += 1;
                self.analyze_statement(&f.body);
                self.loop_depth -= 1;
            }
            Statement::TryStatement(t) => {
                self.branch_depth += 1;
                self.analyze_block(&t.block);
                if let Some(handler) = &t.handler {
                    self.analyze_block(&handler.body);
                }
                if let Some(finalizer) = &t.finalizer {
                    self.analyze_block(finalizer);
                }
                self.branch_depth -= 1;
            }
            Statement::ThrowStatement(t) => {
                self.analyze_expression(&t.argument);
            }
            Statement::SwitchStatement(s) => {
                self.analyze_expression(&s.discriminant);
                self.branch_depth += 1;
                for case in &s.cases {
                    if let Some(test) = &case.test {
                        self.analyze_expression(test);
                    }
                    for stmt in &case.consequent {
                        self.analyze_statement(stmt);
                    }
                }
                self.branch_depth -= 1;
            }
            Statement::LabeledStatement(l) => {
                self.branch_depth += 1;
                self.analyze_statement(&l.body);
                self.branch_depth -= 1;
            }
            Statement::ClassDeclaration(c) => {
                self.analyze_class_declaration(c);
            }
            Statement::ExportDefaultDeclaration(e) => match &e.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(f) => self.analyze_function(f),
                ExportDefaultDeclarationKind::ClassDeclaration(c) => {
                    self.analyze_class_declaration(c);
                }
                _ => {
                    if let Some(expr) = e.declaration.as_expression() {
                        self.analyze_expression(expr);
                    }
                }
            },
            _ => {}
        }
    }

    /// Walk a class body for outer-variable uses. Method bodies are deferred
    /// code, so their outer reads also capture; fields, statics, decorators,
    /// and heritage run at definition time and only count as uses.
    fn analyze_class_declaration(&mut self, class: &Class) {
        if let Some(heritage) = &class.heritage {
            self.analyze_expression(&heritage.expression);
        }
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    for decorator in &method.decorators {
                        self.analyze_expression(&decorator.expression);
                    }
                    self.analyze_computed_key(&method.key);
                    self.analyze_class_method(&method.value);
                }
                ClassElement::PropertyDefinition(prop) => {
                    for decorator in &prop.decorators {
                        self.analyze_expression(&decorator.expression);
                    }
                    self.analyze_computed_key(&prop.key);
                    if let Some(value) = &prop.value {
                        self.analyze_expression(value);
                    }
                }
                ClassElement::StaticBlock(block) => {
                    for stmt in &block.body {
                        self.analyze_statement(stmt);
                    }
                }
                _ => {}
            }
        }
    }

    /// Record an identifier used as a computed class/object key.
    fn analyze_computed_key(&mut self, key: &PropertyKey) {
        if let PropertyKey::Identifier(id) = key {
            self.record_usage(id.name.as_str(), UsageKind::PropertyRead);
        }
    }

    /// Walk one method: defaults and heritage-style reads count as uses,
    /// while body reads of outer variables capture (the method may outlive
    /// its definition scope). Bindings stay unrecorded — method locals must
    /// not overwrite outer entries.
    fn analyze_class_method(&mut self, method: &Function) {
        for param in &method.params.items {
            self.analyze_pattern_defaults(&param.pattern);
            if let Some(default_value) = &param.initializer {
                self.analyze_expression(default_value);
            }
        }
        self.func_depth += 1;
        let mut captured = HashSet::new();
        self.find_captured_vars_in_function_body(method, &mut captured);
        for var in &captured {
            self.mark_escape(var, EscapeKind::ClosureCapture);
            self.closure_vars
                .entry(self.current_function.clone().unwrap_or_default())
                .or_default()
                .insert(var.clone());
        }
        if let Some(body) = &method.body {
            self.current_scope_depth += 1;
            for stmt in &body.statements {
                self.analyze_statement(stmt);
            }
            self.current_scope_depth -= 1;
        }
        self.func_depth -= 1;
    }

    /// Visit default-value expressions in a binding pattern. Bindings
    /// themselves declare locals and are never captures.
    fn analyze_pattern_defaults(&mut self, pattern: &BindingPattern) {
        match pattern {
            BindingPattern::AssignmentPattern(assign) => {
                self.analyze_expression(&assign.right);
                self.analyze_pattern_defaults(&assign.left);
            }
            BindingPattern::ObjectPattern(object) => {
                for prop in &object.properties {
                    self.analyze_pattern_defaults(&prop.value);
                }
                if let Some(rest) = &object.rest {
                    self.analyze_pattern_defaults(&rest.argument);
                }
            }
            BindingPattern::ArrayPattern(array) => {
                for element in &array.elements {
                    if let Some(nested) = element {
                        self.analyze_pattern_defaults(nested);
                    }
                }
                if let Some(rest) = &array.rest {
                    self.analyze_pattern_defaults(&rest.argument);
                }
            }
            _ => {}
        }
    }

    /// Bound a classic `for (let i = 0; i < 100; i++)` induction variable.
    ///
    /// Runs after the update was analyzed (which marks the counter unbounded),
    /// restoring an exact range when init, bound, and step are all literals.
    fn detect_bounded_loop_counter(&mut self, for_stmt: &ForStatement) {
        let Some((counter_name, init_value)) = Self::loop_counter_init(&for_stmt.init) else {
            return;
        };
        let Some(bound_value) = Self::loop_counter_bound(&for_stmt.test, &counter_name) else {
            return;
        };
        if !Self::loop_counter_steps(&for_stmt.update, &counter_name) {
            return;
        }
        let low = init_value.min(bound_value);
        let high = init_value.max(bound_value);
        if let Some(info) = self.var_infos.get_mut(counter_name.as_str()) {
            info.int_range = Some((low, high));
            info.int_range_exact = true;
        }
    }

    fn loop_counter_init(init: &Option<ForStatementInit>) -> Option<(String, i64)> {
        let ForStatementInit::VariableDeclaration(declaration) = init.as_ref()? else {
            return None;
        };
        let declarator = declaration.declarations.first()?;
        let BindingPattern::BindingIdentifier(identifier) = &declarator.id else {
            return None;
        };
        let init_value = Self::int_literal_value(declarator.init.as_ref()?)?;
        Some((identifier.name.to_string(), init_value))
    }

    fn loop_counter_bound(test: &Option<Expression>, counter_name: &str) -> Option<i64> {
        let Expression::BinaryExpression(binary) = test.as_ref()? else {
            return None;
        };
        let (identifier_side, literal_side, flipped) = match (&binary.left, &binary.right) {
            (Expression::Identifier(id), literal) => (id, literal, false),
            (literal, Expression::Identifier(id)) => (id, literal, true),
            _ => return None,
        };
        if identifier_side.name.as_str() != counter_name {
            return None;
        }
        let literal_value = Self::int_literal_value(literal_side)?;
        let operator = if flipped {
            Self::mirror_comparison(binary.operator.as_str())?
        } else {
            binary.operator.as_str()
        };
        match operator {
            "<" => Some(literal_value.saturating_sub(1)),
            "<=" => Some(literal_value),
            ">" => Some(literal_value.saturating_add(1)),
            ">=" => Some(literal_value),
            _ => None,
        }
    }

    fn mirror_comparison(operator: &str) -> Option<&str> {
        match operator {
            "<" => Some(">"),
            "<=" => Some(">="),
            ">" => Some("<"),
            ">=" => Some("<="),
            _ => None,
        }
    }

    fn loop_counter_steps(update: &Option<Expression>, counter_name: &str) -> bool {
        if let Some(Expression::UpdateExpression(step)) = update {
            if let SimpleAssignmentTarget::AssignmentTargetIdentifier(target) = &step.argument {
                return target.name.as_str() == counter_name
                    && matches!(step.operator.as_str(), "++" | "--");
            }
        }
        false
    }

    fn analyze_for_init(&mut self, init: &ForStatementInit) {
        match init {
            ForStatementInit::VariableDeclaration(d) => {
                for decl in &d.declarations {
                    let (name, _) = self.binding_to_identifier(&decl.id);
                    let type_ann =
                        decl.type_annotation.as_ref().map(|ta| self.type_annotation_to_string(ta));
                    if let Some(init_expr) = &decl.init {
                        self.analyze_expression(init_expr);
                    }
                    let var_info = self.fresh_var_info(
                        name.clone(),
                        type_ann,
                        self.current_scope_depth == 0,
                        true,
                    );
                    self.var_infos.insert(name.clone(), var_info);
                    if let Some(init_expr) = &decl.init {
                        self.track_variable_init(&name, init_expr);
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
        self.func_depth += 1;

        if is_async {
            self.async_functions.insert(name.clone());
        }

        for param in &f.params.items {
            let (param_name, _) = self.binding_to_identifier(&param.pattern);
            self.analyze_pattern_defaults(&param.pattern);
            if let Some(default_value) = &param.initializer {
                self.analyze_expression(default_value);
            }
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
                int_range: None,
                int_range_exact: false,
                def_sites: Vec::new(),
                use_sites: Vec::new(),
                has_loop_use: false,
                decl_branch_depth: self.branch_depth,
                decl_func_depth: self.func_depth,
            });
            self.record_def(&param_name, param_class, None, Some(param.span.start));
        }

        if let Some(body) = &f.body {
            self.analyze_function_body(body);
        }

        self.func_depth -= 1;
        self.current_function = old_function;
    }

    fn analyze_function_body(&mut self, body: &FunctionBody) {
        self.current_scope_depth += 1;
        for stmt in &body.statements {
            self.analyze_statement(stmt);
        }
        self.current_scope_depth -= 1;
    }

    /// Record a for-of/for-in target rebinding an outer variable.
    fn record_for_target(&mut self, left: &ForStatementLeft) {
        match left {
            ForStatementLeft::VariableDeclaration(d) => {
                for decl in &d.declarations {
                    if let BindingPattern::BindingIdentifier(id) = &decl.id {
                        self.record_def(id.name.as_str(), OperandClass::Other, None, None);
                    }
                }
            }
            ForStatementLeft::AssignmentTargetIdentifier(id) => {
                self.note_unbounded_native_mutation(id.name.as_str());
                self.record_def(id.name.as_str(), OperandClass::Other, None, None);
            }
            _ => {}
        }
    }

    fn analyze_block(&mut self, block: &BlockStatement) {
        self.current_scope_depth += 1;
        for stmt in &block.body {
            self.analyze_statement(stmt);
        }
        self.current_scope_depth -= 1;
    }

    fn fresh_var_info(
        &self,
        name: String,
        annotated_type: Option<String>,
        is_global: bool,
        is_mutable: bool,
    ) -> VarInfo {
        VarInfo {
            name,
            annotated_type,
            escape_kind: if is_global { EscapeKind::Global } else { EscapeKind::None },
            widened_type: WidenedType::None,
            is_mutable,
            usages: vec![],
            init_operand_class: OperandClass::JsValue,
            int_range: None,
            int_range_exact: true,
            def_sites: Vec::new(),
            use_sites: Vec::new(),
            has_loop_use: false,
            decl_branch_depth: self.branch_depth,
            decl_func_depth: self.func_depth,
        }
    }

    /// Fold an initializer into a freshly recorded variable: operand class,
    /// integer range, identifier chaining, and dynamic widening.
    ///
    /// Integer ranges adopt eagerly: `let b = a` copies `a`'s current value,
    /// so a proven range transfers soundly. Later mutations fold on top.
    fn track_variable_init(&mut self, name: &str, init: &Expression) {
        let init_class = self.infer_expression_class(init);
        let int_value = Self::int_literal_value(init);
        let dynamic = self.is_dynamic_source(init);
        let adopted_range = if let Expression::Identifier(target_id) = init {
            self.var_infos
                .get(target_id.name.as_str())
                .filter(|target| target.int_range_exact)
                .and_then(|target| target.int_range)
        } else {
            None
        };
        let init_span = init.span().start;
        if let Some(info) = self.var_infos.get_mut(name) {
            info.init_operand_class = init_class;
            if let Some(literal_value) = int_value {
                info.int_range = Some((literal_value, literal_value));
            } else if let Some((low, high)) = adopted_range {
                info.int_range = Some((low, high));
            } else {
                info.int_range_exact = false;
            }
            if dynamic {
                info.widened_type = WidenedType::ToJsNumber;
            }
        }
        self.record_def(name, init_class, int_value, Some(init_span));
        if dynamic {
            self.widens.insert(name.to_string(), WidenedType::ToJsNumber);
        }
        if let Expression::Identifier(target_id) = init {
            if target_id.name.as_str() != name {
                self.pending_identifier_inits.push((name.to_string(), target_id.name.to_string()));
            }
        }
    }

    fn analyze_variable_declaration(&mut self, d: &VariableDeclaration) {
        for decl in &d.declarations {
            if let Some(init) = &decl.init {
                self.analyze_expression(init);
            }
            match &decl.id {
                BindingPattern::BindingIdentifier(id) => {
                    self.analyze_identifier_declaration(
                        &id.name,
                        decl.type_annotation.as_ref().map(|ta| self.type_annotation_to_string(ta)),
                        self.current_scope_depth == 0,
                        d.kind == VariableDeclarationKind::Let,
                        decl.init.as_ref(),
                    );
                }
                pattern => {
                    self.analyze_destructured_pattern(
                        pattern,
                        decl.init.as_ref(),
                        self.current_scope_depth == 0,
                        d.kind == VariableDeclarationKind::Let,
                    );
                }
            }
        }
    }

    /// Record one plain identifier declaration: annotation, operand class,
    /// integer range, identifier chaining, and dynamic widening.
    fn analyze_identifier_declaration(
        &mut self,
        name: &str,
        type_ann: Option<String>,
        is_global: bool,
        is_mutable: bool,
        init: Option<&Expression>,
    ) {
        let var_info = self.fresh_var_info(name.to_string(), type_ann, is_global, is_mutable);
        self.var_infos.insert(name.to_string(), var_info);
        if let Some(init) = init {
            self.track_variable_init(name, init);
            // Track async arrow assigned to var: let f = async (...) => ...
            if let Expression::ArrowFunctionExpression(arrow) = init {
                if arrow.r#async {
                    self.async_functions.insert(name.to_string());
                }
            }
            // Track async function expression assigned to var
            if let Expression::FunctionExpression(func) = init {
                if func.r#async {
                    self.async_functions.insert(name.to_string());
                }
            }
        }
    }

    /// Record every name bound by a destructuring declaration. Elements
    /// taken from a literal initializer keep full precision (integer
    /// ranges included); anything else stays boxed, exactly as if the
    /// element had been read off an unknown value.
    fn analyze_destructured_pattern(
        &mut self,
        pattern: &BindingPattern,
        init: Option<&Expression>,
        is_global: bool,
        is_mutable: bool,
    ) {
        for (name, element) in Self::destructure_bindings(pattern, init) {
            let var_info = self.fresh_var_info(name.clone(), None, is_global, is_mutable);
            self.var_infos.insert(name.clone(), var_info);
            match element {
                Some(expr) => {
                    self.track_variable_init(&name, expr);
                    if matches!(expr, Expression::ArrowFunctionExpression(arrow) if arrow.r#async)
                        || matches!(expr, Expression::FunctionExpression(func) if func.r#async)
                    {
                        self.async_functions.insert(name.clone());
                    }
                }
                None => {
                    self.record_def(&name, OperandClass::JsValue, None, None);
                    if let Some(info) = self.var_infos.get_mut(&name) {
                        info.init_operand_class = OperandClass::JsValue;
                        info.int_range_exact = false;
                    }
                }
            }
        }
    }

    /// Bound names with the initializer expression each one reads, when a
    /// literal initializer pins it down. Anything unresolvable (rest
    /// elements, computed keys, unknown sources) binds with no expression.
    fn destructure_bindings<'ast>(
        pattern: &'ast BindingPattern<'ast>,
        init: Option<&'ast Expression<'ast>>,
    ) -> Vec<(String, Option<&'ast Expression<'ast>>)> {
        // A bare defaulted shape proves nothing about the container.
        let root_init =
            if matches!(pattern, BindingPattern::AssignmentPattern(_)) { None } else { init };
        let mut out = Vec::new();
        Self::collect_bindings(pattern, root_init, &mut out);
        out
    }

    /// Walk one pattern level, resolving element expressions against a
    /// literal container when one is available.
    fn collect_bindings<'ast>(
        pattern: &'ast BindingPattern<'ast>,
        init: Option<&'ast Expression<'ast>>,
        out: &mut Vec<(String, Option<&'ast Expression<'ast>>)>,
    ) {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                out.push((id.name.to_string(), init));
            }
            BindingPattern::ObjectPattern(object) => {
                for prop in &object.properties {
                    if prop.computed {
                        Self::collect_bindings(&prop.value, None, out);
                        continue;
                    }
                    let key = Self::pattern_key_name(&prop.key);
                    let sub_init =
                        key.as_deref().and_then(|key| Self::object_init_value(init, key));
                    Self::collect_default_bindings(&prop.value, sub_init, out);
                }
                if let Some(rest) = &object.rest {
                    Self::collect_bindings(&rest.argument, None, out);
                }
            }
            BindingPattern::ArrayPattern(array) => {
                for (index, element) in array.elements.iter().enumerate() {
                    let Some(nested) = element else {
                        continue;
                    };
                    let sub_init = Self::array_init_value(init, index);
                    Self::collect_default_bindings(nested, sub_init, out);
                }
                if let Some(rest) = &array.rest {
                    Self::collect_bindings(&rest.argument, None, out);
                }
            }
            BindingPattern::AssignmentPattern(assign) => {
                Self::collect_default_bindings(&assign.left, init, out);
            }
            _ => {}
        }
    }

    /// Bind a pattern position that may carry a default (`{x = 5}`). A
    /// default proves nothing statically — the runtime value can come
    /// from the initializer instead — so only an initializer-provided
    /// expression carries precision; otherwise the binding stays boxed.
    fn collect_default_bindings<'ast>(
        pattern: &'ast BindingPattern<'ast>,
        init: Option<&'ast Expression<'ast>>,
        out: &mut Vec<(String, Option<&'ast Expression<'ast>>)>,
    ) {
        if let BindingPattern::AssignmentPattern(assign) = pattern {
            Self::collect_bindings(&assign.left, init, out);
            return;
        }
        Self::collect_bindings(pattern, init, out);
    }

    /// A static property key as written (`{x}`, `{"x"}`, `{0}`).
    pub(crate) fn pattern_key_name(key: &PropertyKey) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
            PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
            PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        }
    }

    /// The value expression for `key` when the initializer is an object
    /// literal that defines it.
    pub(crate) fn object_init_value<'ast>(
        init: Option<&'ast Expression<'ast>>,
        key: &str,
    ) -> Option<&'ast Expression<'ast>> {
        let Expression::ObjectExpression(object) = init? else {
            return None;
        };
        for prop in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = prop else {
                continue;
            };
            if Self::pattern_key_name(&property.key).as_deref() != Some(key) {
                continue;
            }
            return Some(&property.value);
        }
        None
    }

    /// The element expression at `index` when the initializer is an array
    /// literal long enough to have one.
    pub(crate) fn array_init_value<'ast>(
        init: Option<&'ast Expression<'ast>>,
        index: usize,
    ) -> Option<&'ast Expression<'ast>> {
        let Expression::ArrayExpression(array) = init? else {
            return None;
        };
        array.elements.get(index)?.as_expression()
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
                if matches!(l.operator.as_str(), "&&" | "||") {
                    self.record_truthy_test(&l.left);
                    self.record_truthy_test(&l.right);
                }
                self.branch_depth += 1;
                self.analyze_expression(&l.left);
                self.analyze_expression(&l.right);
                self.branch_depth -= 1;
            }
            Expression::UnaryExpression(u) => {
                self.analyze_expression(&u.argument);
                if u.operator.as_str() == "!" {
                    self.record_truthy_test(&u.argument);
                }
            }
            Expression::UpdateExpression(update) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(target_id) =
                    &update.argument
                {
                    let target_name = target_id.name.to_string();
                    self.note_unbounded_native_mutation(&target_name);
                    self.record_usage(&target_name, UsageKind::ArithmeticOp);
                    let current_class = self
                        .var_infos
                        .get(target_name.as_str())
                        .map(|info| info.init_operand_class)
                        .unwrap_or(OperandClass::Other);
                    self.record_def(&target_name, current_class, None, Some(update.span.start));
                }
            }
            Expression::ConditionalExpression(c) => {
                self.record_truthy_test(&c.test);
                self.analyze_expression(&c.test);
                self.branch_depth += 1;
                self.analyze_expression(&c.consequent);
                self.analyze_expression(&c.alternate);
                self.branch_depth -= 1;
            }
            Expression::AssignmentExpression(a) => {
                self.analyze_assignment(a);
            }
            Expression::StaticMemberExpression(m) => {
                self.analyze_expression(&m.object);
            }
            Expression::ComputedMemberExpression(m) => {
                self.analyze_expression(&m.object);
                self.analyze_expression(&m.expression);
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
                    if !StringMethodHandler::is_string_method(method) {
                        self.widens.insert(obj_name.clone(), WidenedType::ToJsString);
                    }
                }
                // Array methods that require JsArray. Methods that also exist
                // on strings (slice/indexOf/includes) only widen non-text
                // receivers; split is string-only and never widens here.
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
                ) && self.infer_expression_class(&m.object) != OperandClass::Text
                {
                    self.widens.insert(obj_name.clone(), WidenedType::ToJsArray);
                }
            }
        }

        // A `.length()` *call* still needs a receiver with a length() method,
        // so non-text receivers widen. Plain `.length` reads need nothing:
        // the emitter lowers those to `.size()` universally.
        if let Expression::StaticMemberExpression(m) = &c.callee {
            if let Expression::Identifier(obj) = &m.object {
                let obj_name = obj.name.to_string();
                if m.property.name.as_str() == "length"
                    && self.infer_expression_class(&m.object) != OperandClass::Text
                {
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
            let assigned_class = self.infer_expression_class(&a.right);
            // Only a plain `=` re-establishes a proven literal. A compound
            // `+= 1` keeps the old value in play, so it must never narrow.
            let assigned_literal =
                if a.operator.as_str() == "=" { Self::int_literal_value(&a.right) } else { None };
            self.record_def(&left_name, assigned_class, assigned_literal, Some(a.span.start));

            if self.is_dynamic_source(&a.right) {
                self.widens.insert(left_name.clone(), WidenedType::ToJsNumber);
                self.record_usage(&left_name, UsageKind::DynamicAssign);
            }
            // Reassignment from an await boundary fixes the declared type
            // afterwards, so the declaration must already account for it.
            // (Declarations deduce `await` directly and need no widening.)
            if matches!(&a.right, Expression::AwaitExpression(_)) {
                self.widens.insert(left_name.clone(), WidenedType::ToJsNumber);
                self.record_usage(&left_name, UsageKind::DynamicAssign);
            }
            self.fold_assignment_range(&left_name, a.operator.as_str(), &a.right);
        } else {
            self.analyze_assignment_target_reads(&a.left);
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

    /// Record reads through a non-identifier assignment target. `a[i] = v`
    /// and `o.x = v` both touch their base object, so later moves must see it.
    fn analyze_assignment_target_reads(&mut self, target: &AssignmentTarget) {
        match target {
            AssignmentTarget::ComputedMemberExpression(m) => {
                self.analyze_expression(&m.object);
                self.analyze_expression(&m.expression);
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                self.analyze_expression(&m.object);
            }
            _ => {}
        }
    }

    /// Fold an assignment into the variable's observed integer range.
    ///
    /// Plain `=` with an integer literal extends the proven range. Anything
    /// else (compound ops, non-literals, dynamic sources) marks the range
    /// inexact, which keeps the emitter on `int64_t` instead of `int32_t`.
    fn fold_assignment_range(&mut self, var_name: &str, operator: &str, right: &Expression) {
        if operator != "=" {
            self.note_unbounded_native_mutation(var_name);
            return;
        }
        if let Expression::Identifier(source_id) = right {
            let source_range = self
                .var_infos
                .get(source_id.name.as_str())
                .and_then(|info| if info.int_range_exact { info.int_range } else { None });
            if let Some((low, high)) = source_range {
                self.extend_int_range(var_name, low.min(high), low.max(high));
            } else {
                self.note_unbounded_native_mutation(var_name);
            }
            return;
        }
        if let Some(literal_value) = Self::int_literal_value(right) {
            self.extend_int_range(var_name, literal_value, literal_value);
        } else {
            self.note_unbounded_native_mutation(var_name);
        }
    }

    /// Extract an `i64` from an integer numeric literal, if it fits.
    fn int_literal_value(expr: &Expression) -> Option<i64> {
        if let Expression::NumericLiteral(literal) = expr {
            if literal.value.fract() == 0.0
                && literal.value >= i64::MIN as f64
                && literal.value <= i64::MAX as f64
            {
                return Some(literal.value as i64);
            }
        }
        None
    }

    fn extend_int_range(&mut self, var_name: &str, low: i64, high: i64) {
        if let Some(info) = self.var_infos.get_mut(var_name) {
            if !info.int_range_exact {
                return;
            }
            let (current_low, current_high) = info.int_range.unwrap_or((low, high));
            info.int_range = Some((current_low.min(low), current_high.max(high)));
        }
    }

    fn note_unbounded_native_mutation(&mut self, var_name: &str) {
        if let Some(info) = self.var_infos.get_mut(var_name) {
            info.int_range_exact = false;
        }
    }

    fn analyze_arrow_function(&mut self, f: &ArrowFunctionExpression) {
        for param in &f.params.items {
            self.analyze_pattern_defaults(&param.pattern);
            if let Some(default_value) = &param.initializer {
                self.analyze_expression(default_value);
            }
        }
        self.func_depth += 1;
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
        self.func_depth -= 1;
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
                self.find_captured_vars_in_expr(&c.callee, captured);
                for arg in &c.arguments {
                    self.find_captured_vars_in_arg(arg, captured);
                }
            }
            Expression::ArrowFunctionExpression(f) => {
                if let Some(body) = f.body.as_expression() {
                    self.find_captured_vars_in_expr(body, captured);
                } else if let Some(block) = f.body.as_function_body() {
                    for stmt in &block.statements {
                        self.find_captured_vars_in_stmt(stmt, captured);
                    }
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
            Expression::LogicalExpression(l) => {
                self.find_captured_vars_in_expr(&l.left, captured);
                self.find_captured_vars_in_expr(&l.right, captured);
            }
            Expression::UnaryExpression(u) => {
                self.find_captured_vars_in_expr(&u.argument, captured);
            }
            Expression::UpdateExpression(update) => {
                self.find_captured_vars_in_simple_target(&update.argument, captured);
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
                    match prop {
                        ObjectPropertyKind::ObjectProperty(p) => {
                            self.find_captured_vars_in_expr(&p.value, captured);
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.find_captured_vars_in_expr(&spread.argument, captured);
                        }
                    }
                }
            }
            Expression::ArrayExpression(a) => {
                for el in &a.elements {
                    if let ArrayExpressionElement::SpreadElement(spread) = el {
                        self.find_captured_vars_in_expr(&spread.argument, captured);
                    } else if let Some(expr) = el.as_expression() {
                        self.find_captured_vars_in_expr(expr, captured);
                    }
                }
            }
            Expression::NewExpression(n) => {
                self.find_captured_vars_in_expr(&n.callee, captured);
                for arg in &n.arguments {
                    self.find_captured_vars_in_arg(arg, captured);
                }
            }
            Expression::SequenceExpression(s) => {
                for expr in &s.expressions {
                    self.find_captured_vars_in_expr(expr, captured);
                }
            }
            Expression::TemplateLiteral(t) => {
                for expr in &t.expressions {
                    self.find_captured_vars_in_expr(expr, captured);
                }
            }
            Expression::TaggedTemplateExpression(t) => {
                self.find_captured_vars_in_expr(&t.tag, captured);
                for expr in &t.quasi.expressions {
                    self.find_captured_vars_in_expr(expr, captured);
                }
            }
            Expression::ChainExpression(chain) => {
                self.find_captured_vars_in_chain(&chain.expression, captured);
            }
            Expression::PrivateFieldExpression(p) => {
                self.find_captured_vars_in_expr(&p.object, captured);
            }
            Expression::AwaitExpression(a) => {
                self.find_captured_vars_in_expr(&a.argument, captured);
            }
            Expression::YieldExpression(y) => {
                if let Some(argument) = &y.argument {
                    self.find_captured_vars_in_expr(argument, captured);
                }
            }
            Expression::ImportExpression(import) => {
                self.find_captured_vars_in_expr(&import.source, captured);
                if let Some(options) = &import.options {
                    self.find_captured_vars_in_expr(options, captured);
                }
            }
            Expression::TSAsExpression(cast) => {
                self.find_captured_vars_in_expr(&cast.expression, captured);
            }
            Expression::TSSatisfiesExpression(cast) => {
                self.find_captured_vars_in_expr(&cast.expression, captured);
            }
            Expression::TSTypeAssertion(cast) => {
                self.find_captured_vars_in_expr(&cast.expression, captured);
            }
            Expression::TSNonNullExpression(cast) => {
                self.find_captured_vars_in_expr(&cast.expression, captured);
            }
            Expression::TSInstantiationExpression(cast) => {
                self.find_captured_vars_in_expr(&cast.expression, captured);
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
        match arg {
            Argument::SpreadElement(spread) => {
                self.find_captured_vars_in_expr(&spread.argument, captured);
            }
            _ => {
                if let Some(expr) = arg.as_expression() {
                    self.find_captured_vars_in_expr(expr, captured);
                }
            }
        }
    }

    fn find_captured_vars_in_chain(&self, chain: &ChainElement, captured: &mut HashSet<String>) {
        match chain {
            ChainElement::CallExpression(c) => {
                self.find_captured_vars_in_expr(&c.callee, captured);
                for arg in &c.arguments {
                    self.find_captured_vars_in_arg(arg, captured);
                }
            }
            ChainElement::StaticMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
            }
            ChainElement::PrivateFieldExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
            }
            ChainElement::ComputedMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
                self.find_captured_vars_in_expr(&m.expression, captured);
            }
            ChainElement::TSNonNullExpression(n) => {
                self.find_captured_vars_in_expr(&n.expression, captured);
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_simple_target(
        &self,
        target: &SimpleAssignmentTarget,
        captured: &mut HashSet<String>,
    ) {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                if self.var_infos.contains_key(id.name.as_str()) {
                    captured.insert(id.name.to_string());
                }
            }
            SimpleAssignmentTarget::ComputedMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
                self.find_captured_vars_in_expr(&m.expression, captured);
            }
            SimpleAssignmentTarget::StaticMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
            }
            SimpleAssignmentTarget::PrivateFieldExpression(p) => {
                self.find_captured_vars_in_expr(&p.object, captured);
            }
            _ => {}
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
                for element in &a.elements {
                    if let Some(pattern) = element {
                        self.find_captured_vars_in_maybe_default(pattern, captured);
                    }
                }
                if let Some(rest) = &a.rest {
                    self.find_captured_vars_in_assignment_target(&rest.target, captured);
                }
            }
            AssignmentTarget::ObjectAssignmentTarget(o) => {
                for prop in &o.properties {
                    self.find_captured_vars_in_target_property(prop, captured);
                }
                if let Some(rest) = &o.rest {
                    self.find_captured_vars_in_assignment_target(&rest.target, captured);
                }
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_target_property(
        &self,
        prop: &AssignmentTargetProperty,
        captured: &mut HashSet<String>,
    ) {
        match prop {
            AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p) => {
                if self.var_infos.contains_key(p.binding.name.as_str()) {
                    captured.insert(p.binding.name.to_string());
                }
                if let Some(init) = &p.init {
                    self.find_captured_vars_in_expr(init, captured);
                }
            }
            AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) => {
                self.find_captured_vars_in_maybe_default(&p.binding, captured);
            }
        }
    }

    fn find_captured_vars_in_maybe_default(
        &self,
        pattern: &AssignmentTargetMaybeDefault,
        captured: &mut HashSet<String>,
    ) {
        match pattern {
            AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(with_default) => {
                self.find_captured_vars_in_assignment_target(&with_default.binding, captured);
                self.find_captured_vars_in_expr(&with_default.init, captured);
            }
            AssignmentTargetMaybeDefault::AssignmentTargetIdentifier(id) => {
                if self.var_infos.contains_key(id.name.as_str()) {
                    captured.insert(id.name.to_string());
                }
            }
            AssignmentTargetMaybeDefault::ComputedMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
                self.find_captured_vars_in_expr(&m.expression, captured);
            }
            AssignmentTargetMaybeDefault::StaticMemberExpression(m) => {
                self.find_captured_vars_in_expr(&m.object, captured);
            }
            AssignmentTargetMaybeDefault::PrivateFieldExpression(p) => {
                self.find_captured_vars_in_expr(&p.object, captured);
            }
            AssignmentTargetMaybeDefault::ArrayAssignmentTarget(a) => {
                for element in &a.elements {
                    if let Some(nested) = element {
                        self.find_captured_vars_in_maybe_default(nested, captured);
                    }
                }
                if let Some(rest) = &a.rest {
                    self.find_captured_vars_in_assignment_target(&rest.target, captured);
                }
            }
            AssignmentTargetMaybeDefault::ObjectAssignmentTarget(o) => {
                for prop in &o.properties {
                    match prop {
                        AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p) => {
                            if self.var_infos.contains_key(p.binding.name.as_str()) {
                                captured.insert(p.binding.name.to_string());
                            }
                            if let Some(init) = &p.init {
                                self.find_captured_vars_in_expr(init, captured);
                            }
                        }
                        AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) => {
                            self.find_captured_vars_in_maybe_default(&p.binding, captured);
                        }
                    }
                }
                if let Some(rest) = &o.rest {
                    self.find_captured_vars_in_assignment_target(&rest.target, captured);
                }
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
            Statement::DoWhileStatement(d) => {
                self.find_captured_vars_in_expr(&d.test, captured);
                self.find_captured_vars_in_stmt(&d.body, captured);
            }
            Statement::ForStatement(f) => {
                if let Some(init) = &f.init {
                    match init {
                        ForStatementInit::VariableDeclaration(d) => {
                            for decl in &d.declarations {
                                if let Some(init_expr) = &decl.init {
                                    self.find_captured_vars_in_expr(init_expr, captured);
                                }
                            }
                        }
                        _ => {
                            if let Some(init_expr) = init.as_expression() {
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
            Statement::ThrowStatement(t) => {
                self.find_captured_vars_in_expr(&t.argument, captured);
            }
            Statement::SwitchStatement(s) => {
                self.find_captured_vars_in_expr(&s.discriminant, captured);
                for case in &s.cases {
                    if let Some(test) = &case.test {
                        self.find_captured_vars_in_expr(test, captured);
                    }
                    for stmt in &case.consequent {
                        self.find_captured_vars_in_stmt(stmt, captured);
                    }
                }
            }
            Statement::LabeledStatement(l) => {
                self.find_captured_vars_in_stmt(&l.body, captured);
            }
            Statement::FunctionDeclaration(f) => {
                self.find_captured_vars_in_function_body(f, captured);
            }
            Statement::ClassDeclaration(c) => {
                if let Some(heritage) = &c.heritage {
                    self.find_captured_vars_in_expr(&heritage.expression, captured);
                }
                for element in &c.body.body {
                    if let ClassElement::MethodDefinition(method) = element {
                        self.find_captured_vars_in_function_body(&method.value, captured);
                    }
                }
            }
            _ => {}
        }
    }

    fn find_captured_vars_in_function_body(&self, f: &Function, captured: &mut HashSet<String>) {
        if let Some(body) = &f.body {
            for stmt in &body.statements {
                self.find_captured_vars_in_stmt(stmt, captured);
            }
        }
    }

    fn find_captured_vars_in_block(&self, block: &BlockStatement, captured: &mut HashSet<String>) {
        for stmt in &block.body {
            self.find_captured_vars_in_stmt(stmt, captured);
        }
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

    /// True for expressions whose value shape is unknowable statically.
    ///
    /// Deliberately narrow: `await` is excluded — the awaited coroutine type
    /// is known, so `auto` deduction beats boxing into `JsNumber`. Same for
    /// `fetch`, whose response type the emitter handles directly.
    fn is_dynamic_source(&self, expr: &Expression) -> bool {
        match expr {
            Expression::CallExpression(c) => {
                if let Expression::Identifier(id) = &c.callee {
                    matches!(id.name.as_str(), "parseInt" | "parseFloat")
                } else {
                    false
                }
            }
            Expression::Identifier(id) => self.widens.get(id.name.as_str()).is_some(),
            _ => false,
        }
    }

    fn mark_escape(&mut self, name: &str, kind: EscapeKind) {
        let current = self.escapes.get(name).cloned().unwrap_or(EscapeKind::None);
        self.escapes.insert(name.to_string(), EscapeKind::max(current, kind));
    }

    fn current_site(&self) -> StmtSite {
        StmtSite {
            index: self.statement_index,
            branch_depth: self.branch_depth,
            func_depth: self.func_depth,
        }
    }

    fn record_usage(&mut self, name: &str, usage: UsageKind) {
        let is_read = !matches!(
            usage,
            UsageKind::PropertyWrite
                | UsageKind::AssignedFromVar
                | UsageKind::DynamicAssign
                | UsageKind::ToStringCall
        );
        let site = self.current_site();
        let in_loop = self.loop_depth > 0;
        if let Some(info) = self.var_infos.get_mut(name) {
            info.usages.push(usage);
            if is_read {
                info.use_sites.push(site);
            }
            if in_loop {
                info.has_loop_use = true;
            }
        }
    }

    fn record_def(
        &mut self,
        name: &str,
        value_class: OperandClass,
        int_literal: Option<i64>,
        span_start: Option<u32>,
    ) {
        let site = self.current_site();
        if let Some(info) = self.var_infos.get_mut(name) {
            info.def_sites.push(DefSite { site, value_class, int_literal, span_start });
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

    /// Split wide-declared variables into native regions.
    ///
    /// A non-first `x = <int-literal>` definition starts a native region when
    /// every recorded site sits at branch depth zero in the declaring
    /// function, nothing loops over the variable, and no closure captures it.
    /// The emitter redeclares from that assignment onward under a fresh name.
    fn compute_narrow_splits(&mut self) {
        let captured_names: HashSet<&str> =
            self.closure_vars.values().flatten().map(String::as_str).collect();
        let mut splits = Vec::new();
        let mut taken: HashSet<String> = self.var_infos.keys().cloned().collect();
        for (var_name, info) in &self.var_infos {
            if !matches!(
                self.apply_widening(var_name, info.init_operand_class),
                OperandClass::JsNumber | OperandClass::JsValue
            ) {
                continue;
            }
            if info.has_loop_use || captured_names.contains(var_name.as_str()) {
                continue;
            }
            if !Self::sites_straight_line(info) {
                continue;
            }
            let mut split_count = 0;
            for def in info.def_sites.iter().skip(1) {
                if def.value_class != OperandClass::Integer {
                    continue;
                }
                let (Some(literal), Some(span_start)) = (def.int_literal, def.span_start) else {
                    continue;
                };
                split_count += 1;
                let narrowed_type =
                    if i32::try_from(literal).is_ok() { "int32_t" } else { "int64_t" };
                let mut suffix = split_count;
                let mut narrowed_name = format!("{}_narrowed_{}", var_name, suffix);
                while taken.contains(&narrowed_name) {
                    suffix += 1;
                    narrowed_name = format!("{}_narrowed_{}", var_name, suffix);
                }
                split_count = suffix;
                taken.insert(narrowed_name.clone());
                splits.push(NarrowSplit {
                    var_name: var_name.clone(),
                    def_span_start: span_start,
                    narrowed_name,
                    narrowed_type: narrowed_type.to_string(),
                });
            }
        }
        self.narrow_splits = splits;
    }

    /// True when every recorded site sits at branch depth zero in the
    /// declaring function. Anything else makes region renaming unsound.
    fn sites_straight_line(info: &VarInfo) -> bool {
        info.def_sites
            .iter()
            .all(|def| def.site.branch_depth == 0 && def.site.func_depth == info.decl_func_depth)
            && info
                .use_sites
                .iter()
                .all(|site| site.branch_depth == 0 && site.func_depth == info.decl_func_depth)
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
    pub closure_captures: HashMap<String, HashSet<String>>,
    pub narrow_splits: Vec<NarrowSplit>,
}

impl Default for EscapeAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;

    fn analyze_source(source: &str) -> AnalysisResult {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path("file.ts").unwrap_or_default().with_typescript(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        assert!(parsed.diagnostics.is_empty());
        assert!(!parsed.panicked);
        EscapeAnalyzer::new().analyze_program(&parsed.program)
    }

    #[test]
    fn plain_variable_never_escapes() {
        let result = analyze_source("let count = 5;\nconsole.log(count);");
        assert!(result.escapes.get("count").is_none());
    }

    #[test]
    fn closure_capture_marks_shared_ownership() {
        let result = analyze_source(concat!(
            "function makeCounter() {\n",
            "    let count = 0;\n",
            "    const bump = () => {\n",
            "        count = count + 1;\n",
            "        return count;\n",
            "    };\n",
            "    return bump();\n}",
        ));
        assert_eq!(result.escapes.get("count"), Some(&EscapeKind::ClosureCapture));
    }

    #[test]
    fn returned_variable_marks_single_owner_escape() {
        let result = analyze_source(concat!(
            "function doubleIt(n: number): number {\n",
            "    let doubled = n * 2;\n",
            "    return doubled;\n}",
        ));
        assert_eq!(result.escapes.get("doubled"), Some(&EscapeKind::Return));
    }

    #[test]
    fn known_string_method_keeps_native_type() {
        let result = analyze_source("let label = \"hi\";\nconsole.log(label.toUpperCase());");
        assert!(result.widens.get("label").is_none());
    }

    #[test]
    fn unknown_method_widens_to_js_string() {
        let result = analyze_source("let amount = 100;\nconsole.log(amount.toFixed(2));");
        assert_eq!(result.widens.get("amount"), Some(&WidenedType::ToJsString));
    }

    #[test]
    fn length_on_string_keeps_native_type() {
        let result = analyze_source("let label = \"hi\";\nconsole.log(label.length);");
        assert!(result.widens.get("label").is_none());
    }

    #[test]
    fn length_on_array_widens_to_js_array() {
        let result =
            analyze_source("let items = [1, 2];\nitems.push(3);\nconsole.log(items.length);");
        assert_eq!(result.widens.get("items"), Some(&WidenedType::ToJsArray));
    }

    #[test]
    fn integer_literal_proves_int32_range() {
        let result = analyze_source("let small = 42;\nconsole.log(small);");
        let info = result.var_infos.get("small").unwrap();
        assert_eq!(info.int_range, Some((42, 42)));
        assert!(info.int_range_exact);
    }

    #[test]
    fn computed_reassignment_forfeits_int32_range() {
        let result = analyze_source("let total = 0;\ntotal = total + 5;\nconsole.log(total);");
        let info = result.var_infos.get("total").unwrap();
        assert!(!info.int_range_exact);
    }

    #[test]
    fn literal_bounded_loop_counter_proves_range() {
        let result = analyze_source("for (let i = 0; i < 10; i++) {\nconsole.log(i);\n}");
        let info = result.var_infos.get("i").unwrap();
        assert_eq!(info.int_range, Some((0, 9)));
        assert!(info.int_range_exact);
    }

    #[test]
    fn await_boundary_does_not_force_js_number() {
        let result = analyze_source(concat!(
            "async function load(): Promise<number> {\n",
            "    let value = await fetchValue();\n",
            "    return value;\n}",
        ));
        assert!(result.widens.get("value").is_none());
    }

    #[test]
    fn await_reassignment_widens_declaration() {
        let result = analyze_source(concat!(
            "async function reload(): Promise<number> {\n",
            "    let cached = 0;\n",
            "    cached = await fetchLimit();\n",
            "    return cached;\n}",
        ));
        assert_eq!(result.widens.get("cached"), Some(&WidenedType::ToJsNumber));
    }

    #[test]
    fn dynamic_call_widens_to_js_number() {
        let result = analyze_source("let count = parseInt(input);\nconsole.log(count);");
        assert_eq!(result.widens.get("count"), Some(&WidenedType::ToJsNumber));
    }

    #[test]
    fn binary_comparison_records_signature() {
        let result = analyze_source("let first = 1;\nlet second = 2;\nif (first == second) {}");
        assert!(result.comparison_signatures.iter().any(|signature| {
            signature.kind == ComparisonKind::LooseEqual
                && signature.left == OperandClass::Integer
                && signature.right == OperandClass::Integer
        }));
    }

    #[test]
    fn truthy_string_test_records_signature() {
        let result = analyze_source("let label = \"hi\";\nif (label) {}");
        assert!(result.comparison_signatures.iter().any(|signature| {
            signature.kind == ComparisonKind::TruthyTest && signature.left == OperandClass::Text
        }));
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn captured_var(body: &str, var: &str) -> bool {
        let source = format!("let target = 5;\nlet other = 1;\nconst useIt = () => {};\n", body);
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path("file.ts").unwrap_or_default().with_typescript(true);
        let parsed = Parser::new(&allocator, &source, source_type).parse();
        assert!(parsed.diagnostics.is_empty());
        assert!(!parsed.panicked);
        let result = EscapeAnalyzer::new().analyze_program(&parsed.program);
        result.escapes.get(var) == Some(&EscapeKind::ClosureCapture)
    }

    fn captured_by_arrow(body: &str) -> bool {
        captured_var(body, "target")
    }

    #[test]
    fn captures_call_callee() {
        assert!(captured_var("{ return other(); }", "other"));
    }

    #[test]
    fn captures_logical_operands() {
        assert!(captured_by_arrow("{ return target && other; }"));
        assert!(captured_by_arrow("{ return target || other; }"));
    }

    #[test]
    fn captures_unary_argument() {
        assert!(captured_by_arrow("{ return !target; }"));
    }

    #[test]
    fn captures_update_target() {
        assert!(captured_by_arrow("{ target++; return target; }"));
    }

    #[test]
    fn captures_new_arguments() {
        assert!(captured_by_arrow("{ return new Object(target); }"));
    }

    #[test]
    fn captures_sequence_elements() {
        assert!(captured_by_arrow("{ return (other, target); }"));
    }

    #[test]
    fn captures_template_expressions() {
        assert!(captured_by_arrow("{ return `${target}`; }"));
    }

    #[test]
    fn captures_tagged_template_parts() {
        assert!(captured_by_arrow("{ return other`${target}`; }"));
    }

    #[test]
    fn captures_optional_chain_base() {
        assert!(captured_by_arrow("{ return target?.toString(); }"));
    }

    #[test]
    fn captures_spread_argument() {
        assert!(captured_by_arrow("{ return other(...target); }"));
    }

    #[test]
    fn captures_spread_array_element() {
        assert!(captured_by_arrow("{ return [...target]; }"));
    }

    #[test]
    fn captures_spread_object_property() {
        assert!(captured_by_arrow("{ return { ...target }; }"));
    }

    #[test]
    fn captures_ts_wrapper_inner() {
        assert!(captured_by_arrow("{ return (target as number); }"));
        assert!(captured_by_arrow("{ return target!; }"));
    }

    #[test]
    fn captures_array_destructuring_target() {
        assert!(captured_by_arrow("{ [target] = other; }"));
    }

    #[test]
    fn captures_object_destructuring_target() {
        assert!(captured_by_arrow("{ ({x: target} = other); }"));
    }

    #[test]
    fn captures_destructuring_default_init() {
        assert!(captured_by_arrow("{ [target = other] = other; }"));
    }

    #[test]
    fn captures_thrown_value() {
        assert!(captured_by_arrow("{ throw target; }"));
    }

    #[test]
    fn captures_switch_parts() {
        assert!(captured_by_arrow("{ switch (target) { case other: break; } }"));
    }

    #[test]
    fn captures_do_while_test() {
        assert!(captured_by_arrow("{ do { other; } while (target); }"));
    }

    #[test]
    fn captures_labeled_body() {
        assert!(captured_by_arrow("{ outer: { break outer; } return target; }"));
    }

    #[test]
    fn captures_nested_function_body() {
        assert!(captured_by_arrow("{ function inner() { return target; } return inner(); }"));
    }

    #[test]
    fn captures_for_expression_init() {
        assert!(captured_by_arrow("{ for (target = 0; target < 3; target++) {} }"));
    }

    #[test]
    fn captures_yield_argument() {
        assert!(captured_by_arrow("{ function* gen() { yield target; } return gen(); }"));
    }

    #[test]
    fn captures_dynamic_import_source() {
        assert!(captured_by_arrow("{ return import(target); }"));
    }
}

#[cfg(test)]
mod lifespan_tests {
    use super::*;
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    fn analyze_lifespan(source: &str) -> AnalysisResult {
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path("file.ts").unwrap_or_default().with_typescript(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        assert!(parsed.diagnostics.is_empty());
        assert!(!parsed.panicked);
        EscapeAnalyzer::new().analyze_program(&parsed.program)
    }

    #[test]
    fn returned_class_instance_marks_return() {
        let result = analyze_lifespan(
            "class User {\nname: string = \"\";\n}\nfunction createUser(nm: string): User {\nconst u = new User();\nu.name = nm;\nreturn u;\n}\n",
        );
        assert_eq!(result.escapes.get("u"), Some(&EscapeKind::Return));
    }

    #[test]
    fn definition_and_use_positions_recorded() {
        let result = analyze_lifespan("let count = 5;\nconsole.log(count);\ncount = 6;");
        let info = result.var_infos.get("count").unwrap();
        assert_eq!(info.def_indices(), vec![1, 3]);
        assert_eq!(info.use_indices(), vec![2]);
        assert_eq!(info.first_defined_at(), Some(1));
        assert_eq!(info.last_used_at(), Some(2));
    }

    #[test]
    fn live_after_reflects_later_reads() {
        let result = analyze_lifespan("let count = 5;\nconsole.log(count);\ncount = 6;");
        let info = result.var_infos.get("count").unwrap();
        assert!(info.is_live_after(1));
        assert!(!info.is_live_after(2));
    }

    #[test]
    fn loop_body_use_flags_variable() {
        let result = analyze_lifespan("let total = 0;\nwhile (total < 10) {\ncount(total);\n}");
        let info = result.var_infos.get("total").unwrap();
        assert!(info.has_loop_use);
    }

    #[test]
    fn straight_line_use_leaves_loop_flag_clear() {
        let result = analyze_lifespan("let total = 0;\nconsole.log(total);");
        let info = result.var_infos.get("total").unwrap();
        assert!(!info.has_loop_use);
    }

    #[test]
    fn write_only_variable_has_no_reads() {
        let result = analyze_lifespan("let dropped = 5;\ndropped = 6;");
        let info = result.var_infos.get("dropped").unwrap();
        assert_eq!(info.def_sites.len(), 2);
        assert_eq!(info.read_use_count(), 0);
    }

    #[test]
    fn class_method_body_captures_outer_variable() {
        let result =
            analyze_lifespan("let shared = 1;\nclass Worker {\nwork() {\nreturn shared;\n}\n}");
        assert_eq!(result.escapes.get("shared"), Some(&EscapeKind::ClosureCapture));
    }

    #[test]
    fn parameter_default_counts_as_use() {
        let result = analyze_lifespan(
            "let fallback = [1];\nfunction pick(first = fallback) {\nreturn first;\n}",
        );
        let info = result.var_infos.get("fallback").unwrap();
        assert!(info.last_used_at().is_some());
    }

    #[test]
    fn closure_captures_surfaced_for_move_veto() {
        let result = analyze_lifespan(
            "function outer() {\nlet cell = 0;\nconst bump = () => {\ncell = cell + 1;\nreturn cell;\n};\nreturn bump();\n}",
        );
        let captured: Vec<&String> = result.closure_captures.values().flatten().collect();
        assert!(captured.iter().any(|name| name.as_str() == "cell"));
    }

    #[test]
    fn escaping_closure_keys_captures_to_enclosing_function() {
        let result = analyze_lifespan(
            "function makeCounter() {\nlet count = 0;\nconst bump = () => {\ncount = count + 1;\nreturn count;\n};\nreturn bump;\n}",
        );
        assert_eq!(result.escapes.get("count"), Some(&EscapeKind::ClosureCapture));
        let held = result.closure_captures.get("makeCounter").unwrap();
        assert!(held.contains("count"));
    }

    #[test]
    fn one_split_at_second_assignment() {
        let result = analyze_lifespan(
            "let tally = getCount();\nprint(tally);\ntally = 42;\nprint(tally);\n",
        );
        assert_eq!(result.narrow_splits.len(), 1);
        let split = &result.narrow_splits[0];
        assert_eq!(split.var_name, "tally");
        assert_eq!(split.narrowed_name, "tally_narrowed_1");
        assert_eq!(split.narrowed_type, "int32_t");
    }

    #[test]
    fn no_split_when_first_definition_literal() {
        let result =
            analyze_lifespan("let tally = 1;\nprint(tally);\ntally = 42;\nprint(tally);\n");
        assert!(result.narrow_splits.is_empty());
    }

    #[test]
    fn no_split_when_later_definition_not_literal() {
        let result = analyze_lifespan(
            "let tally = getCount();\nprint(tally);\ntally = read();\nprint(tally);\n",
        );
        assert!(result.narrow_splits.is_empty());
    }

    #[test]
    fn no_split_compound_assignment() {
        let result = analyze_lifespan(
            "let tally = getCount();\nprint(tally);\ntally += 1;\nprint(tally);\n",
        );
        assert!(result.narrow_splits.is_empty());
    }

    #[test]
    fn no_split_under_branch() {
        let result = analyze_lifespan(
            "let tally = getCount();\nif (ready) {\ntally = 42;\n}\nprint(tally);\n",
        );
        assert!(result.narrow_splits.is_empty());
    }

    #[test]
    fn no_split_when_variable_captured() {
        let result = analyze_lifespan(
            "let tally = getCount();\nclass Box {\nread() {\nreturn tally;\n}\n}\ntally = 42;\nprint(tally);\n",
        );
        assert_eq!(result.escapes.get("tally"), Some(&EscapeKind::ClosureCapture));
        assert!(result.narrow_splits.is_empty());
    }

    #[test]
    fn split_name_collides_with_future_variable() {
        let result = analyze_lifespan(
            "let tally_narrowed_1 = 5;\nlet tally = getCount();\nprint(tally);\ntally = 42;\nprint(tally);\n",
        );
        let split = result.narrow_splits.iter().find(|s| s.var_name == "tally").unwrap();
        assert_ne!(split.narrowed_name, "tally_narrowed_1");
    }

    #[test]
    fn object_destructuring_binds_names() {
        let result =
            analyze_lifespan("let point = {x: 1, y: 2};\nlet {x, y} = point;\nprint(x);\n");
        assert!(result.var_infos.contains_key("x"));
        assert!(result.var_infos.contains_key("y"));
    }

    #[test]
    fn array_destructuring_binds_names() {
        let result =
            analyze_lifespan("let pair = [1, 2];\nlet [first, second] = pair;\nprint(first);\n");
        assert!(result.var_infos.contains_key("first"));
        assert!(result.var_infos.contains_key("second"));
    }

    #[test]
    fn literal_elements_keep_precision() {
        let result = analyze_lifespan("let [small] = [42];\nprint(small);\n");
        let info = result.var_infos.get("small").unwrap();
        assert_eq!(info.init_operand_class, OperandClass::Integer);
        assert_eq!(info.int_range, Some((42, 42)));
    }

    #[test]
    fn unknown_source_elements_stay_boxed() {
        let result = analyze_lifespan("let {name} = getUser();\nprint(name);\n");
        let info = result.var_infos.get("name").unwrap();
        assert_eq!(info.init_operand_class, OperandClass::JsValue);
    }

    #[test]
    fn capture_through_destructured_name() {
        let result = analyze_lifespan(
            "let point = {x: 1};\nlet {x} = point;\nconst get = () => x;\nprint(get());\n",
        );
        assert_eq!(result.escapes.get("x"), Some(&EscapeKind::ClosureCapture));
    }
}
