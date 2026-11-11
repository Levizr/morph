use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_semantic::{ScopeFlags, ScopeId, SemanticBuilder, SymbolId};
use oxc_span::GetSpan;
use std::collections::{HashMap, HashSet};

use super::analyzer::{AnalysisResult, EscapeAnalyzer, EscapeKind, WidenedType};
use super::context::{AccessorFlags, Ctx, INDENT, TypeMode};
use super::js_comparison::{
    ComparisonKind, ComparisonSections, ComparisonSignature, OperandClass, build_header,
    cpp_type_to_class, required_includes,
};
use super::string_methods::StringMethodHandler;
use super::type_resolver::{
    native_number_annotation_type, param_type, resolve_type, resolve_type_annotation,
    ts_type_name_to_string,
};

/// One `var` binding hoisted to its function (or program) scope.
struct HoistBinding {
    name: String,
    /// Read before its declaration, or referenced from a nested function:
    /// the binding must start as `undefined`, so it uses `JsLateValue`.
    late: bool,
    /// The name is already bound by a parameter or a hoisted function
    /// declaration in this scope. No new declaration is emitted.
    existing: bool,
    /// Resolved C++ storage type.
    alloc_type: String,
}

/// `var` bindings for one function/program scope.
struct HoistScope {
    bindings: Vec<HoistBinding>,
    by_name: HashMap<String, usize>,
    saved_var_types: Vec<(String, Option<String>)>,
    saved_shared: Vec<(String, bool)>,
    saved_unique: Vec<(String, bool)>,
}

impl HoistScope {
    fn empty() -> Self {
        Self {
            bindings: Vec::new(),
            by_name: HashMap::new(),
            saved_var_types: Vec::new(),
            saved_shared: Vec::new(),
            saved_unique: Vec::new(),
        }
    }

    fn get(&self, name: &str) -> Option<&HoistBinding> {
        self.by_name.get(name).and_then(|idx| self.bindings.get(*idx))
    }

    fn restore(&self, ctx: &mut Ctx) {
        for (name, ty) in &self.saved_var_types {
            match ty {
                Some(ty) => {
                    ctx.var_types.insert(name.clone(), ty.clone());
                }
                None => {
                    ctx.var_types.remove(name);
                }
            }
        }
        for (name, had) in &self.saved_shared {
            if *had {
                ctx.shared_ptr_vars.insert(name.clone());
            } else {
                ctx.shared_ptr_vars.remove(name);
            }
        }
        for (name, had) in &self.saved_unique {
            if *had {
                ctx.unique_ptr_vars.insert(name.clone());
            } else {
                ctx.unique_ptr_vars.remove(name);
            }
        }
    }
}

/// Resolved reference facts for one symbol, used by `var` hoisting.
struct VarSymbolInfo {
    /// A read occurs before the declaration, or a reference sits inside a
    /// nested function: the hoisted binding must start as `undefined`.
    late: bool,
}

/// Type/escape decisions for one variable declarator. Split out of
/// [`CppTranslator::emit_typed_variable_declarator`] so hoisted `var`
/// declarations can reuse the same inference and emit only the assignment
/// at the original declaration site.
struct DeclaratorStorage {
    alloc_type: String,
    cpp_type: String,
    widened: WidenedType,
    escape_kind: EscapeKind,
    new_class_name: Option<String>,
    shared_wrap_call: bool,
}

/// Translation output split by lifetime: includes go at the top of the
/// translation unit, prelude declarations at namespace scope, and body
/// statements where the code executes.
pub struct ProgramParts {
    pub includes: String,
    pub prelude: String,
    pub body: String,
}

impl ProgramParts {
    /// Join the pieces in emission order (includes, prelude, body).
    pub fn join(&self) -> String {
        let mut out = String::new();
        for section in [&self.includes, &self.prelude, &self.body] {
            if section.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(section);
        }
        out
    }
}

pub struct CppTranslator<'a> {
    source: &'a str,
    pub ctx: Ctx,
    type_mode: TypeMode,
    analysis: Option<AnalysisResult>,
    comparison_uses: HashSet<ComparisonSignature>,
    narrow_index: HashMap<(String, u32), (String, String)>,
    active_narrows: HashMap<String, String>,
    destructure_count: usize,
    /// Locals/parameters that call one of the lowered Array.prototype
    /// methods. Their declarations are forced to `JsArray` so callbacks
    /// and mutating methods share JS reference semantics; `std::vector`
    /// stays the fast path everywhere else.
    js_array_receiver_vars: HashSet<String>,
    /// Names assigned a window handle somewhere in the program
    /// (`win = new Window(...)` / `win = useWindow(...)`). Declarations
    /// without a direct window initializer become `JsWindowHandle`, the
    /// JS-visible `JsValue` form: reads stay `undefined` before the
    /// assignment, and later handle operations resolve through
    /// `window_wid(...)`.
    late_window_vars: HashSet<String>,
    /// Names assigned any value somewhere in the program. Empty/nullish
    /// declarations of these names become `JsLateValue`, so a read before
    /// the assignment is `undefined` and later assignments keep JS value
    /// semantics.
    late_value_vars: HashSet<String>,
    /// When set, top-level forward declarations omit parameter defaults.
    /// Project builds declare the defaults once in `morph_api.h`; repeating
    /// them on the premain prototype is a C++ redefinition error.
    omit_param_defaults: bool,
    /// Per-iteration binding work a `continue` must run before jumping to
    /// the next iteration. `None` entries are ordinary loops; the captured
    /// `for` emitter pushes `Some` so `continue` creates the next binding
    /// and runs the update expression exactly like fall-through does.
    loop_continue_actions: Vec<Option<LoopContinueAction>>,
    /// True while binding a `for...of` destructuring pattern. Captured
    /// names become per-iteration `shared_ptr` boxes instead of plain
    /// locals that a closure would reference after the iteration ends.
    box_loop_bindings: bool,
    /// Resolved reference facts per symbol, used to decide whether a
    /// hoisted `var` must start as `undefined`.
    var_symbol_info: HashMap<SymbolId, VarSymbolInfo>,
    /// Active function/program `var` scopes. Nested functions push their
    /// own entry so declaration sites resolve to the innermost binding.
    hoist_stack: Vec<HoistScope>,
    /// Hoist program-level `var` bindings. Module globals turn this off:
    /// their declaration sites sit at namespace scope, where an assignment
    /// statement cannot be emitted.
    hoist_program_vars: bool,
}

#[derive(Clone)]
struct LoopContinueAction {
    /// `(name, pointee_type)` for every loop binding that needs a fresh
    /// box before the update runs.
    copy_vars: Vec<(String, String)>,
    /// The emitted update expression, empty when the loop has none.
    update: String,
}

/// Receiver categories for `for...in` that enumerate index keys. Arrays and
/// vectors can borrow the receiver, while strings need a concrete
/// `std::string` temporary so literals and `JsString` convert before
/// `.size()` is called.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ForInSequence {
    Array,
    Text,
}

/// Array methods lowered to `morph::js_array_*` runtime helpers.
const JS_ARRAY_METHODS: &[&str] = &[
    "map",
    "filter",
    "forEach",
    "reduce",
    "find",
    "findIndex",
    "some",
    "every",
    "sort",
    "splice",
    "shift",
    "unshift",
    "fill",
    "flat",
    "flatMap",
    "push",
    "pop",
    "reverse",
];

/// `Math` constants with a runtime binding in `js_math.h`.
fn is_math_constant(name: &str) -> bool {
    matches!(name, "PI" | "E" | "LN2" | "LN10" | "LOG2E" | "LOG10E" | "SQRT1_2" | "SQRT2")
}

/// One access step from a destructured root to a bound name.
#[derive(Clone)]
enum AccessStep {
    Key(String),
    Index(usize),
}

/// One destructured binding before rendering: declared name plus the
/// absolute step path from the source root.
struct RawBinding {
    name: String,
    steps: Vec<AccessStep>,
}

/// Escape cooked text for a C++ double-quoted literal. Backslash and
/// quote first, then newlines, returns, tabs, and any remaining C0
/// control as `\u00xx` (fixed width, so a following hex digit cannot
/// merge into the escape). A raw newline would end the literal and fail
/// compilation — the Python transpiler broke on exactly this input.
fn escape_cpp_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// A cooked string value as a C++ literal, quotes included.
pub(crate) fn cpp_string_literal(value: &str) -> String {
    format!("\"{}\"", escape_cpp_text(value))
}

/// Binary operator a compound assignment folds into when the target is a
/// signal-backed binding; `=` and the logical assignments yield `None`.
fn compound_assign_binop(op: &str) -> Option<&'static str> {
    match op {
        "+=" => Some("+"),
        "-=" => Some("-"),
        "*=" => Some("*"),
        "/=" => Some("/"),
        "%=" => Some("%"),
        "&=" => Some("&"),
        "|=" => Some("|"),
        "^=" => Some("^"),
        "<<=" => Some("<<"),
        ">>=" => Some(">>"),
        _ => None,
    }
}

/// Escape raw template text (escapes unprocessed) for a C++ literal.
/// Backslash sequences pass through untouched — they are already valid
/// C++ escapes — except backtick, which C++ does not know.
fn escape_cpp_template_raw(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\\' if chars.peek() == Some(&'`') => {
                chars.next();
                out.push('`');
            }
            c => out.push(c),
        }
    }
    out
}

/// True when an expression definitely yields a window handle. Wrappers
/// are unwrapped because `const w = (new Window(...) as Window)` must still
/// be tracked. Conditional/logical forms count only when every branch is a
/// handle, so the declaration's native type is unambiguous.
fn is_window_value_expr(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::CallExpression(call) => {
            matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "useWindow")
        }
        Expression::NewExpression(new) => {
            !new.arguments.is_empty()
                && matches!(&new.callee, Expression::Identifier(id) if id.name.as_str() == "Window")
        }
        Expression::TSAsExpression(a) => is_window_value_expr(&a.expression),
        Expression::TSSatisfiesExpression(s) => is_window_value_expr(&s.expression),
        Expression::TSNonNullExpression(n) => is_window_value_expr(&n.expression),
        Expression::ParenthesizedExpression(p) => is_window_value_expr(&p.expression),
        Expression::ConditionalExpression(c) => {
            is_window_value_expr(&c.consequent) && is_window_value_expr(&c.alternate)
        }
        Expression::LogicalExpression(l) => {
            is_window_value_expr(&l.left) && is_window_value_expr(&l.right)
        }
        _ => false,
    }
}

/// True when a declaration starts without a real value (`let x;`,
/// `let x = null`, `let x = undefined`, plus TS wrappers). Those bindings
/// keep JS value semantics when they are assigned later.
fn is_nullish_value_expr(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::NullLiteral(_) => true,
        Expression::Identifier(id) => id.name.as_str() == "undefined",
        Expression::TSAsExpression(a) => is_nullish_value_expr(&a.expression),
        Expression::TSSatisfiesExpression(s) => is_nullish_value_expr(&s.expression),
        Expression::TSNonNullExpression(n) => is_nullish_value_expr(&n.expression),
        Expression::ParenthesizedExpression(p) => is_nullish_value_expr(&p.expression),
        _ => false,
    }
}

/// Unwrap wrappers that do not change a class expression's value.
fn class_expression_of<'a, 'b>(expr: &'b Expression<'a>) -> Option<&'b Class<'a>> {
    match expr {
        Expression::ClassExpression(class) => Some(class),
        Expression::ParenthesizedExpression(p) => class_expression_of(&p.expression),
        Expression::TSAsExpression(a) => class_expression_of(&a.expression),
        Expression::TSSatisfiesExpression(s) => class_expression_of(&s.expression),
        Expression::TSNonNullExpression(n) => class_expression_of(&n.expression),
        Expression::TSTypeAssertion(t) => class_expression_of(&t.expression),
        _ => None,
    }
}

/// A single-char value as a C++ character literal.
fn cpp_char_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// True for expression-arrow bodies that lower to a void runtime call
/// (`clearTimeout`/`clearInterval` → `morph::clear_timer`). Emitting those
/// as `-> auto { return <void>; }` is ill-formed, so they need a `-> void`
/// body with a bare call instead (docs/dev/bugs#4).
fn is_void_timer_call(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        if let Expression::Identifier(id) = &call.callee {
            return id.name.as_str() == "clearTimeout" || id.name.as_str() == "clearInterval";
        }
    }
    false
}

/// Default C++ initializer for a TS optional parameter (`name?: T`).
/// Optional means "may be omitted": containers/strings default-construct,
/// numbers/bools take their zero value, and JS values become `undefined`
/// so `x ?? fallback` keeps its JS meaning.
fn optional_param_default(cpp_type: &str) -> &'static str {
    let ty = cpp_type.trim();
    if ty == "auto" || ty == "JsValue" {
        "JsValue(JsUndefined{})"
    } else if ty == "JsNumber" || ty == "double" || ty == "float" {
        "0"
    } else if ty.starts_with("int")
        || ty.starts_with("uint")
        || ty.starts_with("long")
        || ty.starts_with("short")
        || ty == "size_t"
    {
        "0"
    } else if ty == "bool" || ty == "JsBoolean" {
        "false"
    } else if ty.starts_with("std::shared_ptr")
        || ty.starts_with("std::unique_ptr")
        || ty.contains('*')
    {
        "nullptr"
    } else {
        "{}"
    }
}

/// Metadata key for a class field. Private names keep their `#` so `#x`
/// and `x` can coexist exactly as they do in JavaScript.
fn class_field_key(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
        PropertyKey::StringLiteral(s) => Some(s.value.to_string()),
        PropertyKey::NumericLiteral(n) => Some(n.value.to_string()),
        PropertyKey::PrivateIdentifier(p) => Some(format!("#{}", p.name)),
        _ => None,
    }
}

fn is_numeric_field_type(ty: &str) -> bool {
    matches!(ty, "JsNumber" | "float" | "double" | "size_t")
        || ty.starts_with("int")
        || ty.starts_with("uint")
        || ty.starts_with("long")
        || ty.starts_with("short")
}

fn is_text_field_type(ty: &str) -> bool {
    matches!(ty, "std::string" | "JsString")
}

fn is_array_field_type(ty: &str) -> bool {
    ty == "JsArray" || ty.starts_with("std::vector<")
}

/// How a `this.<field>` write constrains the field's C++ type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldUseKind {
    Plain,
    AddAssign,
    NumericAssign,
    Update,
}

fn this_field_target_key(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(m)
            if matches!(&m.object, Expression::ThisExpression(_)) =>
        {
            Some(m.property.name.to_string())
        }
        AssignmentTarget::PrivateFieldExpression(p)
            if matches!(&p.object, Expression::ThisExpression(_)) =>
        {
            Some(format!("#{}", p.field.name))
        }
        _ => None,
    }
}

fn this_simple_field_target_key(target: &SimpleAssignmentTarget<'_>) -> Option<String> {
    match target {
        SimpleAssignmentTarget::StaticMemberExpression(m)
            if matches!(&m.object, Expression::ThisExpression(_)) =>
        {
            Some(m.property.name.to_string())
        }
        SimpleAssignmentTarget::PrivateFieldExpression(p)
            if matches!(&p.object, Expression::ThisExpression(_)) =>
        {
            Some(format!("#{}", p.field.name))
        }
        _ => None,
    }
}

fn class_field_target_key(target: &AssignmentTarget<'_>, class: &str) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(id) if id.name.as_str() == class) => {
            Some(m.property.name.to_string())
        }
        AssignmentTarget::PrivateFieldExpression(p) if matches!(&p.object, Expression::Identifier(id) if id.name.as_str() == class) => {
            Some(format!("#{}", p.field.name))
        }
        _ => None,
    }
}

fn class_simple_field_target_key(
    target: &SimpleAssignmentTarget<'_>,
    class: &str,
) -> Option<String> {
    match target {
        SimpleAssignmentTarget::StaticMemberExpression(m) if matches!(&m.object, Expression::Identifier(id) if id.name.as_str() == class) => {
            Some(m.property.name.to_string())
        }
        SimpleAssignmentTarget::PrivateFieldExpression(p) if matches!(&p.object, Expression::Identifier(id) if id.name.as_str() == class) => {
            Some(format!("#{}", p.field.name))
        }
        _ => None,
    }
}

/// True for annotations that promise a dynamic JS value (`any`, `unknown`,
/// unions, intersections). The C++ storage type must be a Js* carrier —
/// `auto` cannot hold either branch.
fn dynamic_annotation(ann: Option<&TSTypeAnnotation<'_>>) -> bool {
    fn is_dynamic(ty: &TSType<'_>) -> bool {
        match ty {
            TSType::TSAnyKeyword(_)
            | TSType::TSUnknownKeyword(_)
            | TSType::TSUnionType(_)
            | TSType::TSIntersectionType(_) => true,
            TSType::TSParenthesizedType(p) => is_dynamic(&p.type_annotation),
            _ => false,
        }
    }
    ann.is_some_and(|ann| is_dynamic(&ann.type_annotation))
}

/// Merge two candidate types for the same class field. Conflicting value
/// classes widen to `JsLateValue` so later assignments keep JS semantics;
/// numeric/text/array pairs pick the JS-visible runtime type.
fn merge_field_types(current: &str, next: &str) -> String {
    if current == next {
        return current.to_string();
    }
    if current.is_empty() || current == "auto" {
        return next.to_string();
    }
    if next.is_empty() || next == "auto" {
        return current.to_string();
    }
    if is_numeric_field_type(current) && is_numeric_field_type(next) {
        return "JsNumber".to_string();
    }
    if is_text_field_type(current) && is_text_field_type(next) {
        return "JsString".to_string();
    }
    if is_array_field_type(current) && is_array_field_type(next) {
        return "JsArray".to_string();
    }
    if matches!(current, "JsValue" | "JsLateValue") || matches!(next, "JsValue" | "JsLateValue") {
        return "JsLateValue".to_string();
    }
    "JsLateValue".to_string()
}

fn collect_binding_identifiers<'b, 'a>(
    pattern: &'b BindingPattern<'a>,
    out: &mut Vec<&'b BindingIdentifier<'a>>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(id) => out.push(id),
        BindingPattern::AssignmentPattern(assign) => {
            collect_binding_identifiers(&assign.left, out);
        }
        BindingPattern::ObjectPattern(object) => {
            for property in &object.properties {
                collect_binding_identifiers(&property.value, out);
            }
            if let Some(rest) = &object.rest {
                collect_binding_identifiers(&rest.argument, out);
            }
        }
        BindingPattern::ArrayPattern(array) => {
            for element in array.elements.iter().flatten() {
                collect_binding_identifiers(element, out);
            }
            if let Some(rest) = &array.rest {
                collect_binding_identifiers(&rest.argument, out);
            }
        }
    }
}

fn collect_formal_param_names(params: &FormalParameters<'_>, out: &mut HashSet<String>) {
    for param in &params.items {
        let mut ids = Vec::new();
        collect_binding_identifiers(&param.pattern, &mut ids);
        for id in ids {
            out.insert(id.name.to_string());
        }
    }
    if let Some(rest) = &params.rest {
        let mut ids = Vec::new();
        collect_binding_identifiers(&rest.rest.argument, &mut ids);
        for id in ids {
            out.insert(id.name.to_string());
        }
    }
}

/// Initializers a hoisted `var` cannot hold once it must start as
/// `undefined`: lambdas, class instances, and class expressions have no
/// `JsValue` conversion.
fn hoist_value_is_unrepresentable(init: Option<&Expression<'_>>) -> bool {
    match init {
        Some(
            Expression::ArrowFunctionExpression(_)
            | Expression::FunctionExpression(_)
            | Expression::ClassExpression(_)
            | Expression::NewExpression(_),
        ) => true,
        Some(Expression::ParenthesizedExpression(p)) => {
            hoist_value_is_unrepresentable(Some(&p.expression))
        }
        Some(Expression::TSAsExpression(e)) => hoist_value_is_unrepresentable(Some(&e.expression)),
        Some(Expression::TSSatisfiesExpression(e)) => {
            hoist_value_is_unrepresentable(Some(&e.expression))
        }
        Some(Expression::TSNonNullExpression(e)) => {
            hoist_value_is_unrepresentable(Some(&e.expression))
        }
        _ => false,
    }
}

/// Collect function declarations in one function/program scope so a
/// same-named `var` reuses the hoisted function binding instead of
/// declaring a second C++ local.
struct FunctionNameScan {
    functions: HashSet<String>,
}

impl<'a> Visit<'a> for FunctionNameScan {
    fn visit_function(&mut self, _function: &Function<'a>, _flags: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _function: &ArrowFunctionExpression<'a>) {}

    fn visit_statement(&mut self, stmt: &Statement<'a>) {
        if let Statement::FunctionDeclaration(function) = stmt {
            if let Some(id) = &function.id {
                self.functions.insert(id.name.to_string());
            }
            return;
        }
        oxc_ast_visit::walk::walk_statement(self, stmt);
    }

    fn visit_declaration(&mut self, decl: &Declaration<'a>) {
        if let Declaration::FunctionDeclaration(function) = decl {
            if let Some(id) = &function.id {
                self.functions.insert(id.name.to_string());
            }
            return;
        }
        oxc_ast_visit::walk::walk_declaration(self, decl);
    }
}

/// Build one [`HoistScope`] by walking the declarations in source order.
/// Types are inferred with the same declarator machinery as ordinary
/// locals; early-read bindings force `JsLateValue` instead.
struct TypeScan<'t, 'a> {
    translator: &'t mut CppTranslator<'a>,
    scope: HoistScope,
    params: HashSet<String>,
    functions: HashSet<String>,
    seen: HashSet<String>,
}

impl<'t, 'a> Visit<'a> for TypeScan<'t, 'a> {
    fn visit_variable_declaration(&mut self, decl: &VariableDeclaration<'a>) {
        if decl.kind == VariableDeclarationKind::Var {
            for declarator in &decl.declarations {
                let mut ids = Vec::new();
                collect_binding_identifiers(&declarator.id, &mut ids);
                for id in ids {
                    let name = id.name.to_string();
                    if !self.seen.insert(name.clone()) {
                        continue;
                    }
                    let existing = self.params.contains(&name) || self.functions.contains(&name);
                    let late = !existing && self.translator.binding_is_late(id);
                    let saved_var_type = self.translator.ctx.var_types.get(&name).cloned();
                    let saved_shared = self.translator.ctx.shared_ptr_vars.contains(&name);
                    let saved_unique = self.translator.ctx.unique_ptr_vars.contains(&name);
                    let mut storage = None;
                    let alloc_type = if existing {
                        self.translator
                            .ctx
                            .var_types
                            .get(&name)
                            .cloned()
                            .unwrap_or_else(|| "JsLateValue".to_string())
                    } else {
                        let resolved = self.translator.declarator_storage(declarator, &name);
                        let native_type = resolved.alloc_type.clone();
                        storage = Some(resolved);
                        let unnamed_type = native_type == "auto" || native_type.contains("auto");
                        if unnamed_type
                            || (late && hoist_value_is_unrepresentable(declarator.init.as_ref()))
                        {
                            // The hoister cannot name a storage type for this
                            // declarator (lambdas, class expressions, and
                            // untyped call results), or the initializer has no
                            // `JsLateValue` conversion. Keep the original
                            // declaration-site lowering instead of widening.
                            // Early reads of these kinds are rejected by
                            // `morph check`; nested-closure reads keep the
                            // native type and are captured by reference.
                            match saved_var_type {
                                Some(ty) => {
                                    self.translator.ctx.var_types.insert(name.clone(), ty);
                                }
                                None => {
                                    self.translator.ctx.var_types.remove(&name);
                                }
                            }
                            continue;
                        }
                        if late {
                            if self.translator.late_window_vars.contains(&name) {
                                "JsWindowHandle".to_string()
                            } else {
                                "JsLateValue".to_string()
                            }
                        } else {
                            native_type
                        }
                    };

                    self.scope.saved_var_types.push((name.clone(), saved_var_type));
                    self.scope.saved_shared.push((name.clone(), saved_shared));
                    self.scope.saved_unique.push((name.clone(), saved_unique));
                    if !existing {
                        self.translator.ctx.var_types.insert(name.clone(), alloc_type.clone());
                        if late {
                            // Late storage is `JsLateValue`/`JsWindowHandle`,
                            // never a smart pointer.
                            self.translator.ctx.shared_ptr_vars.remove(&name);
                            self.translator.ctx.unique_ptr_vars.remove(&name);
                        } else if let Some(storage) = &storage {
                            if storage.alloc_type.starts_with("std::shared_ptr<") {
                                self.translator.ctx.shared_ptr_vars.insert(name.clone());
                            } else {
                                self.translator.ctx.shared_ptr_vars.remove(&name);
                            }
                            if storage.alloc_type.starts_with("std::unique_ptr<") {
                                self.translator.ctx.unique_ptr_vars.insert(name.clone());
                            } else {
                                self.translator.ctx.unique_ptr_vars.remove(&name);
                            }
                        } else {
                            self.translator.ctx.shared_ptr_vars.remove(&name);
                            self.translator.ctx.unique_ptr_vars.remove(&name);
                        }
                        self.translator.ctx.need(&alloc_type);
                    }
                    self.scope.by_name.insert(name.clone(), self.scope.bindings.len());
                    self.scope.bindings.push(HoistBinding { name, late, existing, alloc_type });
                }
            }
        }
        oxc_ast_visit::walk::walk_variable_declaration(self, decl);
    }

    fn visit_function(&mut self, _function: &Function<'a>, _flags: ScopeFlags) {}

    fn visit_arrow_function_expression(&mut self, _function: &ArrowFunctionExpression<'a>) {}
}

/// True when `ref_scope` sits inside a function/arrow nested under
/// `decl_scope`. References in nested functions are conservative `var`
/// hoisting candidates: the closure can run before the declaration.
fn is_nested_var_scope(
    scoping: &oxc_semantic::Scoping,
    ref_scope: ScopeId,
    decl_scope: ScopeId,
) -> bool {
    if ref_scope == decl_scope {
        return false;
    }
    for scope in scoping.scope_ancestors(ref_scope) {
        if scope == decl_scope {
            return false;
        }
        if scoping.scope_flags(scope).is_var() {
            return true;
        }
    }
    false
}

/// A supported `.then`/`.catch` callback: a non-async arrow with zero or
/// one plain identifier parameter. Function-expression callbacks are
/// rejected because their dynamic `this` has no inline lowering.
#[derive(Clone, Copy)]
enum ThenCallback<'b, 'a> {
    Arrow(&'b ArrowFunctionExpression<'a>),
}

impl<'b, 'a> ThenCallback<'b, 'a> {
    fn params(self) -> &'b FormalParameters<'a> {
        match self {
            Self::Arrow(f) => &f.params,
        }
    }

    fn has_param(self) -> bool {
        !self.params().items.is_empty()
    }
}

/// Parse a statement-level `fetch(...)` chain into its root call and the
/// supported callback steps. Supported shapes are `fetch(...)`,
/// `fetch(...).then(cb)`, `fetch(...).catch(cb)`, and
/// `fetch(...).then(cb).catch(cb)`; anything else returns `None`.
fn parse_then_chain<'b, 'a>(
    expr: &'b Expression<'a>,
) -> Option<(&'b CallExpression<'a>, Option<ThenCallback<'b, 'a>>, Option<ThenCallback<'b, 'a>>)> {
    let mut calls: Vec<(&'b str, &'b CallExpression<'a>)> = Vec::new();
    let mut cur = expr;
    loop {
        let Expression::CallExpression(call) = cur else { break };
        let Some(MemberExpression::StaticMemberExpression(member)) =
            call.callee.as_member_expression()
        else {
            break;
        };
        let prop = member.property.name.as_str();
        if prop != "then" && prop != "catch" {
            return None;
        }
        // A second `.then` argument is the onRejected callback; the inline
        // lowering cannot run it, so reject rather than drop it.
        if call.optional || member.optional || call.arguments.len() != 1 {
            return None;
        }
        calls.push((prop, call));
        cur = &member.object;
    }
    let Expression::CallExpression(root) = cur else { return None };
    if !matches!(&root.callee, Expression::Identifier(id) if id.name.as_str() == "fetch") {
        return None;
    }
    calls.reverse();
    let then_cb;
    let catch_cb;
    match calls.as_slice() {
        [(p, c)] if *p == "then" => {
            then_cb = then_callback(c.arguments.first());
            catch_cb = None;
        }
        [(p, c)] if *p == "catch" => {
            then_cb = None;
            catch_cb = then_callback(c.arguments.first());
        }
        [(p1, c1), (p2, c2)] if *p1 == "then" && *p2 == "catch" => {
            then_cb = then_callback(c1.arguments.first());
            catch_cb = then_callback(c2.arguments.first());
        }
        _ => return None,
    }
    Some((root, then_cb, catch_cb))
}

fn then_callback<'b, 'a>(arg: Option<&'b Argument<'a>>) -> Option<ThenCallback<'b, 'a>> {
    let Some(Argument::ArrowFunctionExpression(f)) = arg else { return None };
    if f.r#async || f.params.rest.is_some() || f.params.items.len() > 1 {
        return None;
    }
    if let Some(p) = f.params.items.first()
        && !matches!(p.pattern, BindingPattern::BindingIdentifier(_))
    {
        return None;
    }
    Some(ThenCallback::Arrow(f))
}

impl<'a> CppTranslator<'a> {
    pub fn new(
        source: &'a str,
        indent_level: usize,
        type_mode: TypeMode,
        runtime_path: Option<String>,
    ) -> Self {
        let mut ctx = Ctx::default();
        ctx.indent_level = indent_level;
        // A non-zero indent marks a snippet that will be spliced inside a
        // function or lambda. Nested arrows there may capture the host's
        // locals, so they must not emit a captureless `[]` list.
        ctx.arrow_depth = usize::from(indent_level > 0);
        ctx.runtime_path = runtime_path;
        ctx.type_mode = type_mode;
        let this = Self {
            source,
            ctx,
            type_mode,
            analysis: None,
            comparison_uses: HashSet::new(),
            narrow_index: HashMap::new(),
            active_narrows: HashMap::new(),
            destructure_count: 0,
            js_array_receiver_vars: HashSet::new(),
            late_window_vars: HashSet::new(),
            late_value_vars: HashSet::new(),
            omit_param_defaults: false,
            loop_continue_actions: Vec::new(),
            box_loop_bindings: false,
            var_symbol_info: HashMap::new(),
            hoist_stack: Vec::new(),
            hoist_program_vars: true,
        };
        this
    }

    pub fn set_omit_param_defaults(&mut self, omit: bool) {
        self.omit_param_defaults = omit;
    }

    pub fn set_hoist_program_vars(&mut self, hoist: bool) {
        self.hoist_program_vars = hoist;
    }

    fn span_text(&self, span: oxc_span::Span) -> &'a str {
        span.source_text(self.source)
    }

    fn run_analysis(&mut self, program: &Program<'a>) {
        let mut analyzer = EscapeAnalyzer::new();
        analyzer.set_type_mode(self.type_mode);
        self.analysis = Some(analyzer.analyze_program(program));
    }

    fn indent(&self) -> String {
        INDENT.repeat(self.ctx.indent_level)
    }

    // Forward-declaration signature for one top-level function. Nested
    // functions lower to capturing lambdas inline (no prototype needed).
    fn fn_decl_sig(&mut self, f: &Function<'a>) -> Option<(String, String)> {
        let id = f.id.as_ref()?;
        let name = id.name.to_string();
        if f.body.is_none() || name == "main" {
            return None;
        }
        let ret = if let Some(rt) = &f.return_type {
            resolve_type_annotation(
                Some(rt),
                "auto",
                &self.ctx.template_params,
                false,
                &self.ctx.class_names,
            )
        } else if f.r#async {
            "JsValue".to_string()
        } else if self.has_return(f.body.as_ref().unwrap()) {
            "auto".to_string()
        } else {
            "void".to_string()
        };
        let mut final_ret = ret.clone();
        if f.r#async {
            final_ret = self.ctx.async_result_type(&final_ret);
        }
        if final_ret == "auto" {
            if let Some(body_ref) = f.body.as_ref() {
                if let Some(ty) = self.literal_return_type(body_ref) {
                    final_ret = ty;
                }
            }
        }
        if !f.r#async && final_ret == "JsValue" {
            if let Some(body_ref) = f.body.as_ref() {
                if Self::returns_only_lambdas(body_ref) {
                    final_ret = "auto".to_string();
                }
            }
        }
        let params = if self.omit_param_defaults {
            self.format_params_no_defaults(&f.params)
        } else {
            self.format_params(&f.params)
        };
        let tp = if let Some(tp) = &f.type_parameters {
            let decls: Vec<String> =
                tp.params.iter().map(|p| format!("typename {}", p.name.name)).collect();
            format!("template <{}>\n", decls.join(", "))
        } else {
            String::new()
        };
        let sig = format!("{}{} {}({})", tp, final_ret, name, params);
        Some((name, sig))
    }

    /// Run the analysis pass and register narrowed locals. Shared by whole
    /// program emission and standalone-expression emission.
    pub(crate) fn analyze_program(&mut self, program: &Program<'a>) {
        // Analysis always runs: escape analysis, type widening, comparison
        // tracking, and integer-range proofs feed the single emitter path.
        if self.analysis.is_none() {
            self.run_analysis(program);
        }
        if self.var_symbol_info.is_empty() {
            self.collect_var_symbol_info(program);
        }
        if let Some(analysis) = self.analysis.as_ref() {
            for split in &analysis.narrow_splits {
                self.narrow_index.insert(
                    (split.var_name.clone(), split.def_span_start),
                    (split.narrowed_name.clone(), split.narrowed_type.clone()),
                );
            }
        }
        self.collect_late_vars(program);
        self.collect_js_array_receivers(program);
    }

    /// Resolve every symbol's reference spans once, so `var` hoisting can
    /// tell an early read (or a closure reference) from an ordinary
    /// declaration-order use.
    fn collect_var_symbol_info(&mut self, program: &Program<'a>) {
        let semantic = SemanticBuilder::new().with_build_nodes(true).build(program).semantic;
        let scoping = semantic.scoping();
        let mut info = HashMap::new();
        for symbol_id in scoping.symbol_ids() {
            let decl_scope = scoping.symbol_scope_id(symbol_id);
            let decl_start = scoping.symbol_span(symbol_id).start;
            let mut late = false;
            for &reference_id in scoping.get_resolved_reference_ids(symbol_id) {
                let reference = scoping.get_reference(reference_id);
                let span = semantic.nodes().get_node(reference.node_id()).span();
                if reference.is_read() && span.start < decl_start {
                    late = true;
                }
                if is_nested_var_scope(scoping, reference.scope_id(), decl_scope) {
                    late = true;
                }
                if late {
                    break;
                }
            }
            info.insert(symbol_id, VarSymbolInfo { late });
        }
        self.var_symbol_info = info;
    }

    /// Record names assigned later in the program.
    ///
    /// The scan is whole-file because the declaration and its later
    /// assignment may sit in different snippets (`let x;` in a component
    /// body, `x = ...` in a JSX handler). Per-name tracking matches the
    /// translator's existing Window-handle model; declarations with a
    /// direct initializer keep their inferred native type.
    fn collect_late_vars(&mut self, program: &Program<'a>) {
        struct Collector<'s> {
            windows: &'s mut HashSet<String>,
            values: &'s mut HashSet<String>,
        }
        impl<'a> Visit<'a> for Collector<'_> {
            fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &a.left {
                    // Any write (plain, compound, logical) means a later
                    // declaration without an initializer must stay a JS
                    // value.
                    self.values.insert(id.name.to_string());
                    if a.operator == AssignmentOperator::Assign && is_window_value_expr(&a.right) {
                        self.windows.insert(id.name.to_string());
                    }
                }
                oxc_ast_visit::walk::walk_assignment_expression(self, a);
            }

            fn visit_update_expression(&mut self, u: &oxc_ast::ast::UpdateExpression<'a>) {
                if let oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(id) =
                    &u.argument
                {
                    self.values.insert(id.name.to_string());
                }
                oxc_ast_visit::walk::walk_update_expression(self, u);
            }
        }
        Collector { windows: &mut self.late_window_vars, values: &mut self.late_value_vars }
            .visit_program(program);
    }

    /// Record identifiers used as receivers of lowered Array methods.
    /// Declarations for these names emit `JsArray`, so `map`/`sort`/`splice`
    /// callbacks and in-place mutations keep JS reference semantics instead
    /// of copying a native vector.
    fn collect_js_array_receivers(&mut self, program: &Program<'a>) {
        struct Collector<'s> {
            out: &'s mut HashSet<String>,
        }
        impl<'a> oxc_ast_visit::Visit<'a> for Collector<'_> {
            fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
                if let Expression::StaticMemberExpression(member) = &call.callee {
                    if JS_ARRAY_METHODS.contains(&member.property.name.as_str()) {
                        if let Expression::Identifier(object) = &member.object {
                            self.out.insert(object.name.to_string());
                        }
                    }
                }
                oxc_ast_visit::walk::walk_call_expression(self, call);
            }
        }
        let mut collector = Collector { out: &mut self.js_array_receiver_vars };
        collector.visit_program(program);
    }

    /// Emit one expression for a hosting unit (component const lowering).
    pub(crate) fn emit_expression_host(&mut self, expr: &Expression<'a>) -> String {
        self.emit_expression(expr)
    }

    /// Inferred C++ type of a standalone expression, when known.
    pub(crate) fn expression_type(&self, expr: &Expression<'a>) -> Option<String> {
        self.infer_type_from_init(expr)
    }

    fn hoisted_binding(&self, name: &str) -> Option<&HoistBinding> {
        self.hoist_stack.last().and_then(|scope| scope.get(name))
    }

    /// True when the binding was read before its declaration or is
    /// referenced from a nested function.
    fn binding_is_late(&self, id: &BindingIdentifier<'_>) -> bool {
        id.symbol_id
            .get()
            .and_then(|symbol| self.var_symbol_info.get(&symbol))
            .is_some_and(|info| info.late)
    }

    /// Build the `var` hoist plan for one function/program scope.
    fn scan_hoists(
        &mut self,
        statements: &[Statement<'a>],
        params: Option<&FormalParameters<'a>>,
    ) -> HoistScope {
        let mut param_names = HashSet::new();
        if let Some(params) = params {
            collect_formal_param_names(params, &mut param_names);
        }
        let mut function_scan = FunctionNameScan { functions: HashSet::new() };
        for stmt in statements {
            function_scan.visit_statement(stmt);
        }
        let mut scan = TypeScan {
            translator: self,
            scope: HoistScope {
                bindings: Vec::new(),
                by_name: HashMap::new(),
                saved_var_types: Vec::new(),
                saved_shared: Vec::new(),
                saved_unique: Vec::new(),
            },
            params: param_names,
            functions: function_scan.functions,
            seen: HashSet::new(),
        };
        for stmt in statements {
            scan.visit_statement(stmt);
        }
        scan.scope
    }

    fn emit_hoist_prelude(&self, scope: &HoistScope) -> Option<String> {
        let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
        let mut lines = Vec::new();
        for binding in &scope.bindings {
            if binding.existing || binding.alloc_type.is_empty() {
                continue;
            }
            lines.push(format!(
                "{}{}{} {}{{}};",
                self.indent(),
                prefix,
                binding.alloc_type,
                binding.name
            ));
        }
        if lines.is_empty() { None } else { Some(lines.join("\n")) }
    }

    /// Initializer for a hoisted `var` declaration site. Late bindings
    /// assign a JS value; ordinary bindings reuse the declarator's native
    /// init code.
    fn hoisted_init_code(
        &mut self,
        d: &VariableDeclarator<'a>,
        name: &str,
        late: bool,
    ) -> Option<String> {
        let init = d.init.as_ref()?;
        if late {
            let code = self.emit_expression(init);
            if let Some(inferred) = self.infer_type_from_init(init) {
                if inferred.starts_with("std::vector<") {
                    return Some(format!("JsArray({code})"));
                }
            }
            Some(code)
        } else {
            let storage = self.declarator_storage(d, name);
            self.declarator_init_code(d, &storage)
        }
    }

    pub fn translate_program(&mut self, program: &Program<'a>) -> String {
        self.translate_program_parts(program).join()
    }

    /// Translate a program into separately addressable pieces. Module
    /// initializers need the hoisted declarations at namespace scope and
    /// the executable statements inside a function; ordinary callers use
    /// [`Self::translate_program`] to join them back in emission order.
    pub fn translate_program_parts(&mut self, program: &Program<'a>) -> ProgramParts {
        self.analyze_program(program);
        // Pre-register every class-like name so later-defined bases,
        // `new C()` sites, and `instanceof` targets resolve regardless
        // of declaration order.
        for stmt in &program.body {
            match stmt {
                Statement::ClassDeclaration(c) => {
                    if let Some(id) = &c.id {
                        self.ctx.class_names.insert(id.name.to_string());
                    }
                }
                Statement::TSInterfaceDeclaration(i) => {
                    self.ctx.class_names.insert(i.id.name.to_string());
                }
                Statement::TSEnumDeclaration(e) => {
                    self.ctx.class_names.insert(e.id.name.to_string());
                }
                _ => {}
            }
        }
        self.collect_class_metadata(program);
        let scope = if self.hoist_program_vars {
            self.scan_hoists(&program.body, None)
        } else {
            HoistScope::empty()
        };
        // First pass: forward declarations for top-level functions
        // (needed for mutual recursion where a() calls later-defined b()).
        // Nested functions lower to capturing lambdas inline instead.
        let mut fn_decls: Vec<(String, String)> = Vec::new(); // (name, signature)
        for stmt in &program.body {
            if let Statement::FunctionDeclaration(f) = stmt {
                if let Some(decl) = self.fn_decl_sig(f) {
                    fn_decls.push(decl);
                }
            }
        }
        let hoist_prelude = self.emit_hoist_prelude(&scope);
        let mut fn_prelude = Vec::new();
        for (_, sig) in &fn_decls {
            fn_prelude.push(format!("static inline {};", sig));
        }
        self.hoist_stack.push(scope);
        let mut body_lines = Vec::new();
        for stmt in &program.body {
            if self.is_main_call(stmt) {
                continue;
            }
            if let Some(code) = self.emit_statement(stmt) {
                if !code.trim().is_empty() {
                    body_lines.push(code);
                }
            }
        }
        let scope = self.hoist_stack.pop();
        if let Some(scope) = scope {
            scope.restore(&mut self.ctx);
        }
        let body = body_lines.join("\n");
        if self.ctx.needed.contains("<print>") {
            // C++23 <print> is self-contained, no need for iostream sync
        }
        let mut js_comparison_block = String::new();
        // Header sections come from emit-time records only, so the generated
        // block always matches the helpers actually called below.
        if !self.comparison_uses.is_empty() {
            let sections = ComparisonSections::from_signatures(&self.comparison_uses);
            if sections.needs_helpers() {
                for include in required_includes(sections) {
                    self.ctx.needed.insert(include.to_string());
                }
                if sections.js_types {
                    self.ctx.need("JsValue");
                }
                js_comparison_block = build_header(sections);
            }
        }
        // std::vector printed via println/format needs the vector formatter.
        // js_value_format.h is self-contained (pulls js_value.h itself).
        let uses_vector = self.ctx.needed.iter().any(|h| h == "<vector>");
        let uses_print_fmt = self.ctx.needed.iter().any(|h| h == "<print>" || h == "<format>");
        if uses_vector && uses_print_fmt {
            self.ctx.needed.insert("\"../../runtime/cpp/types/js_value_format.h\"".to_string());
        }
        // Include js_types.h if any Js types are used, or if js_string_helpers.h is needed
        let needs_js_types =
            self.ctx.needed.iter().any(|h| h.contains("js_types") || h.contains("Js"));
        let needs_str_helpers = self.ctx.needed.iter().any(|h| h.contains("js_string_helpers"));
        // Check for actual JS type usage (not just js_string_helpers which is a separate header)
        let has_js_types = self.ctx.needed.iter().any(|h| {
            h.contains("js_types") || (h.contains("Js") && !h.contains("js_string_helpers"))
        });
        if needs_js_types || needs_str_helpers || has_js_types {
            // Only insert if not already present via other Js headers
            let has_js = self.ctx.needed.iter().any(|h| h.contains("js_"));
            if !has_js || needs_str_helpers {
                self.ctx.needed.insert("\"../../runtime/cpp/types/js_types.h\"".to_string());
            }
        }
        let includes = self.ctx.generate_includes();
        let mut prelude_sections = Vec::new();
        if !js_comparison_block.is_empty() {
            prelude_sections.push(js_comparison_block);
        }
        if let Some(hoist_prelude) = hoist_prelude {
            prelude_sections.push(hoist_prelude);
        }
        if !fn_prelude.is_empty() {
            prelude_sections.push(fn_prelude.join("\n"));
        }
        ProgramParts { includes, prelude: prelude_sections.join("\n\n"), body }
    }

    fn is_main_call(&self, stmt: &Statement<'a>) -> bool {
        if let Statement::ExpressionStatement(es) = stmt {
            if let Expression::CallExpression(call) = &es.expression {
                if let Expression::Identifier(id) = &call.callee {
                    return id.name.as_str() == "main";
                }
            }
        }
        false
    }

    fn emit_statement(&mut self, stmt: &Statement<'a>) -> Option<String> {
        match stmt {
            Statement::VariableDeclaration(d) => self.emit_variable_declaration(d),
            Statement::FunctionDeclaration(f) => self.emit_function_declaration(f),
            Statement::ClassDeclaration(c) => Some(self.emit_class(c)),
            Statement::TSInterfaceDeclaration(i) => Some(self.emit_interface(i)),
            Statement::TSTypeAliasDeclaration(t) => self.emit_type_alias(t),
            Statement::TSEnumDeclaration(e) => Some(self.emit_enum(e)),
            Statement::BlockStatement(b) => Some(self.emit_block(b)),
            Statement::ExpressionStatement(e) => {
                if let Some(code) = self.emit_then_chain_statement(&e.expression) {
                    Some(code)
                } else {
                    Some(format!("{}{};", self.indent(), self.emit_expression(&e.expression)))
                }
            }
            Statement::ReturnStatement(r) => Some(self.emit_return(r)),
            Statement::IfStatement(i) => Some(self.emit_if(i)),
            Statement::WhileStatement(w) => Some(self.emit_while(w)),
            Statement::DoWhileStatement(d) => Some(self.emit_do_while(d)),
            Statement::ForStatement(f) => Some(self.emit_for(f)),
            Statement::ForInStatement(f) => Some(self.emit_for_in(f)),
            Statement::ForOfStatement(f) => Some(self.emit_for_of(f)),
            Statement::BreakStatement(_) => Some(format!("{}break;", self.indent())),
            Statement::ContinueStatement(_) => Some(self.emit_continue()),
            Statement::SwitchStatement(sw) => Some(self.emit_switch(sw)),
            Statement::TryStatement(t) => Some(self.emit_try(t)),
            Statement::ThrowStatement(t) => Some(self.emit_throw(t)),
            Statement::EmptyStatement(_) => None,
            Statement::LabeledStatement(l) => {
                let inner = self.emit_statement(&l.body).unwrap_or_default();
                Some(format!("{}// label {}:\n{}", self.indent(), l.label.name, inner))
            }
            Statement::TSNamespaceDeclaration(n) => self.emit_namespace(n),
            Statement::WithStatement(_) => {
                Some(format!("{}/* with not supported */", self.indent()))
            }
            Statement::DebuggerStatement(_) => None,
            Statement::ImportDeclaration(_) => None,
            Statement::ExportAllDeclaration(_) => None,
            Statement::ExportDefaultDeclaration(e) => self.emit_export_default(&e.declaration),
            Statement::ExportNamedDeclaration(_) => None,
            Statement::ExportDeclaration(e) => self.emit_declaration(&e.declaration),
            _ => None,
        }
    }

    fn emit_declaration(&mut self, decl: &Declaration<'a>) -> Option<String> {
        match decl {
            Declaration::VariableDeclaration(d) => self.emit_variable_declaration(d),
            Declaration::FunctionDeclaration(f) => self.emit_function_declaration(f),
            Declaration::ClassDeclaration(c) => Some(self.emit_class(c)),
            Declaration::TSTypeAliasDeclaration(t) => self.emit_type_alias(t),
            Declaration::TSInterfaceDeclaration(i) => Some(self.emit_interface(i)),
            Declaration::TSEnumDeclaration(e) => Some(self.emit_enum(e)),
            _ => None,
        }
    }

    fn emit_export_default(&mut self, decl: &ExportDefaultDeclarationKind<'a>) -> Option<String> {
        match decl {
            ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                self.emit_function_declaration(f)
            }
            ExportDefaultDeclarationKind::ClassDeclaration(c) => Some(self.emit_class(c)),
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(i) => Some(self.emit_interface(i)),
            _ => {
                if let Some(expr) = decl.as_expression() {
                    Some(format!("{}{};", self.indent(), self.emit_expression(expr)))
                } else {
                    None
                }
            }
        }
    }

    fn emit_type_alias(&mut self, decl: &TSTypeAliasDeclaration<'a>) -> Option<String> {
        let name = decl.id.name.to_string();
        let tp = decl
            .type_parameters
            .as_ref()
            .map(|tp| {
                let params: Vec<String> =
                    tp.params.iter().map(|p| p.name.name.to_string()).collect();
                if params.is_empty() { String::new() } else { format!("<{}>", params.join(", ")) }
            })
            .unwrap_or_default();
        let ty = resolve_type(
            Some(&decl.type_annotation),
            "auto",
            &self.ctx.template_params,
            false,
            &self.ctx.class_names,
        );
        self.ctx.need(&ty);
        Some(format!("{}using {}{} = {};", self.indent(), name, tp, ty))
    }

    fn emit_enum(&mut self, decl: &TSEnumDeclaration<'a>) -> String {
        let name = decl.id.name.to_string();
        self.ctx.class_names.insert(name.clone());
        let mut lines = vec![format!("{}enum class {} {{", self.indent(), name)];
        for m in &decl.body.members {
            let id = match &m.id {
                TSEnumMemberName::Identifier(id) => id.name.to_string(),
                TSEnumMemberName::String(s) => s.value.to_string(),
                _ => "/* member */".to_string(),
            };
            if let Some(init) = &m.initializer {
                lines.push(format!("{}{} = {},", self.indent(), id, self.emit_expression(init)));
            } else {
                lines.push(format!("{}{},", self.indent(), id));
            }
        }
        lines.push(format!("{}}};", self.indent()));
        lines.join("\n")
    }

    fn emit_variable_declaration(&mut self, decl: &VariableDeclaration<'a>) -> Option<String> {
        let kind = decl.kind.as_str().to_string();
        let mut decls = Vec::new();
        for d in &decl.declarations {
            if let Some(v) = self.emit_variable_declarator(d, &kind) {
                decls.push(v);
            }
        }
        if decls.is_empty() {
            return None;
        }
        if decls.len() == 1 {
            return Some(decls.into_iter().next().unwrap());
        }
        Some(decls.join("\n"))
    }

    fn emit_variable_declarator(
        &mut self,
        d: &VariableDeclarator<'a>,
        kind: &str,
    ) -> Option<String> {
        if !matches!(&d.id, BindingPattern::BindingIdentifier(_)) {
            if kind == "var" && !self.hoist_stack.is_empty() {
                // `var {a} = obj` binds into the hoisted scope; the
                // destructuring emitters declare fresh locals, which would
                // redeclare the prelude binding. `morph check` rejects this
                // precisely before codegen.
                return Some(format!(
                    "{}/* error: destructured var declarations are not supported */",
                    self.indent()
                ));
            }
            return self.emit_destructured(d, kind);
        }
        let (name, _) = self.binding_to_identifier(&d.id);
        if name.starts_with("/*") {
            return Some(format!("{}/* destructuring not supported */", self.indent()));
        }
        // Hoisted `var`: the declaration became a prelude binding at the
        // top of the function/program, so the original site only assigns.
        if kind == "var"
            && let Some(binding) = self.hoist_stack.last().and_then(|scope| scope.get(&name))
        {
            let late = binding.late;
            let Some(_) = d.init.as_ref() else {
                return None;
            };
            let init_code = self.hoisted_init_code(d, &name, late)?;
            return Some(format!("{}{} = {};", self.indent(), name, init_code));
        }
        // `const C = class { ... }` lowers to a named class declaration;
        // class expressions elsewhere stay rejected by `morph check`.
        if let Some(class) = d.init.as_ref().and_then(class_expression_of) {
            return Some(self.emit_class_named(class, Some(&name)));
        }
        // Track Window handles for method/property lowering: direct
        // `useWindow(...)` / `new Window(...)` initializers keep their WID
        // lowering. A late assignment (`let win; ... win = new Window()`)
        // instead forces the JS-visible `JsWindowHandle`, so reads before
        // assignment stay `undefined`. A `Window` annotation also marks
        // helpers/locals even when the initializer comes from another
        // function.
        let direct_window = d.init.as_ref().is_some_and(is_window_value_expr);
        let late_window = !direct_window && self.late_window_vars.contains(&name);
        if direct_window || late_window {
            self.ctx.window_vars.insert(name.clone());
        }
        let typed_window = d.type_annotation.as_ref().is_some_and(|ta| {
            resolve_type_annotation(
                Some(ta),
                "auto",
                &self.ctx.template_params,
                false,
                &self.ctx.class_names,
            ) == "WID"
        });
        let out = self.emit_typed_variable_declarator(d, &name, kind);
        if typed_window && !late_window {
            self.ctx.var_types.insert(name.clone(), "WID".to_string());
        }
        out
    }

    /// Destructuring declarations (`const {x} = obj`, `const [a] = arr`).
    ///
    /// One `auto` per binding. Literal containers inline each element
    /// value (single evaluation, full precision); identifier sources
    /// repeat a side-effect-free root; anything else stages through one
    /// `__destructure_N` temporary. Patterns with defaults, rest elements,
    /// or computed keys route to the recursive emitter; string iteration
    /// keeps the comment fallback below.
    /// Bindings emit plain `auto` regardless of `kind`, matching how the
    /// typed declarator treats deduced locals, so later moves keep working.
    fn emit_destructured(&mut self, d: &VariableDeclarator<'a>, _kind: &str) -> Option<String> {
        let fallback = || Some(format!("{}/* destructuring not supported */", self.indent()));
        let Some(init) = d.init.as_ref() else {
            return fallback();
        };
        if Self::pattern_has_advanced(&d.id) {
            return Some(self.emit_advanced_destructured(&d.id, init));
        }
        let mut raw: Vec<RawBinding> = Vec::new();
        if !Self::collect_access(&d.id, &mut Vec::new(), &mut raw) {
            return fallback();
        }
        if matches!(init, Expression::ArrayExpression(_) | Expression::ObjectExpression(_)) {
            if let Some(direct) = self.emit_destructured_values(&raw, init) {
                return Some(direct);
            }
            return self.emit_destructured_staged(&raw, init);
        }
        if let Expression::Identifier(id) = init {
            let container_type =
                self.ctx.var_types.get(id.name.as_str()).cloned().unwrap_or_default();
            return self.emit_destructured_access(
                &raw,
                &id.name.to_string(),
                &container_type,
                None,
            );
        }
        self.emit_destructured_staged(&raw, init)
    }

    /// Whether a pattern needs the recursive emitter: defaults, rest
    /// elements, or computed keys. Static object/array patterns keep the
    /// fast path above.
    fn pattern_has_advanced(pattern: &BindingPattern<'_>) -> bool {
        match pattern {
            BindingPattern::BindingIdentifier(_) => false,
            BindingPattern::AssignmentPattern(_) => true,
            BindingPattern::ObjectPattern(object) => {
                object.rest.is_some()
                    || object.properties.iter().any(|property| {
                        property.computed || Self::pattern_has_advanced(&property.value)
                    })
            }
            BindingPattern::ArrayPattern(array) => {
                array.rest.is_some()
                    || array
                        .elements
                        .iter()
                        .flatten()
                        .any(|element| Self::pattern_has_advanced(element))
            }
        }
    }

    fn advanced_container_type(&self, init: &Expression<'a>) -> String {
        match init {
            Expression::ParenthesizedExpression(parenthesized) => {
                self.advanced_container_type(&parenthesized.expression)
            }
            // Array literals always lower to JsArray, even when the
            // inference path would describe a native vector.
            Expression::ArrayExpression(_) => "JsArray".to_string(),
            _ => self.infer_type_from_init(init).unwrap_or_else(|| "JsValue".to_string()),
        }
    }

    /// Destructuring for defaults/rest/computed keys. The source is staged
    /// once, then the pattern is walked recursively so nested combinations
    /// and lazily evaluated defaults preserve JS evaluation order.
    fn emit_advanced_destructured(
        &mut self,
        pattern: &BindingPattern<'a>,
        init: &Expression<'a>,
    ) -> String {
        let fallback = format!("{}/* destructuring not supported */", self.indent());
        let container_type = self.advanced_container_type(init);
        let temp = format!("__destructure_{}", self.destructure_count);
        self.destructure_count += 1;
        let staged_init = self.emit_expression(init);
        self.ctx.need(&container_type);
        self.ctx.var_types.insert(temp.clone(), container_type.clone());
        let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
        let mut lines =
            vec![format!("{}{}auto {} = {};", self.indent(), prefix, temp, staged_init)];
        if !self.emit_pattern_bindings(pattern, &temp, &container_type, &mut lines) {
            return fallback;
        }
        lines.join("\n")
    }

    /// Recursively bind one pattern against an already-evaluated source.
    /// Returns false on shapes the renderer cannot express, so callers can
    /// keep their explicit fallback.
    fn emit_pattern_bindings(
        &mut self,
        pattern: &BindingPattern<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                self.push_destructured_binding(id.name.as_str(), source, source_type, lines);
                true
            }
            BindingPattern::AssignmentPattern(assign) => {
                if Self::value_may_be_undefined(source_type) {
                    let default = self.emit_expression(&assign.right);
                    self.ctx.need("JsValue");
                    // The conditional keeps the default lazy. Avoid a lambda:
                    // top-level destructuring may be emitted inside `main` or
                    // at namespace scope, where captures differ.
                    let source_is_value = Self::bare_type(source_type) == "JsValue";
                    let checked = if source_is_value {
                        source.to_string()
                    } else {
                        format!("JsValue({source})")
                    };
                    let default_source =
                        format!("({checked}.is_undefined() ? JsValue({default}) : {checked})");
                    self.emit_pattern_bindings(&assign.left, &default_source, "JsValue", lines)
                } else {
                    // Native element types are always present; a JS default
                    // here can never fire, so keep the precise native type.
                    self.emit_pattern_bindings(&assign.left, source, source_type, lines)
                }
            }
            BindingPattern::ObjectPattern(object) => {
                self.emit_object_pattern(object, source, source_type, lines)
            }
            BindingPattern::ArrayPattern(array) => {
                self.emit_array_pattern(array, source, source_type, lines)
            }
        }
    }

    fn push_destructured_binding(
        &mut self,
        name: &str,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) {
        self.ctx.need(source_type);
        let pointee =
            source_type.trim_start_matches("const ").trim_end_matches('&').trim().to_string();
        if self.box_loop_bindings
            && self.is_captured_name(name)
            && Self::is_scalar_pointee(&pointee)
        {
            let boxed_type = format!("std::shared_ptr<{pointee}>");
            self.ctx.need(&pointee);
            self.ctx.need(&boxed_type);
            self.ctx.need("std::make_shared");
            self.ctx.var_types.insert(name.to_string(), boxed_type.clone());
            self.ctx.shared_ptr_vars.insert(name.to_string());
            let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
            lines.push(format!(
                "{}{}{} {} = std::make_shared<{}>({});",
                self.indent(),
                prefix,
                boxed_type,
                name,
                pointee,
                source
            ));
            return;
        }
        self.ctx.var_types.insert(name.to_string(), source_type.to_string());
        let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
        lines.push(format!("{}{}auto {} = {};", self.indent(), prefix, name, source));
    }

    fn emit_object_pattern(
        &mut self,
        object: &ObjectPattern<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        let Some(dot) = Self::destructure_dot(source_type, &self.ctx.class_names) else {
            return false;
        };
        if dot && object.properties.iter().any(|property| property.computed) {
            return false;
        }
        if !dot && Self::vector_inner_type(source_type).is_some() {
            return false;
        }
        let mut excluded: Vec<String> = Vec::new();
        for property in &object.properties {
            let access = if let Some(name) = property.key.static_name() {
                let key = name.into_owned();
                excluded.push(cpp_string_literal(&key));
                if dot {
                    format!("{source}.{key}")
                } else {
                    format!("{source}[{}]", cpp_string_literal(&key))
                }
            } else {
                let Some(key_expr) = property.key.as_expression() else {
                    return false;
                };
                if dot {
                    return false;
                }
                let key_temp = format!("__key_{}", self.destructure_count);
                self.destructure_count += 1;
                let key_cpp = self.emit_expression(key_expr);
                self.ctx.need("std::string");
                self.ctx.var_types.insert(key_temp.clone(), "std::string".to_string());
                let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
                lines.push(format!(
                    "{}{}auto {} = morph::js_to_string({}).to_std_string();",
                    self.indent(),
                    prefix,
                    key_temp,
                    key_cpp
                ));
                excluded.push(key_temp.clone());
                format!("{source}[{key_temp}]")
            };
            if !self.emit_pattern_bindings(&property.value, &access, "JsValue", lines) {
                return false;
            }
        }
        if let Some(rest) = &object.rest {
            if dot {
                return false;
            }
            self.ctx.need("JsObject");
            let rest_source =
                format!("morph::js_object_rest({source}, {{{}}})", excluded.join(", "));
            if !self.emit_pattern_bindings(&rest.argument, &rest_source, "JsObject", lines) {
                return false;
            }
        }
        true
    }

    fn emit_array_pattern(
        &mut self,
        array: &ArrayPattern<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        match Self::destructure_dot(source_type, &self.ctx.class_names) {
            Some(false) => {}
            Some(true) | None => return false,
        }
        let bare = Self::bare_type(source_type);
        if bare == "JsObject" {
            return false;
        }
        let elem_type = Self::destructured_array_element_type(source_type);
        for (index, element) in array.elements.iter().enumerate() {
            let Some(nested) = element else {
                continue;
            };
            let access = format!("{source}[{index}]");
            if !self.emit_pattern_bindings(nested, &access, &elem_type, lines) {
                return false;
            }
        }
        if let Some(rest) = &array.rest {
            let start = array.elements.len();
            let Some((rest_source, rest_type)) =
                self.array_rest_expression(source, source_type, start)
            else {
                return false;
            };
            if !self.emit_pattern_bindings(&rest.argument, &rest_source, &rest_type, lines) {
                return false;
            }
        }
        true
    }

    /// Destructuring assignment (`[a, b] = rhs`, `({ c } = o)`).
    ///
    /// The right-hand side is staged once inside an immediately-invoked
    /// lambda so every target reads the same value and computed keys run
    /// exactly once. The lambda returns the staged value, matching the
    /// value of a JavaScript assignment expression. Module-scope
    /// assignments use a captureless lambda because C++ forbids a
    /// capture-default outside a function.
    fn emit_destructuring_assignment(&mut self, a: &AssignmentExpression<'a>) -> String {
        let container_type = self.advanced_container_type(&a.right);
        let temp = format!("__assign_{}", self.destructure_count);
        self.destructure_count += 1;
        let rhs = self.emit_expression(&a.right);
        self.ctx.need(&container_type);
        self.ctx.var_types.insert(temp.clone(), container_type.clone());

        let capture =
            if self.ctx.fn_body_depth > 0 || self.ctx.arrow_depth > 0 || self.ctx.fn_expr_depth > 0
            {
                "[&]"
            } else {
                "[]"
            };
        let base_indent = self.indent();
        let inner_indent = format!("{base_indent}{INDENT}");
        let mut lines = vec![format!("({capture}() {{")];
        lines.push(format!("{inner_indent}auto {temp} = {rhs};"));

        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level += 1;
        let mut assignments = Vec::new();
        let ok = self.emit_assignment_pattern(&a.left, &temp, &container_type, &mut assignments);
        self.ctx.indent_level = old_indent;
        if !ok {
            return format!("{}/* destructuring assignment not supported */", self.indent());
        }
        lines.extend(assignments);
        lines.push(format!("{inner_indent}return {temp};"));
        lines.push(format!("{base_indent}}})()"));
        lines.join("\n")
    }

    fn emit_assignment_pattern(
        &mut self,
        target: &AssignmentTarget<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                self.push_identifier_assignment(id.name.as_str(), source, source_type, lines);
                true
            }
            AssignmentTarget::StaticMemberExpression(_)
            | AssignmentTarget::ComputedMemberExpression(_)
            | AssignmentTarget::PrivateFieldExpression(_) => {
                let target_code = self.emit_assignment_target(target);
                let target_type = self.assignment_target_type(target);
                self.push_value_assignment(
                    &target_code,
                    target_type.as_deref(),
                    source,
                    source_type,
                    lines,
                );
                true
            }
            AssignmentTarget::ArrayAssignmentTarget(array) => {
                self.emit_assignment_array(array, source, source_type, lines)
            }
            AssignmentTarget::ObjectAssignmentTarget(object) => {
                self.emit_assignment_object(object, source, source_type, lines)
            }
            _ => false,
        }
    }

    fn emit_assignment_maybe_default(
        &mut self,
        target: &AssignmentTargetMaybeDefault<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        match target {
            AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(with_default) => {
                if Self::value_may_be_undefined(source_type) {
                    let default = self.emit_expression(&with_default.init);
                    self.ctx.need("JsValue");
                    let checked = if Self::bare_type(source_type) == "JsValue" {
                        source.to_string()
                    } else {
                        format!("JsValue({source})")
                    };
                    let default_source =
                        format!("({checked}.is_undefined() ? JsValue({default}) : {checked})");
                    self.emit_assignment_pattern(
                        &with_default.binding,
                        &default_source,
                        "JsValue",
                        lines,
                    )
                } else {
                    self.emit_assignment_pattern(&with_default.binding, source, source_type, lines)
                }
            }
            _ => target.as_assignment_target().is_some_and(|target| {
                self.emit_assignment_pattern(target, source, source_type, lines)
            }),
        }
    }

    fn emit_assignment_array(
        &mut self,
        array: &ArrayAssignmentTarget<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        match Self::destructure_dot(source_type, &self.ctx.class_names) {
            Some(false) => {}
            Some(true) | None => return false,
        }
        if Self::bare_type(source_type) == "JsObject" {
            return false;
        }
        let elem_type = Self::destructured_array_element_type(source_type);
        for (index, element) in array.elements.iter().enumerate() {
            let Some(nested) = element else {
                continue;
            };
            let access = format!("{source}[{index}]");
            if !self.emit_assignment_maybe_default(nested, &access, &elem_type, lines) {
                return false;
            }
        }
        if let Some(rest) = &array.rest {
            let start = array.elements.len();
            let Some((rest_source, rest_type)) =
                self.array_rest_expression(source, source_type, start)
            else {
                return false;
            };
            if !self.emit_assignment_pattern(&rest.target, &rest_source, &rest_type, lines) {
                return false;
            }
        }
        true
    }

    fn emit_assignment_object(
        &mut self,
        object: &ObjectAssignmentTarget<'a>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) -> bool {
        let Some(access) = Self::assignment_object_access(source_type, &self.ctx.class_names)
        else {
            return false;
        };
        let dot = access != "[]";
        if dot
            && object.properties.iter().any(|property| {
                matches!(
                    property,
                    AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) if p.computed
                )
            })
        {
            return false;
        }
        let mut excluded: Vec<String> = Vec::new();
        for property in &object.properties {
            match property {
                AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p) => {
                    let key = p.binding.name.to_string();
                    excluded.push(cpp_string_literal(&key));
                    let field_type = if dot {
                        self.assignment_object_field_type(source_type, &key)
                    } else {
                        "JsValue".to_string()
                    };
                    let source_expr = if dot {
                        format!("{source}{access}{key}")
                    } else {
                        format!("{source}[{}]", cpp_string_literal(&key))
                    };
                    if let Some(init) = &p.init {
                        if !self.emit_assignment_identifier_default(
                            p.binding.name.as_str(),
                            &source_expr,
                            &field_type,
                            init,
                            lines,
                        ) {
                            return false;
                        }
                    } else {
                        self.push_identifier_assignment(
                            p.binding.name.as_str(),
                            &source_expr,
                            &field_type,
                            lines,
                        );
                    }
                }
                AssignmentTargetProperty::AssignmentTargetPropertyProperty(p) => {
                    if !self.emit_assignment_object_property(
                        p,
                        source,
                        source_type,
                        access,
                        dot,
                        &mut excluded,
                        lines,
                    ) {
                        return false;
                    }
                }
            }
        }
        if let Some(rest) = &object.rest {
            if dot {
                return false;
            }
            self.ctx.need("JsObject");
            let rest_source =
                format!("morph::js_object_rest({source}, {{{}}})", excluded.join(", "));
            if !self.emit_assignment_pattern(&rest.target, &rest_source, "JsObject", lines) {
                return false;
            }
        }
        true
    }

    fn emit_assignment_object_property(
        &mut self,
        property: &AssignmentTargetPropertyProperty<'a>,
        source: &str,
        source_type: &str,
        access: &str,
        dot: bool,
        excluded: &mut Vec<String>,
        lines: &mut Vec<String>,
    ) -> bool {
        let (source_expr, field_type) = if let Some(name) = property.name.static_name() {
            let key = name.into_owned();
            excluded.push(cpp_string_literal(&key));
            let field_type = if dot {
                self.assignment_object_field_type(source_type, &key)
            } else {
                "JsValue".to_string()
            };
            let source_expr = if dot {
                format!("{source}{access}{key}")
            } else {
                format!("{source}[{}]", cpp_string_literal(&key))
            };
            (source_expr, field_type)
        } else {
            let Some(key_expr) = property.name.as_expression() else {
                return false;
            };
            if dot {
                return false;
            }
            let key_temp = format!("__key_{}", self.destructure_count);
            self.destructure_count += 1;
            let key_cpp = self.emit_expression(key_expr);
            self.ctx.need("std::string");
            self.ctx.need("JsValue");
            let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
            lines.push(format!(
                "{}{prefix}auto {key_temp} = morph::js_to_string({key_cpp}).to_std_string();",
                self.indent()
            ));
            excluded.push(key_temp.clone());
            (format!("{source}[{key_temp}]"), "JsValue".to_string())
        };
        self.emit_assignment_maybe_default(&property.binding, &source_expr, &field_type, lines)
    }

    fn emit_assignment_identifier_default(
        &mut self,
        name: &str,
        source: &str,
        source_type: &str,
        init: &Expression<'a>,
        lines: &mut Vec<String>,
    ) -> bool {
        if Self::value_may_be_undefined(source_type) {
            let default = self.emit_expression(init);
            self.ctx.need("JsValue");
            let checked = if Self::bare_type(source_type) == "JsValue" {
                source.to_string()
            } else {
                format!("JsValue({source})")
            };
            let default_source =
                format!("({checked}.is_undefined() ? JsValue({default}) : {checked})");
            self.push_identifier_assignment(name, &default_source, "JsValue", lines);
        } else {
            self.push_identifier_assignment(name, source, source_type, lines);
        }
        true
    }

    fn push_identifier_assignment(
        &self,
        name: &str,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) {
        let target_type = self.ctx.var_types.get(name).cloned();
        if let Some(set_head) = self.signal_set_head(name) {
            let value =
                Self::convert_assignment_source(target_type.as_deref(), source_type, source);
            lines.push(format!("{}{}({});", self.indent(), set_head, value));
            return;
        }
        let target =
            self.active_narrows.get(name).cloned().unwrap_or_else(|| {
                self.deref_shared_scalar(name).unwrap_or_else(|| name.to_string())
            });
        let value = Self::convert_assignment_source(target_type.as_deref(), source_type, source);
        lines.push(format!("{}{} = {};", self.indent(), target, value));
    }

    fn push_value_assignment(
        &self,
        target: &str,
        target_type: Option<&str>,
        source: &str,
        source_type: &str,
        lines: &mut Vec<String>,
    ) {
        let value = Self::convert_assignment_source(target_type, source_type, source);
        lines.push(format!("{}{} = {};", self.indent(), target, value));
    }

    fn convert_assignment_source(
        target_type: Option<&str>,
        source_type: &str,
        source: &str,
    ) -> String {
        let Some(target_type) = target_type else {
            return source.to_string();
        };
        let target = Self::bare_type(target_type);
        let int_target = matches!(
            target,
            "int" | "int32_t" | "int64_t" | "size_t" | "uint" | "uint32_t" | "uint64_t" | "uint8_t"
        );
        let float_target = matches!(target, "float" | "double");
        if !int_target && !float_target {
            return source.to_string();
        }
        let source_bare = Self::bare_type(source_type);
        if !matches!(source_bare, "JsValue" | "JsNumber" | "JsLateValue" | "auto" | "") {
            return source.to_string();
        }
        let primitive = if source_bare == "JsNumber" {
            source.to_string()
        } else {
            format!("std::get<JsNumber>(JsValue({source}).inner)")
        };
        if int_target {
            format!("({primitive}).as_int()")
        } else {
            format!("({primitive}).as_double()")
        }
    }

    fn assignment_target_type(&self, target: &AssignmentTarget<'a>) -> Option<String> {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                self.ctx.var_types.get(id.name.as_str()).cloned()
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                if matches!(&member.object, Expression::ThisExpression(_)) {
                    let class = self.ctx.class_name.as_ref()?;
                    return self
                        .ctx
                        .class_field_types
                        .get(class)
                        .and_then(|fields| fields.get(member.property.name.as_str()))
                        .cloned();
                }
                if let Expression::Identifier(id) = &member.object {
                    if self.ctx.class_names.contains(id.name.as_str()) {
                        return self
                            .ctx
                            .class_field_types
                            .get(id.name.as_str())
                            .and_then(|fields| fields.get(member.property.name.as_str()))
                            .cloned();
                    }
                    let obj_type = self.ctx.var_types.get(id.name.as_str())?;
                    let class = Self::class_name_from_type(obj_type)?;
                    return self
                        .ctx
                        .class_field_types
                        .get(&class)
                        .and_then(|fields| fields.get(member.property.name.as_str()))
                        .cloned();
                }
                None
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                let obj_type = self.infer_type_from_init(&member.object)?;
                if matches!(
                    Self::bare_type(&obj_type),
                    "JsObject" | "JsValue" | "JsArray" | "JsLateValue"
                ) {
                    return Some("JsValue".to_string());
                }
                Self::vector_inner_type(&obj_type).map(|inner| inner.trim().to_string())
            }
            AssignmentTarget::PrivateFieldExpression(private) => {
                if matches!(&private.object, Expression::ThisExpression(_)) {
                    let class = self.ctx.class_name.as_ref()?;
                    return self.ctx.class_field_types.get(class).and_then(|fields| {
                        fields.get(&format!("#{}", private.field.name)).cloned()
                    });
                }
                None
            }
            _ => None,
        }
    }

    fn class_name_from_type(ty: &str) -> Option<String> {
        let bare = Self::bare_type(ty);
        if let Some(inner) =
            bare.strip_prefix("std::shared_ptr<").and_then(|value| value.strip_suffix('>'))
        {
            return Some(inner.trim().to_string());
        }
        if let Some(inner) =
            bare.strip_prefix("std::unique_ptr<").and_then(|value| value.strip_suffix('>'))
        {
            return Some(inner.trim().to_string());
        }
        let first = bare.chars().next()?;
        if first.is_ascii_uppercase() { Some(bare.to_string()) } else { None }
    }

    fn assignment_object_access(
        source_type: &str,
        class_names: &HashSet<String>,
    ) -> Option<&'static str> {
        let bare = Self::bare_type(source_type);
        if bare.starts_with("std::shared_ptr<") || bare.starts_with("std::unique_ptr<") {
            return Some("->");
        }
        if class_names.contains(bare) {
            return Some(".");
        }
        if matches!(bare, "JsObject" | "JsValue" | "JsLateValue" | "auto" | "") {
            return Some("[]");
        }
        None
    }

    fn assignment_object_field_type(&self, source_type: &str, key: &str) -> String {
        let bare = Self::bare_type(source_type);
        let class = if bare.starts_with("std::shared_ptr<")
            || bare.starts_with("std::unique_ptr<")
            || self.ctx.class_names.contains(bare)
        {
            Self::class_name_from_type(bare)
        } else {
            None
        };
        class
            .and_then(|class| {
                self.ctx.class_field_types.get(&class).and_then(|fields| fields.get(key)).cloned()
            })
            .unwrap_or_else(|| "JsValue".to_string())
    }

    fn array_rest_expression(
        &mut self,
        source: &str,
        source_type: &str,
        start: usize,
    ) -> Option<(String, String)> {
        if let Some(inner) = Self::vector_inner_type(source_type) {
            let inner = inner.trim();
            let ty = format!("std::vector<{inner}>");
            self.ctx.need(&ty);
            self.ctx.need("JsValue");
            return Some((format!("morph::js_array_rest({source}, {start})"), ty));
        }
        let bare = Self::bare_type(source_type);
        if matches!(bare, "JsArray" | "JsValue" | "auto" | "") {
            self.ctx.need("JsArray");
            return Some((
                format!("morph::js_array_rest({source}, {start})"),
                "JsArray".to_string(),
            ));
        }
        None
    }

    fn destructured_array_element_type(source_type: &str) -> String {
        Self::vector_inner_type(source_type)
            .map_or_else(|| "JsValue".to_string(), |inner| inner.trim().to_string())
    }

    fn bare_type(source_type: &str) -> &str {
        source_type.trim_start_matches("const ").trim_end_matches('&').trim()
    }

    fn value_may_be_undefined(source_type: &str) -> bool {
        matches!(Self::bare_type(source_type), "" | "auto" | "JsValue" | "JsUndefined")
    }

    /// Bindings off a literal container, inlining each element value.
    /// `None` when any step leaves the literal (the caller restages).
    fn emit_destructured_values(
        &mut self,
        raw: &[RawBinding],
        init: &Expression<'a>,
    ) -> Option<String> {
        let mut lines: Vec<String> = Vec::new();
        for binding in raw {
            let value = Self::literal_value(init, &binding.steps)?;
            let rendered = self.emit_expression(value);
            let elem_type = self.infer_type_from_init(value).unwrap_or("JsValue".to_string());
            self.ctx.need(&elem_type);
            self.ctx.var_types.insert(binding.name.clone(), elem_type);
            let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
            lines.push(format!("{}{}auto {} = {};", self.indent(), prefix, binding.name, rendered));
        }
        Some(lines.join("\n"))
    }

    /// Bindings off an arbitrary source, staged once through a temporary
    /// so the source evaluates exactly once no matter how many names bind.
    fn emit_destructured_staged(
        &mut self,
        raw: &[RawBinding],
        init: &Expression<'a>,
    ) -> Option<String> {
        let container_type = self.infer_type_from_init(init).unwrap_or("JsValue".to_string());
        let temp = format!("__destructure_{}", self.destructure_count);
        self.destructure_count += 1;
        let staged_init = self.emit_expression(init);
        self.ctx.need(&container_type);
        self.ctx.var_types.insert(temp.clone(), container_type.clone());
        let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
        let staged_line = format!("{}{}auto {} = {};", self.indent(), prefix, temp, staged_init);
        self.emit_destructured_access(raw, &temp, &container_type, Some(staged_line))
    }

    /// Bindings off a repeatable root (identifier or staged temporary),
    /// rendered as member access. Key steps need an object-shaped root;
    /// vectors and arrays only serve indices.
    fn emit_destructured_access(
        &mut self,
        raw: &[RawBinding],
        base: &str,
        container_type: &str,
        staged_line: Option<String>,
    ) -> Option<String> {
        let fallback = || Some(format!("{}/* destructuring not supported */", self.indent()));
        let dot = match Self::destructure_dot(container_type, &self.ctx.class_names) {
            Some(dot) => dot,
            None => return fallback(),
        };
        if !dot
            && (container_type.starts_with("std::vector<") || container_type == "JsArray")
            && raw
                .iter()
                .any(|binding| binding.steps.iter().any(|step| matches!(step, AccessStep::Key(_))))
        {
            return fallback();
        }
        let mut lines: Vec<String> = Vec::new();
        if let Some(staged) = staged_line {
            lines.push(staged);
        }
        for binding in raw {
            let access = Self::render_access(base, dot, &binding.steps);
            let elem_type = Self::access_elem_type(container_type, &binding.steps);
            self.ctx.need(&elem_type);
            self.ctx.var_types.insert(binding.name.clone(), elem_type);
            let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
            lines.push(format!("{}{}auto {} = {};", self.indent(), prefix, binding.name, access));
        }
        Some(lines.join("\n"))
    }

    /// Member style for destructured access off a container type. `None`
    /// marks containers destructuring cannot read (strings, scalars).
    /// Bracket access on an unknown root assumes the usual dynamic value.
    fn destructure_dot(container_type: &str, class_names: &HashSet<String>) -> Option<bool> {
        let bare = container_type.trim_start_matches("const ").trim_end_matches('&').trim();
        if bare.starts_with("std::shared_ptr<")
            || bare.starts_with("std::unique_ptr<")
            || class_names.contains(bare)
        {
            return Some(true);
        }
        if bare == "JsObject"
            || bare == "JsValue"
            || bare == "JsLateValue"
            || bare == "auto"
            || bare.is_empty()
            || bare.starts_with("std::vector<")
            || bare == "JsArray"
        {
            return Some(false);
        }
        None
    }

    /// Element type behind one access path. Only a first-level vector
    /// index carries a precise type; everything else reads boxed.
    fn access_elem_type(container_type: &str, steps: &[AccessStep]) -> String {
        if let [AccessStep::Index(_)] = steps {
            if let Some(inner) = Self::vector_inner_type(container_type) {
                return inner.trim().to_string();
            }
        }
        "JsValue".to_string()
    }

    /// Absolute access path per bound name for the static fast path.
    /// Returns `false` on defaults, rest, or computed keys; callers route
    /// those through `emit_pattern_bindings` instead.
    fn collect_access(
        pattern: &BindingPattern,
        steps: &mut Vec<AccessStep>,
        out: &mut Vec<RawBinding>,
    ) -> bool {
        match pattern {
            BindingPattern::BindingIdentifier(id) => {
                out.push(RawBinding { name: id.name.to_string(), steps: steps.clone() });
                true
            }
            BindingPattern::ObjectPattern(object) => {
                for prop in &object.properties {
                    if prop.computed {
                        return false;
                    }
                    let Some(key) = EscapeAnalyzer::pattern_key_name(&prop.key) else {
                        return false;
                    };
                    steps.push(AccessStep::Key(key));
                    if !Self::collect_access_default(&prop.value, steps, out) {
                        return false;
                    }
                    steps.pop();
                }
                if object.rest.is_some() {
                    return false;
                }
                true
            }
            BindingPattern::ArrayPattern(array) => {
                for (index, element) in array.elements.iter().enumerate() {
                    let Some(nested) = element else {
                        continue;
                    };
                    steps.push(AccessStep::Index(index));
                    if !Self::collect_access_default(nested, steps, out) {
                        return false;
                    }
                    steps.pop();
                }
                if array.rest.is_some() {
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    /// The fast path cannot express defaults; advanced patterns are caught
    /// by `pattern_has_advanced` before they reach this collector.
    fn collect_access_default(
        pattern: &BindingPattern,
        steps: &mut Vec<AccessStep>,
        out: &mut Vec<RawBinding>,
    ) -> bool {
        if matches!(pattern, BindingPattern::AssignmentPattern(_)) {
            return false;
        }
        Self::collect_access(pattern, steps, out)
    }

    /// Render one access path off a root expression string.
    fn render_access(base: &str, dot: bool, steps: &[AccessStep]) -> String {
        let mut out = base.to_string();
        for step in steps {
            match step {
                AccessStep::Key(key) => {
                    if dot {
                        out.push('.');
                        out.push_str(key);
                    } else {
                        out.push_str(&format!("[\"{}\"]", key));
                    }
                }
                AccessStep::Index(index) => {
                    out.push_str(&format!("[{}]", index));
                }
            }
        }
        out
    }

    /// Follow an access path through literal containers. `None` as soon
    /// as any step leaves the literal world.
    fn literal_value<'x, 'y>(
        init: &'x Expression<'y>,
        steps: &[AccessStep],
    ) -> Option<&'x Expression<'y>> {
        let mut current = init;
        for step in steps {
            current = match (current, step) {
                (Expression::ObjectExpression(object), AccessStep::Key(key)) => {
                    Self::object_prop_value(object, key)?
                }
                (Expression::ArrayExpression(array), AccessStep::Index(index)) => {
                    array.elements.get(*index)?.as_expression()?
                }
                _ => return None,
            };
        }
        Some(current)
    }

    /// The value expression for `key` in an object literal.
    fn object_prop_value<'x, 'y>(
        object: &'x ObjectExpression<'y>,
        key: &str,
    ) -> Option<&'x Expression<'y>> {
        for prop in &object.properties {
            if let ObjectPropertyKind::ObjectProperty(property) = prop {
                if EscapeAnalyzer::pattern_key_name(&property.key).as_deref() == Some(key) {
                    return Some(&property.value);
                }
            }
        }
        None
    }

    // Intent-based declarator: base type, widening, trusted annotations,
    // integer-range selection, then escape-based allocation. This is the only
    // variable-declaration path.
    //
    // A `new C()` initializer infers as `shared_ptr<C>`, but the declarator
    // owns the single-owner vs shared decision: unwrap to the bare class so
    // escape analysis picks the wrapper instead of inheriting a guess.
    fn new_class_name_of(&self, cpp_type: &str, init: &Option<Expression<'a>>) -> Option<String> {
        let Some(Expression::NewExpression(new_expr)) = init else {
            return None;
        };
        let Expression::Identifier(callee) = &new_expr.callee else {
            return None;
        };
        if !self.ctx.class_names.contains(callee.name.as_str()) {
            return None;
        }
        if *cpp_type == format!("std::shared_ptr<{}>", callee.name.as_str()) {
            return Some(callee.name.to_string());
        }
        None
    }

    /// Constructor arguments of a `new C(...)` initializer, unwrapped.
    /// Each escape arm below chooses its own allocation around them.
    fn new_class_args(&mut self, init: &Option<Expression<'a>>) -> Option<String> {
        let Some(Expression::NewExpression(new_expr)) = init else {
            return None;
        };
        if !matches!(&new_expr.callee, Expression::Identifier(_)) {
            return None;
        }
        let args: Vec<String> =
            new_expr.arguments.iter().map(|arg| self.emit_argument(arg)).collect();
        Some(args.join(", "))
    }

    /// Adopt a known factory return type for `auto x = f()`.
    ///
    /// Returns the adopted type plus whether the call must be wrapped in a
    /// shared construction at the use site. A unique result stays unique
    /// only under single ownership; anything shared wraps instead, since a
    /// `unique_ptr` cannot be copied into a second owner. Wrapping applies
    /// to plain locals only, where the declarator below emits the wrapper
    /// directly around the call.
    fn adopted_call_result(
        &self,
        name: &str,
        raw_escape: &EscapeKind,
        init: &Option<Expression<'a>>,
    ) -> Option<(String, bool)> {
        let Some(Expression::CallExpression(call)) = init else {
            return None;
        };
        let Expression::Identifier(callee) = &call.callee else {
            return None;
        };
        let recorded = self.ctx.fn_return_types.get(callee.name.as_str())?;
        let inner = recorded.strip_prefix("std::unique_ptr<")?.strip_suffix('>')?;
        let Some(analysis) = self.analysis.as_ref() else {
            return None;
        };
        let single_owner = analysis
            .var_infos
            .get(name)
            .is_some_and(|info| !info.has_loop_use && info.read_use_count() <= 1)
            && !analysis
                .closure_captures
                .values()
                .any(|captured| captured.iter().any(|held| held == name));
        if single_owner {
            return Some((recorded.clone(), false));
        }
        if matches!(raw_escape, EscapeKind::None) {
            let shared = format!("std::shared_ptr<{}>", inner);
            return Some((shared, true));
        }
        None
    }

    /// True when `name` uses the fetch Response API (`r.ok()`, `r.status`,
    /// `r.text()`, ...) anywhere in the translated source. Only consulted
    /// for `await fetch(...)` declarators, so the scan is narrow and the
    /// body-text default is untouched.
    fn uses_response_api(&self, name: &str) -> bool {
        const MEMBERS: &[&str] = &[
            "ok",
            "status",
            "statusText",
            "headers",
            "text",
            "json",
            "arrayBuffer",
            "clone",
            "bodyUsed",
            "redirected",
            "type",
            "url",
        ];
        let src = self.source.as_bytes();
        let name_bytes = name.as_bytes();
        if name_bytes.is_empty() {
            return false;
        }
        let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
        let mut i = 0;
        while i + name_bytes.len() + 1 < src.len() {
            if &src[i..i + name_bytes.len()] == name_bytes
                && (i == 0 || !is_ident(src[i - 1]))
                && src[i + name_bytes.len()] == b'.'
            {
                let rest = &src[i + name_bytes.len() + 1..];
                for member in MEMBERS {
                    if rest.len() > member.len()
                        && &rest[..member.len()] == member.as_bytes()
                        && !is_ident(rest[member.len()])
                    {
                        return true;
                    }
                }
                i += name_bytes.len() + 1;
            } else {
                i += 1;
            }
        }
        false
    }

    fn declarator_storage(&mut self, d: &VariableDeclarator<'a>, name: &str) -> DeclaratorStorage {
        if let Some(init) = &d.init {
            if let Expression::AwaitExpression(awaited) = init {
                if let Expression::CallExpression(call) = &awaited.argument {
                    if let Expression::Identifier(callee) = &call.callee {
                        if callee.name.as_str() == "fetch" && self.uses_response_api(name) {
                            self.ctx.needed.insert("\"../../runtime/cpp/net/net.h\"".to_string());
                            return DeclaratorStorage {
                                alloc_type: "morph::net::Response".to_string(),
                                cpp_type: "morph::net::Response".to_string(),
                                widened: WidenedType::None,
                                escape_kind: EscapeKind::None,
                                new_class_name: None,
                                shared_wrap_call: false,
                            };
                        }
                    }
                }
            }
        }
        let base_type = self.declaration_base_type(d);
        let widened = self
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.widens.get(name))
            .cloned()
            .unwrap_or(WidenedType::None);
        let mut cpp_type = self.apply_declaration_widening(d, name, base_type, widened.clone());
        // Intent-based: Promise<T> annotated but init is sync plain call -> use T (match JS runtime)
        if let Some(init) = &d.init {
            if let Some(stripped) = self.strip_result_for_sync_call(&cpp_type, init) {
                cpp_type = stripped;
            }
        }
        // Spread arrays concatenate at runtime, so the declaration must be
        // a JsArray no matter what element inference or widening said.
        if let Some(init) = &d.init {
            if Self::is_spread_array(init) {
                cpp_type = "JsArray".to_string();
            }
        }
        // A local that calls a lowered Array method becomes `JsArray`:
        // shared element storage keeps `sort`/`splice`/`push` mutations and
        // callback results consistent with JS. Other vectors stay native.
        if self.js_array_receiver_vars.contains(name)
            && (cpp_type == "auto" || cpp_type == "JsValue" || cpp_type.starts_with("std::vector<"))
        {
            cpp_type = "JsArray".to_string();
        }
        // Window handles: direct initializers stay WID ints (the native
        // registry key). Late-assigned locals use the JS-visible
        // `JsWindowHandle = JsValue` form so `console.log(win)` prints
        // `undefined` before assignment; handle operations narrow through
        // `window_wid(...)` at use sites.
        if d.init.as_ref().is_some_and(is_window_value_expr) {
            cpp_type = "WID".to_string();
        } else if self.late_window_vars.contains(name) {
            cpp_type = "JsWindowHandle".to_string();
        } else if d.type_annotation.is_none()
            && (d.init.is_none()
                || (self.late_value_vars.contains(name)
                    && d.init.as_ref().is_some_and(is_nullish_value_expr)))
        {
            // `let x;` and `let x = null/undefined` bindings are JS
            // values: reads before the assignment stay `undefined`, and
            // later assignments can hold any value.
            cpp_type = "JsLateValue".to_string();
        }
        let new_class_name = self.new_class_name_of(&cpp_type, &d.init);
        if let Some(class_name) = &new_class_name {
            cpp_type = class_name.clone();
        }
        let escape_kind = self
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.escapes.get(name))
            .cloned()
            .unwrap_or(EscapeKind::None);
        let mut shared_wrap_call = false;
        if cpp_type == "auto" {
            if let Some((adopted, wrap)) = self.adopted_call_result(name, &escape_kind, &d.init) {
                cpp_type = adopted;
                shared_wrap_call = wrap;
            }
        }
        let escape_kind = if cpp_type == "auto" {
            EscapeKind::None
        } else {
            match escape_kind {
                EscapeKind::Return | EscapeKind::Global
                    if !Self::needs_heap_for_escape(&cpp_type, &self.ctx.class_names) =>
                {
                    EscapeKind::None
                }
                EscapeKind::MultipleRefs
                    if !Self::needs_heap_for_escape(&cpp_type, &self.ctx.class_names) =>
                {
                    // Aliased value types copy soundly: native scalars and
                    // strings are values, and Js containers share storage
                    // internally, so a heap wrapper only adds indirection.
                    // (Closure and async escapes still wrap: they extend
                    // lifetime past the frame, which a copy cannot do.)
                    EscapeKind::None
                }
                other => other,
            }
        };
        let alloc_type = if let Some(class_name) = &new_class_name {
            self.ctx.need("JsValue");
            match escape_kind {
                EscapeKind::Return | EscapeKind::Global => {
                    format!("std::unique_ptr<{class_name}>")
                }
                _ => format!("std::shared_ptr<{class_name}>"),
            }
        } else if d.init.is_some() {
            match escape_kind {
                EscapeKind::None => cpp_type.clone(),
                EscapeKind::Return | EscapeKind::Global => {
                    format!("std::unique_ptr<{cpp_type}>")
                }
                EscapeKind::ClosureCapture
                | EscapeKind::MultipleRefs
                | EscapeKind::AsyncBoundary => {
                    format!("std::shared_ptr<{cpp_type}>")
                }
            }
        } else {
            match escape_kind {
                EscapeKind::ClosureCapture
                | EscapeKind::MultipleRefs
                | EscapeKind::AsyncBoundary => {
                    format!("std::shared_ptr<{cpp_type}>")
                }
                EscapeKind::Return | EscapeKind::Global => {
                    format!("std::unique_ptr<{cpp_type}>")
                }
                _ => cpp_type.clone(),
            }
        };
        DeclaratorStorage {
            alloc_type,
            cpp_type,
            widened,
            escape_kind,
            new_class_name,
            shared_wrap_call,
        }
    }

    fn apply_declarator_storage(&mut self, name: &str, storage: &DeclaratorStorage) {
        if storage.alloc_type.starts_with("std::shared_ptr<") {
            self.ctx.shared_ptr_vars.insert(name.to_string());
        }
        if storage.alloc_type.starts_with("std::unique_ptr<") {
            self.ctx.unique_ptr_vars.insert(name.to_string());
        }
        self.ctx.need(&storage.alloc_type);
        self.ctx.var_types.insert(name.to_string(), storage.alloc_type.clone());
    }

    fn declarator_init_code(
        &mut self,
        d: &VariableDeclarator<'a>,
        storage: &DeclaratorStorage,
    ) -> Option<String> {
        let init = d.init.as_ref()?;
        if let Expression::AwaitExpression(awaited) = init {
            if let Expression::CallExpression(call) = &awaited.argument {
                if let Expression::Identifier(callee) = &call.callee {
                    if callee.name.as_str() == "fetch"
                        && storage.alloc_type == "morph::net::Response"
                    {
                        let args: Vec<String> =
                            call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                        return Some(format!(
                            "co_await morph::net::fetch_response({})",
                            args.join(", ")
                        ));
                    }
                }
            }
        }
        let init_code = if let Some(class_name) = &storage.new_class_name {
            let args = self.new_class_args(&d.init).unwrap_or_default();
            let call_args = if args.is_empty() {
                format!("\"{class_name}\"")
            } else {
                format!("\"{class_name}\", {args}")
            };
            match storage.escape_kind {
                EscapeKind::Return | EscapeKind::Global => {
                    format!("morph::js_new_unique<{class_name}>({call_args})")
                }
                _ => format!("morph::js_new<{class_name}>({call_args})"),
            }
        } else {
            match storage.escape_kind {
                EscapeKind::None => {
                    if storage.shared_wrap_call {
                        format!("{}({})", storage.alloc_type, self.emit_expression(init))
                    } else if storage.cpp_type.starts_with("std::vector<")
                        && matches!(init, Expression::ArrayExpression(_))
                    {
                        self.emit_vector_literal_typed(init, Some(&storage.cpp_type))
                    } else if storage.cpp_type == "char"
                        && matches!(init, Expression::StringLiteral(_))
                    {
                        self.emit_char_literal(init)
                    } else {
                        let code = self.emit_expression(init);
                        if matches!(storage.cpp_type.as_str(), "JsValue" | "JsLateValue") {
                            if let Some(inferred) = self.infer_type_from_init(init) {
                                if inferred.starts_with("std::vector<") {
                                    return Some(format!("JsArray({code})"));
                                }
                            }
                        }
                        code
                    }
                }
                EscapeKind::Return | EscapeKind::Global => {
                    if matches!(init, Expression::ArrayExpression(_)) {
                        format!(
                            "std::make_unique<{}>(std::vector{{{}}})",
                            storage.cpp_type,
                            self.emit_vector_literal_typed(init, Some(&storage.cpp_type))
                                .trim_start_matches('{')
                                .trim_end_matches('}')
                        )
                    } else {
                        format!(
                            "std::make_unique<{}>({})",
                            storage.cpp_type,
                            self.emit_expression(init)
                        )
                    }
                }
                EscapeKind::ClosureCapture
                | EscapeKind::MultipleRefs
                | EscapeKind::AsyncBoundary => {
                    format!(
                        "std::make_shared<{}>({})",
                        storage.cpp_type,
                        self.emit_expression(init)
                    )
                }
            }
        };
        let init_code = match Self::is_trusted_native_number(d, &storage.widened) {
            Some(_) => self.convert_number_value(&storage.cpp_type, init, init_code),
            None if storage.cpp_type == "JsString"
                && matches!(
                    self.operand_class_of(init),
                    OperandClass::Integer | OperandClass::Float
                ) =>
            {
                format!("JsString(JsNumber({}))", init_code)
            }
            None => init_code,
        };
        Some(init_code)
    }

    fn emit_typed_variable_declarator(
        &mut self,
        d: &VariableDeclarator<'a>,
        name: &str,
        kind: &str,
    ) -> Option<String> {
        let storage = self.declarator_storage(d, name);
        let init_code = self.declarator_init_code(d, &storage).unwrap_or_default();
        self.apply_declarator_storage(name, &storage);

        let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
        let mut final_type = storage.alloc_type;
        if kind == "const"
            && matches!(
                final_type.as_str(),
                "JsNumber" | "JsBoolean" | "JsString" | "JsUndefined" | "JsNull"
            )
        {
            if !final_type.starts_with("const ") {
                final_type = format!("const {}", final_type);
            }
        }

        if d.init.is_some() {
            Some(format!("{}{}{} {} = {};", self.indent(), prefix, final_type, name, init_code))
        } else {
            Some(format!("{}{}{} {}{{}};", self.indent(), prefix, final_type, name))
        }
    }

    /// Resolve the declared base type: annotations in Strict mode, init
    /// inference otherwise. Shared by every declaration path.
    fn declaration_base_type(&mut self, d: &VariableDeclarator<'a>) -> String {
        match self.type_mode {
            TypeMode::Strict => {
                // In strict mode: use annotation if present, otherwise infer from init
                if let Some(ta) = &d.type_annotation {
                    resolve_type_annotation(
                        Some(ta),
                        "auto",
                        &self.ctx.template_params,
                        false,
                        &self.ctx.class_names,
                    )
                } else if let Some(init) = &d.init {
                    self.infer_type_from_init(init).unwrap_or_else(|| "auto".to_string())
                } else {
                    "auto".to_string()
                }
            }
            TypeMode::Infer => {
                // In infer mode: always infer from init, ignore user annotations
                if let Some(init) = &d.init {
                    self.infer_type_from_init(init).unwrap_or_else(|| "auto".to_string())
                } else {
                    "auto".to_string()
                }
            }
        }
    }

    /// Apply analyzer widening to a declared base type.
    ///
    /// A native number annotation on a proven-unknown future is a trusted
    /// range promise and beats `ToJsNumber`/`ToJsValue` widening. Inferred
    /// integers shrink to `int32_t` when the analyzer proved the full range.
    fn apply_declaration_widening(
        &self,
        d: &VariableDeclarator<'a>,
        name: &str,
        base_type: String,
        widened: WidenedType,
    ) -> String {
        if let Some(trusted) = Self::is_trusted_native_number(d, &widened) {
            return trusted;
        }
        let is_class_type = base_type.starts_with("std::shared_ptr<")
            || self.ctx.class_names.contains(base_type.as_str());
        let cpp_type = match widened {
            WidenedType::ToJsNumber => "JsNumber".to_string(),
            WidenedType::ToJsString => {
                if is_class_type {
                    base_type.clone()
                } else {
                    "JsString".to_string()
                }
            }
            WidenedType::ToJsValue => "JsValue".to_string(),
            WidenedType::ToJsLateValue => "JsLateValue".to_string(),
            WidenedType::ToJsArray => "JsArray".to_string(),
            WidenedType::None => {
                if base_type == "auto" {
                    if d.init.is_some() { base_type } else { "JsValue".to_string() }
                } else {
                    base_type.clone()
                }
            }
        };
        let inferred = matches!(self.type_mode, TypeMode::Infer) || d.type_annotation.is_none();
        let inferred_integer = matches!(cpp_type.as_str(), "int" | "int32_t" | "int64_t");
        if inferred && matches!(widened, WidenedType::None) && inferred_integer {
            return self.select_integer_type(name);
        }
        cpp_type
    }

    /// Check whether a declaration carries a trusted native-number annotation.
    ///
    /// Trust fires only on proven-unknown futures: dynamic widening, no
    /// initializer, or an `await` boundary (where the annotation is the best
    /// bound information available). Statically known values keep inferred
    /// types.
    fn is_trusted_native_number(d: &VariableDeclarator, widened: &WidenedType) -> Option<String> {
        let annotation = d.type_annotation.as_ref()?;
        let trusted = native_number_annotation_type(Some(annotation))?;
        let unknown_future = d.init.is_none()
            || matches!(widened, WidenedType::ToJsNumber | WidenedType::ToJsValue)
            || matches!(d.init, Some(Expression::AwaitExpression(_)));
        if unknown_future { Some(trusted) } else { None }
    }

    /// Convert a dynamic right-hand side into a native integer/float target.
    ///
    /// `int x = <JsNumber>` has no implicit conversion, so the emitter spells
    /// the narrowing: `.as_int()` / `.as_double()`. Anything else passes
    /// through untouched.
    fn convert_number_value(
        &self,
        cpp_type: &str,
        init: &Expression<'a>,
        init_code: String,
    ) -> String {
        let init_class = self.operand_class_of(init);
        if !matches!(init_class, OperandClass::JsNumber | OperandClass::JsValue) {
            return init_code;
        }
        let int_target = cpp_type == "int"
            || cpp_type == "int32_t"
            || cpp_type == "int64_t"
            || cpp_type.starts_with("uint")
            || cpp_type == "size_t";
        let float_target = cpp_type == "float" || cpp_type == "double";
        if !int_target && !float_target {
            return init_code;
        }
        let primitive = if init_class == OperandClass::JsNumber {
            init_code
        } else {
            format!("std::get<JsNumber>(JsValue({}).inner)", init_code)
        };
        if int_target {
            format!("({}).as_int()", primitive)
        } else {
            format!("({}).as_double()", primitive)
        }
    }

    /// Pick `int32_t` when the analyzer proved every assigned value fits,
    /// otherwise `int64_t`. Dynamic futures never reach here (they widen).
    fn select_integer_type(&self, name: &str) -> String {
        let proven_small = self
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.var_infos.get(name))
            .map(|info| {
                info.int_range_exact
                    && info.int_range.is_some_and(|(low, high)| {
                        low >= i32::MIN as i64 && high <= i32::MAX as i64
                    })
            })
            .unwrap_or(false);
        if proven_small { "int32_t".to_string() } else { "int64_t".to_string() }
    }

    /// Element type of `std::vector<T>`, including nested template
    /// arguments (`std::vector<std::vector<JsObject>>` → `std::vector<JsObject>`).
    fn vector_element_type(ty: &str) -> Option<String> {
        let inner = ty.strip_prefix("std::vector<")?.strip_suffix('>')?;
        if inner.is_empty() { None } else { Some(inner.to_string()) }
    }

    fn infer_type_from_init(&self, node: &Expression<'a>) -> Option<String> {
        match node {
            Expression::ParenthesizedExpression(p) => self.infer_type_from_init(&p.expression),
            Expression::TSAsExpression(e) => self.infer_type_from_init(&e.expression),
            Expression::TSSatisfiesExpression(e) => self.infer_type_from_init(&e.expression),
            Expression::TSTypeAssertion(e) => self.infer_type_from_init(&e.expression),
            Expression::TSNonNullExpression(e) => self.infer_type_from_init(&e.expression),
            // `await response.json()` returns the parsed body synchronously
            // in the native runtime; infer from the body reader itself.
            Expression::AwaitExpression(a) if Self::is_sync_response_body_call(&a.argument) => {
                self.infer_type_from_init(&a.argument)
            }
            Expression::StringLiteral(_) => Some("std::string".to_string()),
            Expression::TemplateLiteral(_) => Some("std::string".to_string()),
            Expression::BooleanLiteral(_) => Some("bool".to_string()),
            Expression::NumericLiteral(n) => {
                if n.value.fract() == 0.0 && n.value.abs() <= i64::MAX as f64 {
                    Some("int64_t".to_string())
                } else {
                    Some("double".to_string())
                }
            }
            Expression::NullLiteral(_) => Some("JsNull".to_string()),
            Expression::NewTarget(_) => Some("JsValue".to_string()),
            Expression::Identifier(id) => {
                if id.name.as_str() == "undefined" {
                    Some("JsUndefined".to_string())
                } else {
                    self.ctx.var_types.get(id.name.as_str()).cloned()
                }
            }
            Expression::ArrayExpression(arr) => {
                if arr.elements.is_empty() {
                    Some("std::vector<JsValue>".to_string())
                } else {
                    // Arrays with spread elements need runtime handling -> JsArray
                    let has_spread = arr
                        .elements
                        .iter()
                        .any(|e| matches!(e, ArrayExpressionElement::SpreadElement(_)));
                    if has_spread {
                        return Some("JsArray".to_string());
                    }
                    let mut elem_types = Vec::new();
                    for el in &arr.elements {
                        if let Some(expr) = el.as_expression() {
                            if let Some(t) = self.infer_type_from_init(expr) {
                                elem_types.push(t);
                            } else {
                                elem_types.push("JsValue".to_string());
                            }
                        }
                    }
                    if elem_types.is_empty() {
                        Some("std::vector<JsValue>".to_string())
                    } else if elem_types.iter().all(|t| t == &elem_types[0]) {
                        Some(format!("std::vector<{}>", elem_types[0]))
                    } else {
                        Some("std::vector<JsValue>".to_string())
                    }
                }
            }
            Expression::ObjectExpression(_) => Some("JsObject".to_string()),
            Expression::ComputedMemberExpression(m) => {
                let obj_type = self.infer_type_from_init(&m.object)?;
                if obj_type == "JsArray" || obj_type == "JsObject" {
                    return Some("JsValue".to_string());
                }
                // `std::vector<JsObject>[i]` yields the element type, so
                // chained member access can choose bracket notation.
                if let Some(elem) = Self::vector_element_type(&obj_type) {
                    return Some(elem);
                }
                None
            }
            Expression::StaticMemberExpression(m) => {
                if matches!(&m.object, Expression::ThisExpression(_))
                    && let Some(class) = &self.ctx.class_name
                    && let Some(ty) = self
                        .ctx
                        .class_field_types
                        .get(class)
                        .and_then(|fields| fields.get(m.property.name.as_str()))
                {
                    return Some(ty.clone());
                }
                let obj_type = self.infer_type_from_init(&m.object)?;
                if obj_type == "JsObject" || obj_type == "JsValue" || obj_type == "JsArray" {
                    return Some("JsValue".to_string());
                }
                None
            }
            Expression::PrivateFieldExpression(p) => {
                if matches!(&p.object, Expression::ThisExpression(_))
                    && let Some(class) = &self.ctx.class_name
                {
                    return self
                        .ctx
                        .class_field_types
                        .get(class)
                        .and_then(|fields| fields.get(&format!("#{}", p.field.name)))
                        .cloned();
                }
                None
            }
            Expression::NewExpression(n) => {
                if let Expression::Identifier(id) = &n.callee {
                    if id.name.as_str() == "Promise" {
                        let inner = n
                            .type_arguments
                            .as_ref()
                            .and_then(|ta| ta.params.first())
                            .map(|t| {
                                resolve_type(
                                    Some(t),
                                    "JsValue",
                                    &self.ctx.template_params,
                                    false,
                                    &self.ctx.class_names,
                                )
                            })
                            .unwrap_or_else(|| "JsValue".to_string());
                        if inner == "void" {
                            return Some("morph::Task".to_string());
                        }
                        return Some(format!("morph::Result<{}>", inner));
                    }
                    if self.ctx.class_names.contains(id.name.as_str()) {
                        return Some(format!(
                            "std::shared_ptr<{}>",
                            self.class_cpp_name(id.name.as_str())
                        ));
                    }
                }
                Some("JsValue".to_string())
            }
            Expression::CallExpression(c) => {
                if let Expression::StaticMemberExpression(member) = &c.callee {
                    // `JSON.parse(...)` / `JSON.stringify(...)` produce a
                    // `JsValue`; dot access on the result lowers to a
                    // bracket lookup (Part 3A).
                    if matches!(&member.object, Expression::Identifier(id) if id.name.as_str() == "JSON")
                        && matches!(member.property.name.as_str(), "parse" | "stringify")
                    {
                        return Some("JsValue".to_string());
                    }
                    if self.is_response_like_expr(&member.object) {
                        match member.property.name.as_str() {
                            "json" => return Some("JsValue".to_string()),
                            "text" | "arrayBuffer" => return Some("std::string".to_string()),
                            _ => {}
                        }
                    }
                }
                let Expression::Identifier(id) = &c.callee else {
                    return None;
                };
                // Timer handles are ints in the runtime (`task.h`): naming
                // the type (instead of `auto`) lets escaping declarations
                // wrap in `shared_ptr` so cleanup closures capture the
                // handle safely (docs/dev/bugs#4).
                if matches!(id.name.as_str(), "setTimeout" | "setInterval") {
                    return Some("int".to_string());
                }
                // Ambient function return types let `make().name` choose
                // bracket access when `make` returns a JsObject.
                self.ctx
                    .fn_return_types
                    .get(id.name.as_str())
                    .or_else(|| self.ctx.var_types.get(id.name.as_str()))
                    .cloned()
            }
            _ => None,
        }
    }

    fn binding_to_identifier(&self, pat: &BindingPattern<'a>) -> (String, Option<String>) {
        match pat {
            BindingPattern::BindingIdentifier(id) => (id.name.to_string(), None),
            BindingPattern::AssignmentPattern(a) => self.binding_to_identifier(&a.left),
            BindingPattern::ObjectPattern(_) => ("/* object destructuring */".to_string(), None),
            BindingPattern::ArrayPattern(_) => ("/* array destructuring */".to_string(), None),
        }
    }

    fn emit_block(&mut self, block: &BlockStatement<'a>) -> String {
        if block.body.is_empty() {
            return "{}".to_string();
        }
        let mut lines = vec!["{".to_string()];
        let inner_indent = self.ctx.indent_level + 1;
        let old = self.ctx.indent_level;
        self.ctx.indent_level = inner_indent;
        for stmt in &block.body {
            if let Some(code) = self.emit_statement(stmt) {
                lines.push(code);
            }
        }
        self.ctx.indent_level = old;
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_return(&mut self, r: &ReturnStatement<'a>) -> String {
        if self.ctx.drop_return_value {
            return format!("{}co_return;", self.indent());
        }
        let kw = if self.ctx.is_async_fn > 0 { "co_return" } else { "return" };
        if let Some(arg) = &r.argument {
            if let Expression::Identifier(id) = arg {
                let already_moved = self
                    .ctx
                    .var_types
                    .get(id.name.as_str())
                    .map(|var_type| var_type.starts_with("std::unique_ptr<"))
                    .unwrap_or(false);
                // A returned `unique_ptr` moves implicitly (and stays
                // eligible for NRVO); an explicit move would only pessimize.
                if !already_moved && self.move_eligible_at(id.name.as_str(), id.span.start) {
                    return format!("{}{} std::move({});", self.indent(), kw, id.name);
                }
            }
            format!("{}{} {};", self.indent(), kw, self.emit_expression(arg))
        } else {
            format!("{}{};", self.indent(), kw)
        }
    }

    fn emit_if(&mut self, i: &IfStatement<'a>) -> String {
        let cond = self.emit_truthy_test(&i.test);
        let cons_code = self.emit_statement(&i.consequent).unwrap_or_else(|| "{}".to_string());
        let mut result = if matches!(&i.consequent, Statement::BlockStatement(_)) {
            format!("{}if ({}) {}", self.indent(), cond, cons_code)
        } else {
            format!("{}if ({}) {{\n{}\n{}}}", self.indent(), cond, cons_code, self.indent())
        };
        if let Some(alt) = &i.alternate {
            let alt_code = self.emit_statement(alt).unwrap_or_default();
            if matches!(alt, Statement::IfStatement(_) | Statement::BlockStatement(_)) {
                result.push_str(&format!(" else {}", alt_code.trim_start()));
            } else {
                result.push_str(&format!(" else {{\n{}\n{}}}", alt_code, self.indent()));
            }
        }
        result
    }

    /// `continue` in a captured `for` must first create the next iteration
    /// binding and run the update, because those steps normally live at the
    /// end of the loop body and `continue` skips them.
    fn emit_continue(&self) -> String {
        let action = self.loop_continue_actions.last().cloned().flatten();
        let Some(action) = action else {
            return format!("{}continue;", self.indent());
        };
        let base = self.indent();
        let inner = format!("{base}{INDENT}");
        let mut lines = vec![format!("{base}{{")];
        for (name, ty) in &action.copy_vars {
            lines.push(format!("{inner}{name} = std::make_shared<{ty}>(*{name});"));
        }
        if !action.update.trim().is_empty() {
            lines.push(format!("{inner}{};", action.update.trim().trim_end_matches(';')));
        }
        lines.push(format!("{inner}continue;"));
        lines.push(format!("{base}}}"));
        lines.join("\n")
    }

    fn emit_while(&mut self, w: &WhileStatement<'a>) -> String {
        let cond = self.emit_truthy_test(&w.test);
        self.loop_continue_actions.push(None);
        let body_code = self.emit_statement(&w.body).unwrap_or_else(|| "{}".to_string());
        self.loop_continue_actions.pop();
        let is_infinite = cond.trim().trim_matches(|c| c == '(' || c == ')').trim() == "true"
            || cond.trim() == "1";
        // For exact Python match, wrap true in extra parens to get ((true))
        let cond_emit =
            if is_infinite && cond.trim() == "true" { format!("({})", cond) } else { cond.clone() };
        if is_infinite && self.ctx.fn_body_depth > 0 {
            self.ctx.has_infinite_loop = true;
            self.ctx.needed.insert("\"../../runtime/cpp/reactivity/task.h\"".to_string());
            let bi = INDENT.repeat(self.ctx.indent_level + 1);
            if matches!(&w.body, Statement::BlockStatement(_)) {
                let mut stripped = body_code.trim_end().to_string();
                if stripped.ends_with('}') {
                    stripped.truncate(stripped.len() - 1);
                    // For exact Python match, make break; have no indent (replicate bug)
                    let stripped_fixed = stripped.replace(&format!("{}break;", bi), "break;");
                    let new_body = format!(
                        "{}\n{}co_await morph::next_frame();\n{}}}",
                        stripped_fixed.trim_end(),
                        bi,
                        self.indent()
                    );
                    return format!("{}while ({}) {}", self.indent(), cond_emit, new_body);
                }
                return format!("{}while ({}) {}", self.indent(), cond_emit, body_code);
            } else {
                return format!(
                    "{}while ({}) {{\n{}\n{}co_await morph::next_frame();\n{}}}",
                    self.indent(),
                    cond_emit,
                    body_code,
                    bi,
                    self.indent()
                );
            }
        }
        if matches!(&w.body, Statement::BlockStatement(_)) {
            format!("{}while ({}) {}", self.indent(), cond_emit, body_code)
        } else {
            format!("{}while ({}) {{\n{}\n{}}}", self.indent(), cond_emit, body_code, self.indent())
        }
    }

    fn emit_do_while(&mut self, d: &DoWhileStatement<'a>) -> String {
        self.loop_continue_actions.push(None);
        let body = self.emit_statement(&d.body).unwrap_or_else(|| "{}".to_string());
        self.loop_continue_actions.pop();
        let cond = self.emit_truthy_test(&d.test);
        format!("{}do {} while ({});", self.indent(), body, cond)
    }

    fn emit_for(&mut self, f: &ForStatement<'a>) -> String {
        if let Some(code) = self.emit_for_captured_bindings(f) {
            return code;
        }
        self.loop_continue_actions.push(None);
        let out = self.emit_for_plain(f);
        self.loop_continue_actions.pop();
        out
    }

    fn emit_for_plain(&mut self, f: &ForStatement<'a>) -> String {
        let init = f.init.as_ref().and_then(|init| self.emit_for_init(init)).unwrap_or_default();
        let cond = f.test.as_ref().map(|e| self.emit_truthy_test(e)).unwrap_or_default();
        let update = f.update.as_ref().map(|e| self.emit_expression(e)).unwrap_or_default();
        let body = self.emit_statement(&f.body).unwrap_or_else(|| "{}".to_string());
        let init_clean = init.trim().trim_end_matches(';').to_string();
        let is_infinite =
            cond.trim().is_empty() && init_clean.is_empty() && update.trim().is_empty();
        if is_infinite && self.ctx.fn_body_depth > 0 {
            self.ctx.has_infinite_loop = true;
            self.ctx.needed.insert("\"../../runtime/cpp/reactivity/task.h\"".to_string());
            let bi = INDENT.repeat(self.ctx.indent_level + 1);
            if matches!(&f.body, Statement::BlockStatement(_)) {
                let mut stripped = body.trim_end().to_string();
                if stripped.ends_with('}') {
                    stripped.truncate(stripped.len() - 1);
                    let new_body = format!(
                        "{}\n{}co_await morph::next_frame();\n{}}}",
                        stripped.trim_end(),
                        bi,
                        self.indent()
                    );
                    return format!(
                        "{}for ({}; {}; {}) {}",
                        self.indent(),
                        init_clean,
                        cond,
                        update,
                        new_body
                    );
                }
            } else {
                return format!(
                    "{}for ({}; {}; {}) {{\n{}\n{}co_await morph::next_frame();\n{}}}",
                    self.indent(),
                    init_clean,
                    cond,
                    update,
                    body,
                    bi,
                    self.indent()
                );
            }
        }
        if matches!(&f.body, Statement::BlockStatement(_)) {
            format!("{}for ({}; {}; {}) {}", self.indent(), init_clean, cond, update, body)
        } else {
            format!(
                "{}for ({}; {}; {}) {{\n{}\n{}}}",
                self.indent(),
                init_clean,
                cond,
                update,
                body,
                self.indent()
            )
        }
    }

    /// C++ type for one `for`-init declarator, before escape wrapping.
    fn for_init_declarator_type(&mut self, decl: &VariableDeclarator<'a>) -> String {
        match self.type_mode {
            TypeMode::Strict => {
                if let Some(ta) = &decl.type_annotation {
                    resolve_type_annotation(
                        Some(ta),
                        "auto",
                        &self.ctx.template_params,
                        false,
                        &self.ctx.class_names,
                    )
                } else if let Some(init) = &decl.init {
                    self.infer_type_from_init(init).unwrap_or_else(|| "auto".to_string())
                } else {
                    "auto".to_string()
                }
            }
            TypeMode::Infer => {
                if let Some(init) = &decl.init {
                    self.infer_type_from_init(init).unwrap_or_else(|| "auto".to_string())
                } else {
                    "auto".to_string()
                }
            }
        }
    }

    /// Lower a classic `for (let/const ...)` loop whose binding is captured
    /// by a closure. Each iteration gets a fresh `shared_ptr` box: closures
    /// capture the box by value, the body mutates the current box, and a
    /// copy is made before the update runs so the next iteration owns a new
    /// binding. `continue` performs that same copy + update through
    /// [`Self::emit_continue`].
    fn emit_for_captured_bindings(&mut self, f: &ForStatement<'a>) -> Option<String> {
        let ForStatementInit::VariableDeclaration(declaration) = f.init.as_ref()? else {
            return None;
        };
        if !matches!(
            declaration.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        ) {
            return None;
        }
        let mut captured_any = false;
        for decl in &declaration.declarations {
            let BindingPattern::BindingIdentifier(id) = &decl.id else {
                return None;
            };
            captured_any |= self.is_captured_name(id.name.as_str());
        }
        if !captured_any {
            return None;
        }

        // Only scalar bindings get an extra box. Class/container bindings
        // already lower to `shared_ptr` carriers (or value copies that share
        // their runtime storage), and wrapping those again would change
        // member-access and method-call lowering.
        let mut planned = Vec::new();
        for decl in &declaration.declarations {
            let BindingPattern::BindingIdentifier(id) = &decl.id else {
                return None;
            };
            let mut cpp_type = self.for_init_declarator_type(decl);
            if cpp_type == "auto" {
                cpp_type = "JsValue".to_string();
            }
            if !Self::is_scalar_pointee(&cpp_type) {
                return None;
            }
            planned.push((id, cpp_type));
        }

        self.ctx.need("std::make_shared");
        self.ctx.need("JsUndefined");
        let mut declarations = Vec::new();
        let mut copy_vars = Vec::new();
        let mut saved = Vec::new();
        for (decl, (id, cpp_type)) in declaration.declarations.iter().zip(planned) {
            let name = id.name.to_string();
            self.ctx.need(&cpp_type);
            let boxed_type = format!("std::shared_ptr<{cpp_type}>");
            self.ctx.need(&boxed_type);
            let init_code = if let Some(init) = &decl.init {
                if cpp_type == "char" && matches!(init, Expression::StringLiteral(_)) {
                    self.emit_char_literal(init)
                } else {
                    self.emit_expression(init)
                }
            } else {
                "JsUndefined{}".to_string()
            };
            saved.push((
                name.clone(),
                self.ctx.var_types.get(&name).cloned(),
                self.ctx.shared_ptr_vars.contains(&name),
            ));
            self.ctx.var_types.insert(name.clone(), boxed_type.clone());
            self.ctx.shared_ptr_vars.insert(name.clone());
            declarations
                .push(format!("{boxed_type} {name} = std::make_shared<{cpp_type}>({init_code});"));
            copy_vars.push((name, cpp_type));
        }

        let cond = f.test.as_ref().map(|e| self.emit_truthy_test(e)).unwrap_or_default();
        let update = f.update.as_ref().map(|e| self.emit_expression(e)).unwrap_or_default();

        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level += 1;
        let for_indent = self.indent();
        let mut lines = vec![format!("{}{{", self.indent())];
        for declaration in &declarations {
            lines.push(format!("{for_indent}{declaration}"));
        }
        self.ctx.indent_level += 1;
        let body_indent = self.indent();
        self.loop_continue_actions.push(Some(LoopContinueAction {
            copy_vars: copy_vars.clone(),
            update: update.clone(),
        }));
        let body_lines = self.emit_loop_body_lines(&f.body);
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent + 1;
        lines.push(format!("{for_indent}for (; {}; ) {{", cond));
        lines.extend(body_lines);
        for (name, ty) in &copy_vars {
            lines.push(format!("{body_indent}{name} = std::make_shared<{ty}>(*{name});"));
        }
        if !update.trim().is_empty() {
            lines.push(format!("{body_indent}{};", update.trim().trim_end_matches(';')));
        }
        lines.push(format!("{for_indent}}}"));
        self.ctx.indent_level = old_indent;
        lines.push(format!("{}}}", self.indent()));

        for (name, previous, was_shared) in saved {
            match previous {
                Some(ty) => {
                    self.ctx.var_types.insert(name.clone(), ty);
                }
                None => {
                    self.ctx.var_types.remove(&name);
                }
            }
            if was_shared {
                self.ctx.shared_ptr_vars.insert(name);
            } else {
                self.ctx.shared_ptr_vars.remove(&name);
            }
        }
        Some(lines.join("\n"))
    }

    fn emit_for_init(&mut self, init: &ForStatementInit<'a>) -> Option<String> {
        match init {
            ForStatementInit::VariableDeclaration(d) => {
                let kind = d.kind.as_str().to_string();
                // Hoisted `var` init: the binding already exists, so the
                // for-init contributes assignments only.
                let hoisted_var = kind == "var"
                    && d.declarations.iter().any(|decl| {
                        let (name, _) = self.binding_to_identifier(&decl.id);
                        self.hoisted_binding(&name).is_some()
                    });
                if hoisted_var {
                    let mut parts = Vec::new();
                    for decl in &d.declarations {
                        if !matches!(&decl.id, BindingPattern::BindingIdentifier(_)) {
                            continue;
                        }
                        let (name, _) = self.binding_to_identifier(&decl.id);
                        if name.starts_with("/*") {
                            continue;
                        }
                        let Some(_) = decl.init.as_ref() else {
                            continue;
                        };
                        let late = self
                            .hoist_stack
                            .last()
                            .and_then(|scope| scope.get(&name))
                            .is_some_and(|binding| binding.late);
                        let init_code = self.hoisted_init_code(decl, &name, late)?;
                        parts.push(format!("{} = {}", name, init_code));
                    }
                    return if parts.is_empty() { None } else { Some(parts.join(", ")) };
                }
                // For init, don't add `static` and emit as `type name = init` without trailing `;` and without indent
                let mut parts = Vec::new();
                for decl in &d.declarations {
                    let (name, _) = self.binding_to_identifier(&decl.id);
                    if name.starts_with("/*") {
                        continue;
                    }
                    let cpp_type = self.for_init_declarator_type(decl);
                    self.ctx.need(&cpp_type);
                    self.ctx.var_types.insert(name.to_string(), cpp_type.clone());
                    let final_type = if kind == "const"
                        && matches!(
                            cpp_type.as_str(),
                            "JsNumber" | "JsBoolean" | "JsString" | "JsUndefined" | "JsNull"
                        ) {
                        format!("const {}", cpp_type)
                    } else {
                        cpp_type
                    };
                    if let Some(init) = &decl.init {
                        let init_code = if final_type.starts_with("std::vector<")
                            && matches!(init, Expression::ArrayExpression(_))
                        {
                            self.emit_vector_literal_typed(init, Some(&final_type))
                        } else if final_type == "char"
                            && matches!(init, Expression::StringLiteral(_))
                        {
                            self.emit_char_literal(init)
                        } else {
                            self.emit_expression(init)
                        };
                        if matches!(init, Expression::NewExpression(_)) {
                            self.ctx.shared_ptr_vars.insert(name.to_string());
                        }
                        parts.push(format!("{} {} = {}", final_type, name, init_code));
                    } else {
                        parts.push(format!("{} {}", final_type, name));
                    }
                }
                if parts.is_empty() { None } else { Some(parts.join(", ")) }
            }
            _ => Some(self.span_text(init.span()).to_string()),
        }
    }

    fn for_of_element_type(&self, right: &Expression<'a>) -> String {
        if self.iterates_js_sequence(right) {
            return "JsValue".to_string();
        }
        if let Some(ty) = self.infer_type_from_init(right) {
            if let Some(inner) = Self::vector_inner_type(&ty) {
                return inner.trim().to_string();
            }
            if matches!(Self::bare_type(&ty), "JsArray" | "JsObject" | "JsValue" | "JsLateValue") {
                return "JsValue".to_string();
            }
        }
        "JsValue".to_string()
    }

    fn collect_binding_names(pattern: &BindingPattern<'_>, out: &mut Vec<String>) {
        match pattern {
            BindingPattern::BindingIdentifier(id) => out.push(id.name.to_string()),
            BindingPattern::AssignmentPattern(assign) => {
                Self::collect_binding_names(&assign.left, out);
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    Self::collect_binding_names(&property.value, out);
                }
                if let Some(rest) = &object.rest {
                    Self::collect_binding_names(&rest.argument, out);
                }
            }
            BindingPattern::ArrayPattern(array) => {
                for element in array.elements.iter().flatten() {
                    Self::collect_binding_names(element, out);
                }
                if let Some(rest) = &array.rest {
                    Self::collect_binding_names(&rest.argument, out);
                }
            }
        }
    }

    fn emit_loop_pattern(
        &mut self,
        pattern: &BindingPattern<'a>,
        element_type: &str,
        right_code: &str,
        body: &Statement<'a>,
        is_await: bool,
    ) -> String {
        let temp = format!("__loop_item_{}", self.destructure_count);
        self.destructure_count += 1;
        self.ctx.need(element_type);
        self.ctx.var_types.insert(temp.clone(), element_type.to_string());

        let mut names = Vec::new();
        Self::collect_binding_names(pattern, &mut names);
        let saved: Vec<(String, Option<String>)> = names
            .iter()
            .map(|name| (name.clone(), self.ctx.var_types.get(name).cloned()))
            .collect();

        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level += 1;
        let mut binding_lines = Vec::new();
        let old_box = self.box_loop_bindings;
        self.box_loop_bindings = true;
        let bindings_ok =
            self.emit_pattern_bindings(pattern, &temp, element_type, &mut binding_lines);
        self.box_loop_bindings = old_box;
        self.loop_continue_actions.push(None);
        let body_lines = self.emit_loop_body_lines(body);
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent;

        self.ctx.var_types.remove(&temp);
        for (name, previous) in saved {
            match previous {
                Some(ty) => {
                    self.ctx.var_types.insert(name, ty);
                }
                None => {
                    self.ctx.var_types.remove(&name);
                }
            }
        }

        if !bindings_ok {
            return format!("{}/* destructuring not supported */", self.indent());
        }
        let await_prefix = if is_await { "co_await " } else { "" };
        let mut lines = vec![format!(
            "{}for (auto {} : {}{}) {{",
            self.indent(),
            temp,
            await_prefix,
            right_code
        )];
        lines.extend(binding_lines);
        lines.extend(body_lines);
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_loop_assignment_pattern(
        &mut self,
        target: &AssignmentTarget<'a>,
        element_type: &str,
        right_code: &str,
        body: &Statement<'a>,
        is_await: bool,
    ) -> String {
        let temp = format!("__loop_item_{}", self.destructure_count);
        self.destructure_count += 1;
        self.ctx.need(element_type);
        self.ctx.var_types.insert(temp.clone(), element_type.to_string());

        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level += 1;
        let mut assignments = Vec::new();
        let assignments_ok =
            self.emit_assignment_pattern(target, &temp, element_type, &mut assignments);
        self.loop_continue_actions.push(None);
        let body_lines = self.emit_loop_body_lines(body);
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent;
        self.ctx.var_types.remove(&temp);

        if !assignments_ok {
            return format!("{}/* destructuring assignment not supported */", self.indent());
        }
        let await_prefix = if is_await { "co_await " } else { "" };
        let mut lines = vec![format!(
            "{}for (auto {} : {}{}) {{",
            self.indent(),
            temp,
            await_prefix,
            right_code
        )];
        lines.extend(assignments);
        lines.extend(body_lines);
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_loop_body_lines(&mut self, body: &Statement<'a>) -> Vec<String> {
        let mut lines = Vec::new();
        match body {
            Statement::BlockStatement(block) => {
                for stmt in &block.body {
                    if let Some(code) = self.emit_statement(stmt) {
                        lines.push(code);
                    }
                }
            }
            _ => {
                if let Some(code) = self.emit_statement(body) {
                    lines.push(code);
                }
            }
        }
        lines
    }

    /// Classify a `for...in` receiver that enumerates index keys rather than
    /// values. Both arrays/vectors and strings have `.size()`; `None` keeps
    /// the object-key path.
    fn for_in_sequence(&self, expr: &Expression<'a>) -> Option<ForInSequence> {
        let expr = Self::unwrap_expr(expr);
        match expr {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => {
                Some(ForInSequence::Text)
            }
            _ => {
                let ty = self.infer_type_from_init(expr)?;
                let bare = Self::bare_type(&ty);
                if bare == "JsArray" || bare.starts_with("std::vector<") {
                    Some(ForInSequence::Array)
                } else if bare == "std::string" || bare == "JsString" {
                    Some(ForInSequence::Text)
                } else {
                    None
                }
            }
        }
    }

    fn emit_for_in_sequence_captured(
        &mut self,
        f: &ForInStatement<'a>,
        name: &str,
        sequence: ForInSequence,
    ) -> Option<String> {
        let right = self.emit_expression(&f.right);
        let serial = self.destructure_count;
        self.destructure_count += 1;
        let seq_temp = format!("__morph_for_in_seq_{serial}");
        let index_temp = format!("__morph_for_in_index_{serial}");
        let ty = "std::string";
        let boxed_type = format!("std::shared_ptr<{ty}>");
        self.ctx.need(ty);
        self.ctx.need(&boxed_type);
        self.ctx.need("std::make_shared");
        let previous = self.ctx.var_types.get(name).cloned();
        let was_shared = self.ctx.shared_ptr_vars.contains(name);
        self.ctx.var_types.insert(name.to_string(), boxed_type.clone());
        self.ctx.shared_ptr_vars.insert(name.to_string());

        let old_indent = self.ctx.indent_level;
        let mut lines = vec![match sequence {
            ForInSequence::Array => {
                format!("{}const auto& {} = {};", self.indent(), seq_temp, right)
            }
            ForInSequence::Text => {
                format!("{}std::string {} = {};", self.indent(), seq_temp, right)
            }
        }];
        self.ctx.indent_level += 1;
        lines.push(format!(
            "{}for (std::size_t {} = 0; {} < {}.size(); ++{}) {{",
            self.indent(),
            index_temp,
            index_temp,
            seq_temp,
            index_temp
        ));
        lines.push(format!(
            "{}std::shared_ptr<{}> {} = std::make_shared<{}>(std::to_string({}));",
            self.indent(),
            ty,
            name,
            ty,
            index_temp
        ));
        self.ctx.indent_level += 1;
        self.loop_continue_actions.push(None);
        lines.extend(self.emit_loop_body_lines(&f.body));
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent;
        lines.push(format!("{}}}", self.indent()));

        match previous {
            Some(previous) => {
                self.ctx.var_types.insert(name.to_string(), previous);
            }
            None => {
                self.ctx.var_types.remove(name);
            }
        }
        if was_shared {
            self.ctx.shared_ptr_vars.insert(name.to_string());
        } else {
            self.ctx.shared_ptr_vars.remove(name);
        }
        Some(lines.join("\n"))
    }

    fn emit_for_in_sequence(
        &mut self,
        f: &ForInStatement<'a>,
        sequence: ForInSequence,
    ) -> Option<String> {
        let declaration_name = match &f.left {
            ForStatementLeft::VariableDeclaration(declaration) => {
                let first = declaration.declarations.first()?;
                let BindingPattern::BindingIdentifier(id) = &first.id else {
                    return None;
                };
                Some(id.name.to_string())
            }
            _ => None,
        };
        let right = self.emit_expression(&f.right);
        let serial = self.destructure_count;
        self.destructure_count += 1;
        let seq_temp = format!("__morph_for_in_seq_{serial}");
        let index_temp = format!("__morph_for_in_index_{serial}");
        self.ctx.need("std::string");

        let hoisted = declaration_name.as_ref().is_some_and(|name| {
            self.hoist_stack.last().is_some_and(|scope| scope.get(name).is_some())
        });
        let previous = if hoisted {
            None
        } else {
            declaration_name.as_ref().and_then(|name| self.ctx.var_types.get(name).cloned())
        };
        if !hoisted {
            if let Some(name) = &declaration_name {
                self.ctx.var_types.insert(name.clone(), "std::string".to_string());
            }
        }

        let old_indent = self.ctx.indent_level;
        let mut lines = vec![match sequence {
            ForInSequence::Array => {
                format!("{}const auto& {} = {};", self.indent(), seq_temp, right)
            }
            ForInSequence::Text => {
                format!("{}std::string {} = {};", self.indent(), seq_temp, right)
            }
        }];
        self.ctx.indent_level += 1;
        lines.push(format!(
            "{}for (std::size_t {} = 0; {} < {}.size(); ++{}) {{",
            self.indent(),
            index_temp,
            index_temp,
            seq_temp,
            index_temp
        ));
        if let Some(name) = &declaration_name {
            if hoisted {
                lines.push(format!("{}{} = std::to_string({});", self.indent(), name, index_temp));
            } else {
                lines.push(format!(
                    "{}std::string {} = std::to_string({});",
                    self.indent(),
                    name,
                    index_temp
                ));
            }
        } else {
            let target = self.emit_for_left(&f.left);
            lines.push(format!("{}{} = std::to_string({});", self.indent(), target, index_temp));
        }
        self.ctx.indent_level += 1;
        self.loop_continue_actions.push(None);
        lines.extend(self.emit_loop_body_lines(&f.body));
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent;
        lines.push(format!("{}}}", self.indent()));

        if !hoisted {
            if let Some(name) = declaration_name {
                match previous {
                    Some(ty) => {
                        self.ctx.var_types.insert(name, ty);
                    }
                    None => {
                        self.ctx.var_types.remove(&name);
                    }
                }
            }
        }
        Some(lines.join("\n"))
    }

    fn emit_for_in_captured(&mut self, f: &ForInStatement<'a>) -> Option<String> {
        let ForStatementLeft::VariableDeclaration(declaration) = &f.left else {
            return None;
        };
        if !matches!(
            declaration.kind,
            VariableDeclarationKind::Let | VariableDeclarationKind::Const
        ) {
            return None;
        }
        let first = declaration.declarations.first()?;
        let BindingPattern::BindingIdentifier(id) = &first.id else {
            return None;
        };
        let name = id.name.to_string();
        if !self.is_captured_name(&name) {
            return None;
        }
        if let Some(sequence) = self.for_in_sequence(&f.right) {
            return self.emit_for_in_sequence_captured(f, &name, sequence);
        }
        let right = self.emit_expression(&f.right);
        let is_obj = if let Expression::Identifier(id) = &f.right {
            self.ctx
                .var_types
                .get(id.name.as_str())
                .map(|t| t == "JsObject" || t == "JsValue")
                .unwrap_or(false)
        } else {
            false
        };
        let iter = if is_obj { format!("{}.sorted_keys()", right) } else { right };
        let temp = format!("__loop_item_{}", self.destructure_count);
        self.destructure_count += 1;
        let ty = "std::string";
        let boxed_type = format!("std::shared_ptr<{ty}>");
        self.ctx.need(ty);
        self.ctx.need(&boxed_type);
        self.ctx.need("std::make_shared");
        let previous = self.ctx.var_types.get(&name).cloned();
        let was_shared = self.ctx.shared_ptr_vars.contains(&name);
        self.ctx.var_types.insert(name.clone(), boxed_type.clone());
        self.ctx.shared_ptr_vars.insert(name.clone());

        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level += 1;
        let mut lines = vec![format!("{}for (auto {} : {}) {{", self.indent(), temp, iter)];
        lines.push(format!(
            "{}std::shared_ptr<{}> {} = std::make_shared<{}>({});",
            self.indent(),
            ty,
            name,
            ty,
            temp
        ));
        self.ctx.indent_level += 1;
        self.loop_continue_actions.push(None);
        lines.extend(self.emit_loop_body_lines(&f.body));
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent;
        lines.push(format!("{}}}", self.indent()));

        match previous {
            Some(previous) => {
                self.ctx.var_types.insert(name.clone(), previous);
            }
            None => {
                self.ctx.var_types.remove(&name);
            }
        }
        if was_shared {
            self.ctx.shared_ptr_vars.insert(name);
        } else {
            self.ctx.shared_ptr_vars.remove(&name);
        }
        Some(lines.join("\n"))
    }

    fn emit_for_in(&mut self, f: &ForInStatement<'a>) -> String {
        if let Some(code) = self.emit_for_in_captured(f) {
            return code;
        }
        if let Some(sequence) = self.for_in_sequence(&f.right)
            && let Some(code) = self.emit_for_in_sequence(f, sequence)
        {
            return code;
        }
        let left = self.emit_for_left(&f.left);
        let right = self.emit_expression(&f.right);
        let is_obj = if let Expression::Identifier(id) = &f.right {
            self.ctx
                .var_types
                .get(id.name.as_str())
                .map(|t| t == "JsObject" || t == "JsValue")
                .unwrap_or(false)
        } else {
            false
        };
        let iter = if is_obj { format!("{}.sorted_keys()", right) } else { right };
        self.loop_continue_actions.push(None);
        let body = self.emit_statement(&f.body).unwrap_or_else(|| "{}".to_string());
        self.loop_continue_actions.pop();
        if self.for_left_binds_new_var(&f.left) {
            format!("{}for (auto {} : {}) {}", self.indent(), left, iter, body)
        } else {
            // A C++ range-for always declares its binding. Assign the
            // existing target (hoisted `var` or plain assignment target)
            // from a per-iteration temporary instead.
            let temp = format!("__morph_loop_value_{}", self.destructure_count);
            self.destructure_count += 1;
            format!(
                "{}for (auto {} : {}) {{ {} = {}; {} }}",
                self.indent(),
                temp,
                iter,
                left,
                temp,
                body
            )
        }
    }

    fn emit_for_of_captured_binding(
        &mut self,
        declarator: &VariableDeclarator<'a>,
        right: &Expression<'a>,
        body: &Statement<'a>,
        is_await: bool,
    ) -> Option<String> {
        let BindingPattern::BindingIdentifier(id) = &declarator.id else {
            return None;
        };
        let name = id.name.to_string();
        let element_type = self.for_of_element_type(right);
        // A plain `for (auto x : ...)` already creates a fresh C++ variable
        // per iteration. Only scalar elements need an extra box so mutations
        // inside one iteration stay visible to that iteration's closures;
        // class/container carriers keep their existing lowering.
        if !Self::is_scalar_pointee(&element_type) {
            return None;
        }
        let boxed_type = format!("std::shared_ptr<{element_type}>");
        self.ctx.need(&element_type);
        self.ctx.need(&boxed_type);
        self.ctx.need("std::make_shared");
        let temp = format!("__loop_item_{}", self.destructure_count);
        self.destructure_count += 1;
        let right_code = self.emit_expression(right);
        let previous = self.ctx.var_types.get(&name).cloned();
        let was_shared = self.ctx.shared_ptr_vars.contains(&name);
        self.ctx.var_types.insert(name.clone(), boxed_type.clone());
        self.ctx.shared_ptr_vars.insert(name.clone());

        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level += 1;
        let await_prefix = if is_await { "co_await " } else { "" };
        let mut lines = vec![format!(
            "{}for (auto {} : {}{}) {{",
            self.indent(),
            temp,
            await_prefix,
            right_code
        )];
        lines.push(format!(
            "{}std::shared_ptr<{}> {} = std::make_shared<{}>({});",
            self.indent(),
            element_type,
            name,
            element_type,
            temp
        ));
        self.ctx.indent_level += 1;
        self.loop_continue_actions.push(None);
        lines.extend(self.emit_loop_body_lines(body));
        self.loop_continue_actions.pop();
        self.ctx.indent_level = old_indent;
        lines.push(format!("{}}}", self.indent()));

        match previous {
            Some(previous) => {
                self.ctx.var_types.insert(name.clone(), previous);
            }
            None => {
                self.ctx.var_types.remove(&name);
            }
        }
        if was_shared {
            self.ctx.shared_ptr_vars.insert(name);
        } else {
            self.ctx.shared_ptr_vars.remove(&name);
        }
        Some(lines.join("\n"))
    }

    fn emit_for_of(&mut self, f: &ForOfStatement<'a>) -> String {
        if let ForStatementLeft::VariableDeclaration(declaration) = &f.left
            && let Some(first) = declaration.declarations.first()
            && !matches!(&first.id, BindingPattern::BindingIdentifier(_))
        {
            let element_type = self.for_of_element_type(&f.right);
            let right = self.emit_expression(&f.right);
            return self.emit_loop_pattern(&first.id, &element_type, &right, &f.body, f.r#await);
        }
        if let Some(target) = f.left.as_assignment_target()
            && matches!(
                target,
                AssignmentTarget::ArrayAssignmentTarget(_)
                    | AssignmentTarget::ObjectAssignmentTarget(_)
            )
        {
            let element_type = self.for_of_element_type(&f.right);
            let right = self.emit_expression(&f.right);
            return self.emit_loop_assignment_pattern(
                target,
                &element_type,
                &right,
                &f.body,
                f.r#await,
            );
        }
        if let ForStatementLeft::VariableDeclaration(declaration) = &f.left
            && matches!(
                declaration.kind,
                VariableDeclarationKind::Let | VariableDeclarationKind::Const
            )
            && let Some(first) = declaration.declarations.first()
            && let BindingPattern::BindingIdentifier(id) = &first.id
            && self.is_captured_name(id.name.as_str())
            && let Some(code) =
                self.emit_for_of_captured_binding(first, &f.right, &f.body, f.r#await)
        {
            return code;
        }
        let left = self.emit_for_left(&f.left);
        let right = self.emit_expression(&f.right);
        // `for (auto x : <js sequence>)` binds a JsValue element. Record the
        // loop variable so numeric contexts convert (`acc += x` needs a
        // ToNumber step); the binding is scoped to the loop body.
        let loop_var = match &f.left {
            ForStatementLeft::VariableDeclaration(d) => {
                d.declarations.first().map(|first| self.binding_to_identifier(&first.id).0)
            }
            _ => None,
        };
        let js_seq = self.iterates_js_sequence(&f.right);
        let prev_type = loop_var
            .as_ref()
            .filter(|_| js_seq)
            .map(|name| self.ctx.var_types.insert(name.clone(), "JsValue".to_string()));
        self.loop_continue_actions.push(None);
        let body = self.emit_statement(&f.body).unwrap_or_else(|| "{}".to_string());
        self.loop_continue_actions.pop();
        match (loop_var, prev_type) {
            (Some(name), Some(Some(prev))) => {
                self.ctx.var_types.insert(name, prev);
            }
            (Some(name), Some(None)) => {
                self.ctx.var_types.remove(&name);
            }
            _ => {}
        }
        if self.for_left_binds_new_var(&f.left) {
            if f.r#await {
                format!("{}for (auto {} : co_await {}) {}", self.indent(), left, right, body)
            } else {
                format!("{}for (auto {} : {}) {}", self.indent(), left, right, body)
            }
        } else {
            let temp = format!("__morph_loop_value_{}", self.destructure_count);
            self.destructure_count += 1;
            if f.r#await {
                format!(
                    "{}for (auto {} : co_await {}) {{ {} = {}; {} }}",
                    self.indent(),
                    temp,
                    right,
                    left,
                    temp,
                    body
                )
            } else {
                format!(
                    "{}for (auto {} : {}) {{ {} = {}; {} }}",
                    self.indent(),
                    temp,
                    right,
                    left,
                    temp,
                    body
                )
            }
        }
    }

    /// Response body readers are synchronous in the native runtime, so
    /// `await response.json()` must not emit `co_await` on a `JsValue`.
    fn is_sync_response_body_call(expr: &Expression<'a>) -> bool {
        let Expression::CallExpression(call) = expr else {
            return false;
        };
        let Some(MemberExpression::StaticMemberExpression(member)) =
            call.callee.as_member_expression()
        else {
            return false;
        };
        matches!(member.property.name.as_str(), "json" | "text" | "arrayBuffer" | "clone")
    }

    /// True when a `for (... of ...)` right-hand side is a js sequence whose
    /// elements are JsValue: array/object literals and `JsArray` / `JsObject`
    /// / `JsValue` bindings. Everything else keeps default element typing.
    fn iterates_js_sequence(&self, expr: &Expression<'a>) -> bool {
        match expr {
            Expression::ArrayExpression(_) | Expression::ObjectExpression(_) => true,
            Expression::Identifier(id) => self
                .ctx
                .var_types
                .get(id.name.as_str())
                .is_some_and(|t| t == "JsArray" || t == "JsObject" || t == "JsValue"),
            _ => false,
        }
    }

    /// `for (var x ...)` binds into the hoisted function scope, so the loop
    /// header must assign the existing binding instead of declaring a new
    /// one. Every other declaration form still declares its own C++ local.
    fn for_left_binds_new_var(&self, left: &ForStatementLeft<'a>) -> bool {
        match left {
            ForStatementLeft::VariableDeclaration(d) => {
                if d.kind != VariableDeclarationKind::Var {
                    return true;
                }
                let Some(first) = d.declarations.first() else {
                    return true;
                };
                let (name, _) = self.binding_to_identifier(&first.id);
                self.hoisted_binding(&name).is_none()
            }
            _ => false,
        }
    }

    fn emit_for_left(&mut self, left: &ForStatementLeft<'a>) -> String {
        match left {
            ForStatementLeft::VariableDeclaration(d) => {
                if let Some(first) = d.declarations.first() {
                    let (name, _) = self.binding_to_identifier(&first.id);
                    if d.kind == VariableDeclarationKind::Var
                        && self.hoisted_binding(&name).is_some()
                    {
                        return name;
                    }
                }
                if let Some(first) = d.declarations.first() {
                    let (name, _) = self.binding_to_identifier(&first.id);
                    name
                } else {
                    "auto".to_string()
                }
            }
            ForStatementLeft::AssignmentTargetIdentifier(id) => id.name.to_string(),
            ForStatementLeft::StaticMemberExpression(s) => {
                format!("{}.{}", self.emit_expression(&s.object), s.property.name)
            }
            ForStatementLeft::ComputedMemberExpression(c) => format!(
                "{}[{}]",
                self.emit_expression(&c.object),
                self.emit_expression(&c.expression)
            ),
            _ => "auto".to_string(),
        }
    }

    fn emit_switch(&mut self, sw: &SwitchStatement<'a>) -> String {
        // JavaScript `switch` uses strict equality, accepts arbitrary case
        // expressions, and allows fallthrough. C++ case labels are integral
        // constants, so dispatch in JS order first and switch on the
        // resulting tag: `break`/`continue`/fallthrough then keep their
        // native C++ meaning, including a `default` in the middle.
        let disc = self.emit_expression(&sw.discriminant);
        let bi = INDENT.repeat(self.ctx.indent_level + 1);
        self.ctx.need("JsValue");
        let mut lines = vec![format!("{}{{", self.indent())];
        lines.push(format!("{}JsValue __morph_switch_disc = {};", bi, disc));
        lines.push(format!("{}int __morph_switch_tag = -1;", bi));

        let mut default_tag = None;
        let mut emitted_test = false;
        for (tag, case) in sw.cases.iter().enumerate() {
            let Some(test) = &case.test else {
                default_tag = Some(tag);
                continue;
            };
            let test_code = self.emit_expression(test);
            let test_class = self.operand_class_of(test);
            self.record_comparison_use(
                ComparisonKind::StrictEqual,
                OperandClass::JsValue,
                test_class,
            );
            let keyword = if emitted_test { "else if" } else { "if" };
            lines.push(format!(
                "{}{} (morph::js_cmp::strict_eq(__morph_switch_disc, {})) {{ __morph_switch_tag = {}; }}",
                bi, keyword, test_code, tag
            ));
            emitted_test = true;
        }
        if let Some(tag) = default_tag {
            if emitted_test {
                lines.push(format!("{}else {{ __morph_switch_tag = {}; }}", bi, tag));
            } else {
                lines.push(format!("{}__morph_switch_tag = {};", bi, tag));
            }
        } else if emitted_test {
            lines.push(format!("{}else {{ __morph_switch_tag = -1; }}", bi));
        }

        lines.push(format!("{}switch (__morph_switch_tag) {{", bi));
        for (tag, case) in sw.cases.iter().enumerate() {
            if case.test.is_some() {
                lines.push(format!("{}case {}:", bi, tag));
            } else {
                lines.push(format!("{}default:", bi));
            }
            for stmt in &case.consequent {
                if let Some(code) = self.emit_statement(stmt) {
                    lines.push(format!("{}    {}", bi, code.trim_start()));
                }
            }
        }
        lines.push(format!("{}}}", bi));
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn emit_try(&mut self, t: &TryStatement<'a>) -> String {
        let bi = INDENT.repeat(self.ctx.indent_level + 1);
        let mut lines: Vec<String> = Vec::new();
        let has_finally = t.finalizer.is_some();
        let has_handler = t.handler.is_some();
        // For catch+finally, use flag to avoid double finally (once in catch, once after)
        let use_caught_flag = has_handler && has_finally;
        if use_caught_flag {
            lines.push(format!("{}{{ bool __morph_caught = false;", self.indent()));
        }
        // try body — use emit_block and strip trailing } like Python
        let raw_body = self.emit_block(&t.block);
        let mut body_stripped = raw_body.trim_end().to_string();
        if body_stripped.ends_with('}') {
            body_stripped.truncate(body_stripped.len() - 1);
            body_stripped = body_stripped.trim_end().to_string();
        }
        lines.push(format!("{}try {}", self.indent(), body_stripped));
        // Python has a blank line with indent after throw; before } catch
        lines.push(format!("{}", self.indent()));
        let mut has_catch = false;
        let mut finalizer_block_str: Option<String> = None;
        if let Some(handler) = &t.handler {
            has_catch = true;
            let param = handler
                .param
                .as_ref()
                .map(|p| {
                    let (name, _) = self.binding_to_identifier(&p.pattern);
                    name
                })
                .unwrap_or_else(|| "_e".to_string());
            self.ctx.var_types.insert(param.clone(), "JsValue".to_string());
            let h = self.emit_block(&handler.body);
            let h_lines: Vec<&str> = h.split('\n').collect();
            lines.push(format!("{}}} catch (JsValue& {}) {{", self.indent(), param));
            if use_caught_flag {
                lines.push(format!("{}__morph_caught = true;", bi));
            }
            for l in &h_lines[1..h_lines.len() - 1] {
                lines.push(l.to_string());
            }
            if let Some(finalizer) = &t.finalizer {
                let f = self.emit_block(finalizer);
                finalizer_block_str = Some(f.clone());
                let f_lines: Vec<&str> = f.split('\n').collect();
                for l in &f_lines[1..f_lines.len() - 1] {
                    lines.push(l.to_string());
                }
            }
            lines.push(format!("{}}}", self.indent()));
        }
        if let Some(finalizer) = &t.finalizer {
            let f_str = finalizer_block_str.clone().unwrap_or_else(|| self.emit_block(finalizer));
            let f_lines: Vec<&str> = f_str.split('\n').collect();
            if !has_catch {
                lines.push(format!("{}}} catch (...) {{", self.indent()));
                for l in &f_lines[1..f_lines.len() - 1] {
                    lines.push(l.to_string());
                }
                lines.push(format!("{}throw;", bi));
                lines.push(format!("{}}}", self.indent()));
                // Python duplicates finalizer inner again and then adds extra throw handling
                // Duplicate handling for finally guarantee
                for l in &f_lines[1..f_lines.len() - 1] {
                    lines.push(l.to_string());
                }
                lines.push(format!("{}}}", self.indent()));
                // Normal exit path duplicated
                for l in &f_lines[1..f_lines.len() - 1] {
                    let stripped = l.trim();
                    if !stripped.is_empty() {
                        lines.push(format!("{}{}", self.indent(), stripped));
                    }
                }
                return lines.join("\n");
            } else {
                // handler exists, need catch-all
                if let Some(last) = lines.last_mut() {
                    if last.trim() == "}" {
                        *last = format!("{}}} catch (...) {{", self.indent());
                    } else {
                        lines.push(format!("{}}} catch (...) {{", self.indent()));
                    }
                } else {
                    lines.push(format!("{}}} catch (...) {{", self.indent()));
                }
                for l in &f_lines[1..f_lines.len() - 1] {
                    lines.push(l.to_string());
                }
                lines.push(format!("{}throw;", bi));
                for l in &f_lines[1..f_lines.len() - 1] {
                    lines.push(l.to_string());
                }
                lines.push(format!("{}}}", self.indent()));
                // Trailing finally for normal path only (caught path already ran finally)
                if use_caught_flag {
                    lines.push(format!("{}if (!__morph_caught) {{", self.indent()));
                    for l in &f_lines[1..f_lines.len() - 1] {
                        let stripped = l.trim();
                        if !stripped.is_empty() {
                            lines.push(format!("{}{}", bi, stripped));
                        }
                    }
                    lines.push(format!("{}}}", self.indent()));
                    lines.push(format!("{}}}", self.indent()));
                    return lines.join("\n");
                }
                for l in &f_lines[1..f_lines.len() - 1] {
                    let stripped = l.trim();
                    if !stripped.is_empty() {
                        lines.push(format!("{}{}", self.indent(), stripped));
                    }
                }
                return lines.join("\n");
            }
        } else if !has_catch {
            lines.push(format!("{}}}", self.indent()));
        }
        if use_caught_flag {
            lines.push(format!("{}}}", self.indent()));
        }
        lines.join("\n")
    }

    fn emit_throw(&mut self, t: &ThrowStatement<'a>) -> String {
        self.ctx.need("JsValue");
        format!("{}throw JsValue({});", self.indent(), self.emit_expression(&t.argument))
    }

    /// `unique_ptr<C>` when a sync function annotated `-> C` hands back
    /// exactly one `new C()`-built local. Direct body statements only:
    /// branched or multi-value returns keep today's signature.
    fn unique_return_class(&self, f: &Function<'a>, ret: &str) -> Option<String> {
        if !self.ctx.class_names.contains(ret) {
            return None;
        }
        let body = f.body.as_ref()?;
        let returned = Self::direct_returned_identifiers(body)?;
        if returned.len() != 1 {
            return None;
        }
        let var_name = &returned[0];
        let escapes_return = self
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.escapes.get(var_name))
            .is_some_and(|kind| *kind == EscapeKind::Return);
        if !escapes_return {
            return None;
        }
        if self.new_built_class(body, var_name).as_deref() != Some(ret) {
            return None;
        }
        Some(format!("std::unique_ptr<{}>", ret))
    }

    /// Distinct identifiers returned by direct body statements. `None`
    /// when anything else is returned: the caller must not guess.
    fn direct_returned_identifiers(body: &FunctionBody<'a>) -> Option<Vec<String>> {
        let mut returned: Vec<String> = Vec::new();
        for stmt in &body.statements {
            if let Statement::ReturnStatement(r) = stmt {
                match &r.argument {
                    Some(Expression::Identifier(id)) => {
                        let name = id.name.to_string();
                        if !returned.contains(&name) {
                            returned.push(name);
                        }
                    }
                    Some(_) => {
                        return None;
                    }
                    None => {}
                }
            }
        }
        Some(returned)
    }

    /// The constructed class when `var_name` is declared as `new C()`
    /// in a direct body statement.
    fn new_built_class(&self, body: &FunctionBody<'a>, var_name: &str) -> Option<String> {
        for stmt in &body.statements {
            if let Statement::VariableDeclaration(d) = stmt {
                for decl in &d.declarations {
                    let (decl_name, _) = self.binding_to_identifier(&decl.id);
                    if decl_name != var_name {
                        continue;
                    }
                    if let Some(Expression::NewExpression(n)) = &decl.init {
                        if let Expression::Identifier(callee) = &n.callee {
                            return Some(callee.name.to_string());
                        }
                    }
                    return None;
                }
            }
        }
        None
    }

    fn emit_function_declaration(&mut self, f: &Function<'a>) -> Option<String> {
        let Some(id) = &f.id else {
            return None;
        };
        let name = id.name.to_string();
        if f.body.is_none() {
            return None;
        }
        // Generators not supported - reject with clear error
        if f.generator {
            return Some(format!("/* error: generators not supported (function {}) */", name));
        }
        // Functions own their `new.target`: only a `new` call on this
        // function would make it defined, and Morph lowers `new` for
        // classes only.
        let old_in_constructor = self.ctx.in_constructor;
        self.ctx.in_constructor = false;
        // Nested functions lower to capturing lambdas: C++ forbids
        // function definitions inside function bodies, while a lambda
        // keeps the definition local and captures outer locals by reference.
        // Arrows and function expressions count as nesting for this purpose,
        // but their own capture rules are untouched.
        let is_nested =
            self.ctx.fn_body_depth > 0 || self.ctx.arrow_depth > 0 || self.ctx.fn_expr_depth > 0;
        // ... rest of function
        let is_async = f.r#async;
        if is_async {
            self.ctx.async_fns.insert(name.clone());
        }
        let old_has_loop = self.ctx.has_infinite_loop;
        self.ctx.has_infinite_loop = false;
        let ret = if let Some(rt) = &f.return_type {
            resolve_type_annotation(
                Some(rt),
                "auto",
                &self.ctx.template_params,
                false,
                &self.ctx.class_names,
            )
        } else {
            if name == "main" {
                "int".to_string()
            } else if is_async {
                "JsValue".to_string()
            } else if self.has_return(f.body.as_ref().unwrap()) {
                "auto".to_string()
            } else {
                "void".to_string()
            }
        };
        // A top-level definition never repeats defaults: they live on the
        // forward declaration (standalone output) or `morph_api.h`
        // (project builds). Nested functions become lambdas with no
        // prototype, so they keep their defaults at the definition.
        let params = if is_nested {
            self.format_params(&f.params)
        } else {
            self.format_params_no_defaults(&f.params)
        };
        if name == "main" && is_async {
            let code = self.emit_async_main(f, &ret, &params);
            self.ctx.has_infinite_loop = old_has_loop || self.ctx.has_infinite_loop;
            self.ctx.in_constructor = old_in_constructor;
            return Some(code);
        }
        let mut final_ret = ret.clone();
        if is_async {
            final_ret = self.ctx.async_result_type(&ret);
        }
        if !is_async && final_ret == "auto" {
            if let Some(body_ref) = f.body.as_ref() {
                if let Some(ty) = self.literal_return_type(body_ref) {
                    final_ret = ty;
                }
            }
        }
        // Intent-based: sync fn annotated Promise<T> (Result<T>) but returning plain value
        // should return T directly to match JS runtime (e.g. wrap<T>(x:T): Promise<T> { return x; })
        if !is_async && final_ret.starts_with("morph::Result<") {
            if let Some(body_ref) = f.body.as_ref() {
                if Self::all_returns_plain(body_ref) {
                    if let Some(inner) = Self::result_inner(&final_ret) {
                        final_ret = inner;
                    }
                }
            }
        }
        // A bare class return that hands back one `new`-built local must be
        // unique: the declarator below emits `unique_ptr` for that variable,
        // so the signature has to match or the result will not compile.
        if !is_async {
            if let Some(unique_ret) = self.unique_return_class(f, &final_ret) {
                final_ret = unique_ret;
            }
        }
        // A lambda has no conversion to JsValue, so a function returning
        // only lambdas cannot honor an `any` annotation: deduce instead.
        if !is_async && final_ret == "JsValue" {
            if let Some(body_ref) = f.body.as_ref() {
                if Self::returns_only_lambdas(body_ref) {
                    final_ret = "auto".to_string();
                }
            }
        }
        self.ctx.fn_return_types.insert(name.clone(), final_ret.clone());
        self.ctx.need(&final_ret);
        let outer_fn = self.ctx.current_fn.clone();
        self.ctx.current_fn = Some(name.clone());
        // A parameter named `e` is the JsObject event by Morph convention
        // (docs, emitter lambdas): member reads lower to subscripts.
        let mut event_params = Vec::new();
        for p in &f.params.items {
            let (pname, _) = self.binding_to_identifier(&p.pattern);
            if pname == "e" {
                event_params.push(pname);
            }
        }
        let old_handlers = self.ctx.js_object_params.clone();
        self.ctx.js_object_params.extend(event_params);
        self.ctx.fn_body_depth += 1;
        if is_async {
            self.ctx.is_async_fn += 1;
        }
        // Nested functions own their returns; only the async-main wrapper
        // discards them (the old Python transpiler dropped nested ones too).
        let old_drop = self.ctx.drop_return_value;
        self.ctx.drop_return_value = false;
        let mut body =
            self.emit_function_body_with_params(f.body.as_ref().unwrap(), Some(&f.params));
        self.ctx.drop_return_value = old_drop;
        if is_async {
            self.ctx.is_async_fn -= 1;
        }
        self.ctx.fn_body_depth -= 1;
        // Ensure async void (Task) fns are coroutines even with no await/return
        if is_async
            && final_ret == "morph::Task"
            && !body.contains("co_return")
            && !body.contains("co_await")
        {
            // Inject co_return; before final }
            if let Some(pos) = body.rfind('}') {
                body.insert_str(pos, "    co_return;\n");
            }
        }
        if self.ctx.has_infinite_loop && name != "main" && !is_async {
            final_ret = "morph::Task".to_string();
            self.ctx.needed.insert("\"../../runtime/cpp/reactivity/task.h\"".to_string());
        }
        let tp = if let Some(tp) = &f.type_parameters {
            let decls: Vec<String> =
                tp.params.iter().map(|p| format!("typename {}", p.name.name)).collect();
            format!("template <{}>\n", decls.join(", "))
        } else {
            String::new()
        };
        let header = if name != "main" {
            format!("static inline\n{} {}({})", final_ret, name, params)
        } else {
            format!("{} {}({})", final_ret, name, params)
        };
        let result = if is_nested {
            format!("{}auto {} = [&]({}) -> {} {};", self.indent(), name, params, final_ret, body)
        } else {
            format!("{}{}\n{}", tp, header, body)
        };
        self.ctx.has_infinite_loop = old_has_loop || self.ctx.has_infinite_loop;
        self.ctx.current_fn = outer_fn;
        self.ctx.js_object_params = old_handlers;
        self.ctx.in_constructor = old_in_constructor;
        Some(result)
    }

    fn has_return(&self, body: &FunctionBody<'a>) -> bool {
        body.statements.iter().any(|s| self.stmt_has_return(s))
    }

    fn stmt_has_return(&self, stmt: &Statement<'a>) -> bool {
        match stmt {
            Statement::ReturnStatement(_) => true,
            Statement::BlockStatement(b) => b.body.iter().any(|s| self.stmt_has_return(s)),
            Statement::IfStatement(i) => {
                self.stmt_has_return(&i.consequent)
                    || i.alternate.as_ref().map(|a| self.stmt_has_return(a)).unwrap_or(false)
            }
            _ => false,
        }
    }

    fn result_inner(s: &str) -> Option<String> {
        let prefix = "morph::Result<";
        if !s.starts_with(prefix) || !s.ends_with('>') {
            return None;
        }
        let inner = &s[prefix.len()..s.len() - 1];
        Some(inner.trim().to_string())
    }

    fn unwrap_expr<'b>(e: &'b Expression<'a>) -> &'b Expression<'a> {
        match e {
            Expression::TSAsExpression(a) => Self::unwrap_expr(&a.expression),
            Expression::TSSatisfiesExpression(s) => Self::unwrap_expr(&s.expression),
            Expression::TSNonNullExpression(n) => Self::unwrap_expr(&n.expression),
            Expression::ParenthesizedExpression(p) => Self::unwrap_expr(&p.expression),
            Expression::TSInstantiationExpression(x) => Self::unwrap_expr(&x.expression),
            _ => e,
        }
    }

    fn return_arg_is_plain(arg: &Expression<'a>) -> bool {
        let u = Self::unwrap_expr(arg);
        match u {
            Expression::CallExpression(_) => false,
            Expression::NewExpression(_) => false,
            Expression::AwaitExpression(_) => false,
            Expression::YieldExpression(_) => false,
            Expression::ImportExpression(_) => false,
            _ => true,
        }
    }

    fn callee_name(e: &Expression<'a>) -> Option<String> {
        let u = Self::unwrap_expr(e);
        match u {
            Expression::Identifier(id) => Some(id.name.to_string()),
            Expression::StaticMemberExpression(m) => {
                // For obj.method(), return method name? For async check we need base? Return method for simplicity
                Some(m.property.name.to_string())
            }
            _ => None,
        }
    }

    fn is_async_callee(&self, name: &str) -> bool {
        if name == "fetch" {
            return true;
        }
        if self.ctx.async_fns.contains(name) {
            return true;
        }
        if let Some(a) = self.analysis.as_ref() {
            if a.async_functions.contains(name) {
                return true;
            }
        }
        false
    }

    fn strip_result_for_sync_call(&self, cpp_type: &str, init: &Expression<'a>) -> Option<String> {
        let inner = Self::result_inner(cpp_type)?;
        let u = Self::unwrap_expr(init);
        if let Expression::CallExpression(call) = u {
            if let Some(callee) = Self::callee_name(&call.callee) {
                if self.is_async_callee(&callee) {
                    return None;
                }
                // Sync call returning plain value but annotated Result -> use inner
                return Some(inner);
            }
            // Unknown callee (e.g. method call) - be conservative, keep Result
            return None;
        }
        None
    }

    fn collect_returns<'b>(stmts: &'b [Statement<'a>], out: &mut Vec<&'b Expression<'a>>) {
        for s in stmts {
            match s {
                Statement::ReturnStatement(r) => {
                    if let Some(arg) = &r.argument {
                        out.push(arg);
                    }
                }
                Statement::BlockStatement(b) => Self::collect_returns(&b.body, out),
                Statement::IfStatement(i) => {
                    Self::collect_returns(std::slice::from_ref(&i.consequent), out);
                    if let Some(alt) = &i.alternate {
                        Self::collect_returns(std::slice::from_ref(alt), out);
                    }
                }
                Statement::ForStatement(f) => {
                    Self::collect_returns(std::slice::from_ref(&f.body), out)
                }
                Statement::ForInStatement(f) => {
                    Self::collect_returns(std::slice::from_ref(&f.body), out)
                }
                Statement::ForOfStatement(f) => {
                    Self::collect_returns(std::slice::from_ref(&f.body), out)
                }
                Statement::WhileStatement(w) => {
                    Self::collect_returns(std::slice::from_ref(&w.body), out)
                }
                Statement::DoWhileStatement(d) => {
                    Self::collect_returns(std::slice::from_ref(&d.body), out)
                }
                Statement::TryStatement(t) => {
                    Self::collect_returns(&t.block.body, out);
                    if let Some(h) = &t.handler {
                        Self::collect_returns(&h.body.body, out);
                    }
                    if let Some(fin) = &t.finalizer {
                        Self::collect_returns(&fin.body, out);
                    }
                }
                Statement::SwitchStatement(sw) => {
                    for c in &sw.cases {
                        Self::collect_returns(&c.consequent, out);
                    }
                }
                Statement::LabeledStatement(l) => {
                    Self::collect_returns(std::slice::from_ref(&l.body), out)
                }
                _ => {}
            }
        }
    }

    /// Return type for a function whose body returns only literals. This
    /// keeps `auto` from swallowing an object factory: callers need the
    /// concrete `JsObject` type to choose bracket member access.
    fn literal_return_type(&self, body: &FunctionBody<'a>) -> Option<String> {
        let mut args = Vec::new();
        Self::collect_returns(&body.statements, &mut args);
        if args.is_empty() {
            return None;
        }
        let mut ty: Option<String> = None;
        for arg in args {
            let literal = matches!(
                Self::unwrap_expr(arg),
                Expression::ObjectExpression(_)
                    | Expression::ArrayExpression(_)
                    | Expression::StringLiteral(_)
                    | Expression::TemplateLiteral(_)
                    | Expression::BooleanLiteral(_)
                    | Expression::NumericLiteral(_)
                    | Expression::NullLiteral(_)
            ) || matches!(
                Self::unwrap_expr(arg),
                Expression::Identifier(id) if id.name.as_str() == "undefined"
            );
            if !literal {
                return None;
            }
            let this = self.infer_type_from_init(arg)?;
            if ty.as_ref().is_some_and(|prev| prev != &this) {
                return None;
            }
            ty = Some(this);
        }
        ty
    }

    fn all_returns_plain(body: &FunctionBody<'a>) -> bool {
        let mut args = Vec::new();
        Self::collect_returns(&body.statements, &mut args);
        if args.is_empty() {
            return false;
        }
        args.iter().all(|a| Self::return_arg_is_plain(a))
    }

    /// True when every returned value is a lambda. A lambda has no
    /// conversion to JsValue, so callers must deduce the return type.
    fn returns_only_lambdas(body: &FunctionBody<'a>) -> bool {
        let mut args = Vec::new();
        Self::collect_returns(&body.statements, &mut args);
        if args.is_empty() {
            return false;
        }
        args.iter().all(|a| {
            matches!(
                Self::unwrap_expr(a),
                Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
            )
        })
    }

    fn emit_async_main(&mut self, f: &Function<'a>, _ret: &str, _params: &str) -> String {
        self.ctx.needed.insert("\"../../runtime/cpp/reactivity/task.h\"".to_string());
        self.ctx.is_async_fn += 1;
        let old_drop = self.ctx.drop_return_value;
        self.ctx.drop_return_value = true;
        let outer_fn = self.ctx.current_fn.clone();
        let main_name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or_default();
        self.ctx.current_fn = Some(main_name);
        let body = self.emit_function_body_with_params(f.body.as_ref().unwrap(), Some(&f.params));
        self.ctx.current_fn = outer_fn;
        self.ctx.drop_return_value = old_drop;
        self.ctx.is_async_fn -= 1;
        let mut body_stripped = body.trim_end().to_string();
        if body_stripped.ends_with('}') {
            body_stripped.truncate(body_stripped.len() - 1);
            body_stripped = body_stripped.trim_end().to_string();
            body_stripped.push_str("\nco_return;\n}");
        }
        format!(
            "int main() {{\n    morph::Task _main_task = [&]() -> morph::Task {{\n{}\n    }}();\n    while (!_main_task.done()) {{\n        morph::process_tasks();\n    }}\n    return 0;\n}}",
            body_stripped
        )
    }

    /// Emit a function body, including destructuring bindings for `params`
    /// before the first user statement. The parameter list itself carries one
    /// synthetic argument per destructured pattern.
    fn emit_function_body_with_params(
        &mut self,
        body: &FunctionBody<'a>,
        params: Option<&FormalParameters<'a>>,
    ) -> String {
        if body.statements.is_empty() && params.is_none() {
            return "{}".to_string();
        }
        let mut lines = vec!["{".to_string()];
        let old = self.ctx.indent_level;
        self.ctx.indent_level = old + 1;
        if let Some(prelude) = params.and_then(|params| self.destructured_param_prelude(params)) {
            lines.push(prelude);
        }
        let scope = self.scan_hoists(&body.statements, params);
        if let Some(prelude) = self.emit_hoist_prelude(&scope) {
            lines.push(prelude);
        }
        self.hoist_stack.push(scope);
        for stmt in &body.statements {
            if let Some(code) = self.emit_statement(stmt) {
                lines.push(code);
            }
        }
        let scope = self.hoist_stack.pop();
        if let Some(scope) = scope {
            scope.restore(&mut self.ctx);
        }
        self.ctx.indent_level = old;
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn format_params(&mut self, params: &FormalParameters<'a>) -> String {
        self.format_params_impl(params, true)
    }

    fn format_params_no_defaults(&mut self, params: &FormalParameters<'a>) -> String {
        self.format_params_impl(params, false)
    }

    fn format_params_impl(
        &mut self,
        params: &FormalParameters<'a>,
        include_defaults: bool,
    ) -> String {
        let mut parts = Vec::new();
        for (index, p) in params.items.iter().enumerate() {
            if let Some(decl) = self.destructured_param_decl(p, index, include_defaults) {
                parts.push(decl);
                continue;
            }
            let (name, _) = self.binding_to_identifier(&p.pattern);
            if name.starts_with("/*") {
                continue;
            }
            // Respect explicit type annotations regardless of mode; an
            // unannotated param can still be typed by its call site
            // (`.then((response) => ...)` fixes `response`'s C++ type).
            let cpp_type = if let Some(ta) = &p.type_annotation {
                resolve_type_annotation(
                    Some(ta),
                    "auto",
                    &self.ctx.template_params,
                    false,
                    &self.ctx.class_names,
                )
            } else if let Some(ty) = self.ctx.contextual_param_types.get(&p.span.start) {
                ty.clone()
            } else {
                "auto".to_string()
            };
            // Parameters that call Array methods use JsArray so mutations
            // reach the caller's array (`sort`, `splice`, ...).
            let cpp_type = if self.js_array_receiver_vars.contains(&name)
                && (cpp_type == "auto" || cpp_type.starts_with("std::vector<"))
            {
                "JsArray".to_string()
            } else {
                cpp_type
            };
            let cpp_type = self.wrap_type(&cpp_type);
            self.ctx.need(&cpp_type);
            self.ctx.var_types.insert(name.clone(), cpp_type.clone());
            let param_cpp = param_type(&cpp_type);
            if param_cpp == "std::string_view" {
                self.ctx.need("std::string_view");
            }
            let mut decl = format!("{} {}", param_cpp, name);
            if include_defaults {
                // Default value is in FormalParameter.initializer, not in pattern.
                if let Some(init) = &p.initializer {
                    decl.push_str(&format!(" = {}", self.emit_expression(init)));
                } else if p.optional {
                    // TS `name?: T` means "may be omitted".
                    decl.push_str(&format!(" = {}", optional_param_default(&cpp_type)));
                }
            }
            parts.push(decl);
        }
        if let Some(rest) = &params.rest {
            let (name, _) = self.binding_to_identifier(&rest.rest.argument);
            let cpp_type = "JsArray".to_string();
            let cpp_type = self.wrap_type(&cpp_type);
            self.ctx.need(&cpp_type);
            self.ctx.var_types.insert(name.clone(), cpp_type.clone());
            let param_cpp = param_type(&cpp_type);
            parts.push(format!("{} {}", param_cpp, name));
        }
        parts.join(", ")
    }

    /// One C++ parameter for a destructured parameter. The pattern is
    /// lowered at the top of the function body (see
    /// [`Self::destructured_param_prelude`]) so a call site still passes one
    /// object/array argument, matching JavaScript semantics.
    fn destructured_param_decl(
        &mut self,
        p: &FormalParameter<'a>,
        index: usize,
        include_defaults: bool,
    ) -> Option<String> {
        if !matches!(p.pattern, BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_))
        {
            return None;
        }
        let cpp_type = self.destructured_param_cpp_type(p);
        let name = format!("__arg{index}");
        self.ctx.var_types.insert(name.clone(), cpp_type.clone());
        let param_cpp = param_type(&cpp_type);
        if param_cpp == "std::string_view" {
            self.ctx.need("std::string_view");
        }
        let mut decl = format!("{param_cpp} {name}");
        if include_defaults && let Some(init) = &p.initializer {
            decl.push_str(" = ");
            decl.push_str(&self.emit_expression(init));
        }
        Some(decl)
    }

    fn params_bind_name(params: &FormalParameters<'a>, name: &str) -> bool {
        params.items.iter().any(|p| Self::pattern_binds_name(&p.pattern, name))
            || params
                .rest
                .as_ref()
                .is_some_and(|rest| Self::pattern_binds_name(&rest.rest.argument, name))
    }

    fn pattern_binds_name(pattern: &BindingPattern<'a>, name: &str) -> bool {
        match pattern {
            BindingPattern::BindingIdentifier(id) => id.name.as_str() == name,
            BindingPattern::ObjectPattern(obj) => {
                obj.properties.iter().any(|prop| Self::pattern_binds_name(&prop.value, name))
                    || obj
                        .rest
                        .as_ref()
                        .is_some_and(|rest| Self::pattern_binds_name(&rest.argument, name))
            }
            BindingPattern::ArrayPattern(array) => {
                array
                    .elements
                    .iter()
                    .flatten()
                    .any(|element| Self::pattern_binds_name(element, name))
                    || array
                        .rest
                        .as_ref()
                        .is_some_and(|rest| Self::pattern_binds_name(&rest.argument, name))
            }
            BindingPattern::AssignmentPattern(assign) => {
                Self::pattern_binds_name(&assign.left, name)
            }
        }
    }

    fn destructured_param_cpp_type(&mut self, p: &FormalParameter<'a>) -> String {
        let cpp_type = if let Some(ta) = &p.type_annotation {
            resolve_type_annotation(
                Some(ta),
                "auto",
                &self.ctx.template_params,
                false,
                &self.ctx.class_names,
            )
        } else {
            "auto".to_string()
        };
        let cpp_type = self.wrap_type(&cpp_type);
        self.ctx.need(&cpp_type);
        cpp_type
    }

    /// Statements binding every name in a destructured parameter to its
    /// field/index on the synthetic argument. Emitted at function-body scope.
    fn destructured_param_prelude(&mut self, params: &FormalParameters<'a>) -> Option<String> {
        let mut lines = Vec::new();
        for (index, p) in params.items.iter().enumerate() {
            if !matches!(
                p.pattern,
                BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_)
            ) {
                continue;
            }
            let cpp_type = self.destructured_param_cpp_type(p);
            let base = format!("__arg{index}");
            // A whole-pattern default already reaches the C++ signature, but
            // an explicit `undefined` argument must trigger it too. Guard the
            // staged source lazily; native typed parameters cannot be
            // undefined and keep their precise type.
            let guarded = p.initializer.as_ref().and_then(|init| {
                if !Self::value_may_be_undefined(&cpp_type) {
                    return None;
                }
                let default = self.emit_expression(init);
                self.ctx.need("JsValue");
                Some(format!("({base}.is_undefined() ? JsValue({default}) : {base})"))
            });
            let source = guarded.as_deref().unwrap_or(&base);
            if Self::pattern_has_advanced(&p.pattern) || guarded.is_some() {
                if !self.emit_pattern_bindings(&p.pattern, source, &cpp_type, &mut lines) {
                    continue;
                }
                continue;
            }
            let mut raw: Vec<RawBinding> = Vec::new();
            if !Self::collect_access(&p.pattern, &mut Vec::new(), &mut raw) {
                continue;
            }
            let Some(dot) = Self::destructure_dot(&cpp_type, &self.ctx.class_names) else {
                continue;
            };
            if !dot
                && (cpp_type.starts_with("std::vector<") || cpp_type == "JsArray")
                && raw.iter().any(|binding| {
                    binding.steps.iter().any(|step| matches!(step, AccessStep::Key(_)))
                })
            {
                continue;
            }
            for binding in raw {
                let access = Self::render_access(&base, dot, &binding.steps);
                let elem_type = Self::access_elem_type(&cpp_type, &binding.steps);
                self.ctx.need(&elem_type);
                self.ctx.var_types.insert(binding.name.clone(), elem_type);
                lines.push(format!("{}auto {} = {};", self.indent(), binding.name, access));
            }
        }
        if lines.is_empty() { None } else { Some(lines.join("\n")) }
    }

    fn wrap_type(&mut self, cpp_type: &str) -> String {
        if self.ctx.template_params.contains(cpp_type) {
            self.ctx.need("std::shared_ptr");
            return format!("std::shared_ptr<{}>", cpp_type);
        }
        cpp_type.to_string()
    }

    /// Register field types and accessors for every named class before
    /// emission, so a method can use an accessor on a class declared later
    /// in the file.
    pub(crate) fn collect_class_metadata(&mut self, program: &Program<'a>) {
        // Names first: field inference may construct a class declared later
        // (`items = new Item()`), and `infer_type_from_init` consults
        // `class_names` to choose a pointer type.
        struct NameFinder {
            names: Vec<String>,
        }
        impl<'a> Visit<'a> for NameFinder {
            fn visit_class(&mut self, class: &Class<'a>) {
                if let Some(id) = &class.id {
                    self.names.push(id.name.to_string());
                }
                oxc_ast_visit::walk::walk_class(self, class);
            }

            fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
                if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                    (&declarator.id, &declarator.init)
                    && class_expression_of(init).is_some()
                {
                    self.names.push(id.name.to_string());
                }
                oxc_ast_visit::walk::walk_variable_declarator(self, declarator);
            }
        }
        struct Finder<'t, 'a> {
            translator: &'t mut CppTranslator<'a>,
        }
        impl<'a> Visit<'a> for Finder<'_, 'a> {
            fn visit_class(&mut self, class: &Class<'a>) {
                if let Some(id) = &class.id {
                    let name = id.name.to_string();
                    self.translator.register_class_metadata(class, &name);
                }
                oxc_ast_visit::walk::walk_class(self, class);
            }

            // `const C = class { ... }` gets metadata under the binding
            // name, matching the declaration it lowers to.
            fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
                if let (BindingPattern::BindingIdentifier(id), Some(init)) =
                    (&declarator.id, &declarator.init)
                    && let Some(class) = class_expression_of(init)
                {
                    let name = id.name.to_string();
                    self.translator.register_class_metadata(class, &name);
                }
                oxc_ast_visit::walk::walk_variable_declarator(self, declarator);
            }
        }

        let mut names = NameFinder { names: Vec::new() };
        names.visit_program(program);
        self.ctx.class_names.extend(names.names);

        let mut finder = Finder { translator: self };
        finder.visit_program(program);
    }

    /// Snapshot of the class metadata collected so far, for snippet
    /// translations in other translation units.
    pub(crate) fn class_metadata(&self) -> crate::ClassMetadata {
        crate::ClassMetadata {
            names: self.ctx.class_names.clone(),
            field_types: self.ctx.class_field_types.clone(),
            accessors: self.ctx.class_accessors.clone(),
        }
    }

    /// Infer the C++ type of every field on one class.
    ///
    /// The declaration annotation wins; otherwise the initializer and every
    /// `this.<field> = ...` write are merged. Conflicting value classes
    /// widen to `JsLateValue`. Fields assigned but never declared are
    /// synthesized so plain constructor assignment still lowers.
    fn register_class_metadata(&mut self, class: &Class<'a>, name: &str) {
        let mut fields: HashMap<String, String> = HashMap::new();
        let mut accessors: HashMap<String, AccessorFlags> = HashMap::new();
        for el in &class.body.body {
            match el {
                ClassElement::PropertyDefinition(p) => {
                    let Some(key) = class_field_key(&p.key) else { continue };
                    if let Some(ty) = self.class_field_decl_type(p) {
                        fields.insert(key, ty);
                    }
                }
                ClassElement::MethodDefinition(m) => {
                    if matches!(m.kind, MethodDefinitionKind::Get | MethodDefinitionKind::Set) {
                        let Some(key) = class_field_key(&m.key) else { continue };
                        let flags = accessors.entry(key).or_default();
                        if m.kind == MethodDefinitionKind::Get {
                            if m.r#static {
                                flags.static_getter = true;
                            } else {
                                flags.getter = true;
                            }
                        } else if m.r#static {
                            flags.static_setter = true;
                        } else {
                            flags.setter = true;
                        }
                    }
                }
                _ => {}
            }
        }
        // Constructor parameter properties are fields too; their annotated
        // or initializer type feeds later `this.<field>` inference.
        for (field, ty, _) in self.constructor_parameter_properties(class) {
            fields.entry(field).or_insert(ty);
        }
        // Make the declared types visible while merging method writes so
        // `this.count += this.step` can resolve `this.step`'s type.
        self.ctx.class_field_types.insert(name.to_string(), fields.clone());
        let old_class = self.ctx.class_name.replace(name.to_string());
        let field_uses = self.collect_this_field_uses(class, name);
        self.ctx.class_name = old_class;
        for (key, value, kind) in field_uses {
            // Accessor writes call the setter; they never declare a field.
            if accessors.contains_key(&key) {
                continue;
            }
            let Some(ty) = Self::field_use_type(value, kind) else { continue };
            match fields.get_mut(&key) {
                Some(current) => *current = merge_field_types(current, &ty),
                None => {
                    fields.insert(key, ty);
                }
            }
        }
        // A declared field with no type evidence is `undefined` in JS.
        for el in &class.body.body {
            if let ClassElement::PropertyDefinition(p) = el {
                let Some(key) = class_field_key(&p.key) else { continue };
                fields.entry(key).or_insert_with(|| "JsValue".to_string());
            }
        }
        self.ctx.class_field_types.insert(name.to_string(), fields);
        if !accessors.is_empty() {
            self.ctx.class_accessors.insert(name.to_string(), accessors);
        }
    }

    /// Field type from an annotation, or from the initializer when the
    /// annotation is absent.
    fn class_field_decl_type(&self, p: &PropertyDefinition<'a>) -> Option<String> {
        if let Some(ann) = &p.type_annotation {
            if dynamic_annotation(Some(ann)) {
                return Some("JsValue".to_string());
            }
            return Some(resolve_type_annotation(
                Some(ann),
                "auto",
                &self.ctx.template_params,
                true,
                &self.ctx.class_names,
            ));
        }
        p.value.as_ref().and_then(|value| self.infer_type_from_init(value))
    }

    /// Every `this.<field> = ...` / `this.<field>++` inside a method body,
    /// together with the inferred type of the assigned value.
    fn collect_this_field_uses(
        &self,
        class: &Class<'a>,
        class_name: &str,
    ) -> Vec<(String, Option<String>, FieldUseKind)> {
        struct Collector<'t, 'a> {
            translator: &'t CppTranslator<'a>,
            class_name: &'t str,
            /// Types of the method parameters currently being visited;
            /// metadata collection runs before parameter emission.
            param_types: HashMap<String, String>,
            uses: Vec<(String, Option<String>, FieldUseKind)>,
        }
        impl<'a> Collector<'_, 'a> {
            fn infer_use_type(&self, expr: &Expression<'a>) -> Option<String> {
                if let Expression::Identifier(id) = expr
                    && let Some(ty) = self.param_types.get(id.name.as_str())
                {
                    return Some(ty.clone());
                }
                self.translator.infer_type_from_init(expr)
            }
        }
        impl<'a> Visit<'a> for Collector<'_, 'a> {
            fn visit_assignment_expression(&mut self, a: &AssignmentExpression<'a>) {
                let key = this_field_target_key(&a.left)
                    .or_else(|| class_field_target_key(&a.left, self.class_name));
                if let Some(key) = key {
                    let kind = match a.operator.as_str() {
                        "=" | "&&=" | "||=" | "??=" => FieldUseKind::Plain,
                        "+=" => FieldUseKind::AddAssign,
                        _ => FieldUseKind::NumericAssign,
                    };
                    let ty = self.infer_use_type(&a.right);
                    self.uses.push((key, ty, kind));
                }
                oxc_ast_visit::walk::walk_assignment_expression(self, a);
            }

            fn visit_update_expression(&mut self, u: &UpdateExpression<'a>) {
                let key = this_simple_field_target_key(&u.argument)
                    .or_else(|| class_simple_field_target_key(&u.argument, self.class_name));
                if let Some(key) = key {
                    self.uses.push((key, None, FieldUseKind::Update));
                }
                oxc_ast_visit::walk::walk_update_expression(self, u);
            }

            // A nested class has its own `this`.
            fn visit_class(&mut self, _class: &Class<'a>) {}
        }
        let mut collector = Collector {
            translator: self,
            class_name,
            param_types: HashMap::new(),
            uses: Vec::new(),
        };
        for el in &class.body.body {
            if let ClassElement::MethodDefinition(m) = el
                && let Some(body) = &m.value.body
            {
                collector.param_types.clear();
                for p in &m.value.params.items {
                    let (name, _) = self.binding_to_identifier(&p.pattern);
                    if name.starts_with("/*") {
                        continue;
                    }
                    let ty = p.type_annotation.as_ref().map_or_else(
                        || {
                            p.initializer
                                .as_ref()
                                .and_then(|init| self.infer_type_from_init(init))
                                .unwrap_or_else(|| "auto".to_string())
                        },
                        |ta| {
                            if dynamic_annotation(Some(ta)) {
                                "JsValue".to_string()
                            } else {
                                resolve_type_annotation(
                                    Some(ta),
                                    "auto",
                                    &self.ctx.template_params,
                                    false,
                                    &self.ctx.class_names,
                                )
                            }
                        },
                    );
                    collector.param_types.insert(name, ty);
                }
                collector.visit_function_body(body);
            }
        }
        collector.uses
    }

    fn field_use_type(value: Option<String>, kind: FieldUseKind) -> Option<String> {
        match kind {
            FieldUseKind::Update => Some("int64_t".to_string()),
            FieldUseKind::NumericAssign => {
                let ty = value.unwrap_or_else(|| "JsNumber".to_string());
                Some(if is_numeric_field_type(&ty) { ty } else { "JsNumber".to_string() })
            }
            FieldUseKind::AddAssign => {
                let ty = value.unwrap_or_else(|| "JsValue".to_string());
                Some(if is_text_field_type(&ty) {
                    "JsString".to_string()
                } else if is_numeric_field_type(&ty) {
                    ty
                } else {
                    "JsLateValue".to_string()
                })
            }
            FieldUseKind::Plain => value,
        }
    }

    /// Constructor parameter properties (`constructor(public x: number)`):
    /// every parameter with an accessibility modifier or `readonly` also
    /// declares an instance field initialized from that argument.
    fn constructor_parameter_properties(&self, class: &Class<'a>) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        for el in &class.body.body {
            let ClassElement::MethodDefinition(m) = el else { continue };
            if m.kind != MethodDefinitionKind::Constructor {
                continue;
            }
            for p in &m.value.params.items {
                if p.accessibility.is_none() && !p.readonly {
                    continue;
                }
                let (field, _) = self.binding_to_identifier(&p.pattern);
                if field.starts_with("/*") {
                    continue;
                }
                let ty = p.type_annotation.as_ref().map_or_else(
                    || {
                        p.initializer
                            .as_ref()
                            .and_then(|init| self.infer_type_from_init(init))
                            .unwrap_or_else(|| "JsValue".to_string())
                    },
                    |ann| {
                        if dynamic_annotation(Some(ann)) {
                            "JsValue".to_string()
                        } else {
                            resolve_type_annotation(
                                Some(ann),
                                "auto",
                                &self.ctx.template_params,
                                true,
                                &self.ctx.class_names,
                            )
                        }
                    },
                );
                let access = match p.accessibility {
                    Some(TSAccessibility::Private) => "private",
                    Some(TSAccessibility::Protected) => "protected",
                    _ => "public",
                }
                .to_string();
                out.push((field, ty, access));
            }
        }
        out
    }

    fn emit_class(&mut self, class: &Class<'a>) -> String {
        self.emit_class_named(class, None)
    }

    fn emit_class_named(&mut self, class: &Class<'a>, override_name: Option<&str>) -> String {
        let name = override_name
            .map(str::to_string)
            .or_else(|| class.id.as_ref().map(|id| id.name.to_string()))
            .unwrap_or_else(|| "DefaultClass".to_string());
        if !name.is_empty() {
            self.ctx.class_names.insert(name.clone());
            self.register_class_metadata(class, &name);
        }
        let old_tp = self.ctx.template_params.clone();
        let old_class = self.ctx.class_name.clone();
        let old_super = self.ctx.super_class_name.clone();
        let old_in_constructor = self.ctx.in_constructor;
        self.ctx.in_constructor = false;
        self.ctx.class_name = Some(name.clone());
        // Store super class name for constructor super() handling
        if let Some(heritage) = &class.heritage {
            if let Expression::Identifier(id) = &heritage.expression {
                self.ctx.super_class_name = Some(id.name.to_string());
            } else {
                self.ctx.super_class_name = Some(self.emit_expression(&heritage.expression));
            }
        } else {
            self.ctx.super_class_name = None;
        }
        let mut lines = Vec::new();
        if let Some(tp) = &class.type_parameters {
            let decls: Vec<String> =
                tp.params.iter().map(|p| format!("typename {}", p.name.name)).collect();
            let names: Vec<String> = tp.params.iter().map(|p| p.name.name.to_string()).collect();
            for n in &names {
                self.ctx.template_params.insert(n.clone());
            }
            lines.push(format!("template <{}>", decls.join(", ")));
        }
        let mut bases = Vec::new();
        let mut unknown_bases = Vec::new();
        if let Some(heritage) = &class.heritage {
            let expr = &heritage.expression;
            if let Expression::Identifier(id) = expr {
                if self.ctx.class_names.contains(id.name.as_str()) {
                    bases.push(format!("public {}", id.name));
                } else {
                    unknown_bases.push(id.name.to_string());
                }
            } else {
                bases.push(format!("public {}", self.emit_expression(expr)));
            }
        }
        for imp in &class.implements {
            let base = ts_type_name_to_string(&imp.expression);
            if self.ctx.class_names.contains(base.as_str()) {
                bases.push(format!("public {}", base));
            } else {
                unknown_bases.push(base);
            }
        }
        let heritage_str =
            if bases.is_empty() { String::new() } else { format!(" : {}", bases.join(", ")) };
        lines.push(format!("class {}{} {{", name, heritage_str));
        for unknown in &unknown_bases {
            lines.push(format!("{}// unsupported: unknown base {}", INDENT, unknown));
        }
        // Collect private field names for this class
        let mut private_fields = HashSet::new();
        for el in &class.body.body {
            if let ClassElement::PropertyDefinition(p) = el {
                if let PropertyKey::PrivateIdentifier(priv_id) = &p.key {
                    private_fields.insert(priv_id.name.to_string());
                }
            }
        }
        if !private_fields.is_empty() {
            self.ctx.private_fields.insert(name.clone(), private_fields.clone());
        }
        // Increase indent for class body (like Python's sub)
        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level = 1;
        let mut access_map: HashMap<String, Vec<String>> = HashMap::new();
        // Parameter properties are real fields even though they never have
        // a PropertyDefinition. A declared field of the same name wins.
        let declared_fields: HashSet<String> = class
            .body
            .body
            .iter()
            .filter_map(|el| match el {
                ClassElement::PropertyDefinition(p) => class_field_key(&p.key),
                _ => None,
            })
            .collect();
        let mut known_fields = declared_fields;
        for (field, ty, access) in self.constructor_parameter_properties(class) {
            if !known_fields.insert(field.clone()) {
                continue;
            }
            let ty = self.wrap_type(&ty);
            self.ctx.need(&ty);
            access_map.entry(access).or_default().push(format!("{} {};", ty, field));
        }
        // Fields assigned only through `this.x = ...` / `Counter.x = ...`
        // still need a declaration. Sort for stable output.
        let mut synthesized: Vec<(String, String)> = self
            .ctx
            .class_field_types
            .get(&name)
            .map(|fields| fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();
        synthesized.sort_by(|a, b| a.0.cmp(&b.0));
        for (field, ty) in synthesized {
            if field.starts_with('#') || !known_fields.insert(field.clone()) {
                continue;
            }
            let ty = self.wrap_type(&ty);
            self.ctx.need(&ty);
            access_map.entry("public".to_string()).or_default().push(format!("{} {};", ty, field));
        }
        for el in &class.body.body {
            // `#field` declarations emit in the private section below with
            // their mangled name and inferred type.
            if matches!(el, ClassElement::PropertyDefinition(p)
                if matches!(p.key, PropertyKey::PrivateIdentifier(_)))
            {
                continue;
            }
            let (access, code) = self.emit_class_element(el, &name);
            access_map.entry(access).or_default().push(code);
        }
        self.ctx.indent_level = old_indent;
        let order = ["public", "private", "protected"];
        let mut first = true;
        for acc in order {
            if let Some(members) = access_map.get(acc) {
                if members.is_empty() {
                    continue;
                }
                if !first {
                    lines.push(String::new());
                }
                first = false;
                lines.push(format!("{}:", acc));
                for m in members {
                    if m.trim().is_empty() {
                        continue;
                    }
                    lines.push(format!("{}{}", INDENT, m));
                }
            }
        }
        for (acc, members) in &access_map {
            if order.contains(&acc.as_str()) {
                continue;
            }
            if !first {
                lines.push(String::new());
            }
            first = false;
            lines.push(format!("{}:", acc));
            for m in members {
                lines.push(format!("{}{}", INDENT, m));
            }
        }
        // Emit `#private` fields with mangled names and inferred types.
        let private_defs: Vec<&PropertyDefinition<'a>> = class
            .body
            .body
            .iter()
            .filter_map(|el| match el {
                ClassElement::PropertyDefinition(p)
                    if matches!(p.key, PropertyKey::PrivateIdentifier(_)) =>
                {
                    Some(p.as_ref())
                }
                _ => None,
            })
            .collect();
        if !private_defs.is_empty() {
            if !first {
                lines.push(String::new());
            }
            lines.push("private:".to_string());
            for p in private_defs {
                let PropertyKey::PrivateIdentifier(priv_id) = &p.key else { continue };
                let field = priv_id.name.to_string();
                let mangled = format!("__private_{}_{}", name, field);
                let cpp_type = self
                    .ctx
                    .class_field_types
                    .get(&name)
                    .and_then(|fields| fields.get(&format!("#{field}")))
                    .cloned()
                    .unwrap_or_else(|| "JsValue".to_string());
                let cpp_type = self.wrap_type(&cpp_type);
                self.ctx.need(&cpp_type);
                let prefix = if p.r#static { "static inline " } else { "" };
                if let Some(value) = &p.value {
                    let saved_static = self.ctx.in_static_method;
                    self.ctx.in_static_method = p.r#static;
                    let value = self.emit_expression(value);
                    self.ctx.in_static_method = saved_static;
                    lines
                        .push(format!("{}{}{} {} = {};", INDENT, prefix, cpp_type, mangled, value));
                } else {
                    lines.push(format!("{}{}{} {};", INDENT, prefix, cpp_type, mangled));
                }
            }
        }
        let all_bases: Vec<String> = {
            let mut v = Vec::new();
            if let Some(heritage) = &class.heritage {
                if let Expression::Identifier(id) = &heritage.expression {
                    v.push(id.name.to_string());
                }
            }
            for imp in &class.implements {
                v.push(ts_type_name_to_string(&imp.expression));
            }
            v
        };
        for base_name in all_bases {
            if let Some(props) = self.ctx.interface_props.get(&base_name).cloned() {
                for (prop_name, prop_type) in props {
                    let getter = format!("get{}{}", prop_name[..1].to_uppercase(), &prop_name[1..]);
                    lines.push(format!(
                        "{}{} {}() const override {{ return {}; }}",
                        INDENT, prop_type, getter, prop_name
                    ));
                }
            }
        }
        lines.push("};".to_string());
        self.ctx.class_name = old_class;
        self.ctx.super_class_name = old_super;
        self.ctx.template_params = old_tp;
        self.ctx.in_constructor = old_in_constructor;
        lines.join("\n")
    }

    fn format_constructor_params(&mut self, params: &FormalParameters<'a>) -> String {
        let mut parts = Vec::new();
        for (index, p) in params.items.iter().enumerate() {
            if let Some(decl) = self.destructured_param_decl(p, index, false) {
                parts.push(decl);
                continue;
            }
            let (name, _) = self.binding_to_identifier(&p.pattern);
            if name.starts_with("/*") {
                continue;
            }
            let cpp_type = p
                .type_annotation
                .as_ref()
                .map(|ta| {
                    resolve_type_annotation(
                        Some(ta),
                        "auto",
                        &self.ctx.template_params,
                        false,
                        &self.ctx.class_names,
                    )
                })
                .unwrap_or_else(|| "auto".to_string());
            let cpp_type = self.wrap_type(&cpp_type);
            self.ctx.need(&cpp_type);
            let param_type_str = param_type(&cpp_type);
            if param_type_str == "std::string_view" {
                self.ctx.needed.insert("<string_view>".to_string());
            }
            let mut decl = format!("{} p_{}", param_type_str, name);
            if let Some(init) = &p.initializer {
                decl.push_str(&format!(" = {}", self.emit_expression(init)));
            } else if p.optional {
                decl.push_str(&format!(" = {}", optional_param_default(&cpp_type)));
            }
            parts.push(decl);
        }
        if let Some(rest) = &params.rest {
            let (name, _) = self.binding_to_identifier(&rest.rest.argument);
            parts.push(format!("auto p_{}", name));
        }
        parts.join(", ")
    }

    fn emit_class_element(&mut self, el: &ClassElement<'a>, class_name: &str) -> (String, String) {
        match el {
            ClassElement::MethodDefinition(m) => {
                let private_key = matches!(m.key, PropertyKey::PrivateIdentifier(_));
                let name = if let PropertyKey::PrivateIdentifier(p) = &m.key {
                    format!("__private_{}_{}", class_name, p.name)
                } else {
                    self.property_key_to_string(&m.key).unwrap_or_default()
                };
                let access = m
                    .accessibility
                    .map(|a| format!("{:?}", a).to_lowercase())
                    .unwrap_or_else(|| "public".to_string());
                let access = if private_key {
                    "private".to_string()
                } else {
                    match access.as_str() {
                        "private" => "private".to_string(),
                        "protected" => "protected".to_string(),
                        _ => "public".to_string(),
                    }
                };
                if m.kind == MethodDefinitionKind::Constructor {
                    // Parameter defaults and the initializer list run inside
                    // the constructor call, so `new.target` is available.
                    let old_in_constructor = self.ctx.in_constructor;
                    self.ctx.in_constructor = true;
                    // Use constructor-specific formatting with p_ prefix and initializer list (like Python)
                    let has_destructured = m.value.params.items.iter().any(|p| {
                        matches!(
                            p.pattern,
                            BindingPattern::ObjectPattern(_) | BindingPattern::ArrayPattern(_)
                        )
                    });
                    let params_str = self.format_constructor_params(&m.value.params);
                    let param_names: std::collections::HashSet<String> = m
                        .value
                        .params
                        .items
                        .iter()
                        .filter_map(|p| {
                            let (n, _) = self.binding_to_identifier(&p.pattern);
                            if n.starts_with("/*") { None } else { Some(n) }
                        })
                        .collect();
                    let mut super_entries: Vec<String> = Vec::new();
                    let mut init_entries: Vec<String> = Vec::new();
                    let mut initialized_fields: HashSet<String> = HashSet::new();
                    // Parameter properties initialize before the body, like
                    // the `this.x = p_x` form they desugar to.
                    for p in &m.value.params.items {
                        if p.accessibility.is_none() && !p.readonly {
                            continue;
                        }
                        let (field, _) = self.binding_to_identifier(&p.pattern);
                        if field.starts_with("/*") || !initialized_fields.insert(field.clone()) {
                            continue;
                        }
                        init_entries.push(format!("{field}(std::move(p_{field}))"));
                    }
                    let mut remaining: Vec<&Statement<'a>> = Vec::new();
                    let mut super_seen = false;
                    if let Some(body) = &m.value.body {
                        for stmt in &body.statements {
                            if !super_seen {
                                if let Statement::ExpressionStatement(es) = stmt {
                                    if let Expression::CallExpression(call) = &es.expression {
                                        if let Expression::Super(_) = &call.callee {
                                            super_seen = true;
                                            let super_is_known =
                                                self.ctx.super_class_name.as_ref().is_some_and(
                                                    |b| self.ctx.class_names.contains(b.as_str()),
                                                );
                                            if !super_is_known {
                                                continue;
                                            }
                                            let base = self
                                                .ctx
                                                .super_class_name
                                                .clone()
                                                .unwrap_or_else(|| "Base".to_string());
                                            let super_args: Vec<String> = call
                                                .arguments
                                                .iter()
                                                .filter_map(|a| a.as_expression())
                                                .map(|e| {
                                                    if let Expression::Identifier(id) = e {
                                                        if param_names.contains(id.name.as_str()) {
                                                            return format!(
                                                                "std::move(p_{})",
                                                                id.name
                                                            );
                                                        }
                                                    }
                                                    self.emit_expression(e)
                                                })
                                                .collect();
                                            super_entries.push(format!(
                                                "{}({})",
                                                base,
                                                super_args.join(", ")
                                            ));
                                            continue;
                                        }
                                    }
                                }
                            }
                            // Check for this.prop = rhs
                            let mut is_this_assign = false;
                            if let Statement::ExpressionStatement(es) = stmt {
                                if let Expression::AssignmentExpression(assign) = &es.expression {
                                    if assign.operator.as_str() == "=" {
                                        if let AssignmentTarget::StaticMemberExpression(mem) =
                                            &assign.left
                                        {
                                            if let Expression::ThisExpression(_) = &mem.object {
                                                let prop = mem.property.name.to_string();
                                                let rhs_str = if let Expression::Identifier(id) =
                                                    &assign.right
                                                {
                                                    if param_names.contains(id.name.as_str()) {
                                                        format!("p_{}", id.name)
                                                    } else {
                                                        self.emit_expression(&assign.right)
                                                    }
                                                } else {
                                                    self.emit_expression(&assign.right)
                                                };
                                                if !has_destructured
                                                    && initialized_fields.insert(prop.clone())
                                                {
                                                    init_entries.push(format!(
                                                        "{prop}(std::move({rhs_str}))"
                                                    ));
                                                    is_this_assign = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            if is_this_assign {
                                continue;
                            }
                            remaining.push(stmt);
                        }
                    }
                    let init_entries: Vec<String> =
                        super_entries.into_iter().chain(init_entries).collect();
                    let init_str = if init_entries.is_empty() {
                        String::new()
                    } else {
                        format!("\n{}{}: {}", self.indent(), INDENT, init_entries.join(", "))
                    };
                    // Build body with remaining statements. Destructured
                    // constructor params bind after the initializer list
                    // because the prelude names are locals, not parameters.
                    let body_str = if remaining.is_empty() && !has_destructured {
                        "{}".to_string()
                    } else {
                        let mut lines = vec!["{".to_string()];
                        let old_indent = self.ctx.indent_level;
                        self.ctx.indent_level = old_indent + 1;
                        self.ctx.fn_body_depth += 1;
                        if has_destructured
                            && let Some(prelude) = self.destructured_param_prelude(&m.value.params)
                        {
                            lines.push(prelude);
                        }
                        for stmt in remaining {
                            if let Some(code) = self.emit_statement(stmt) {
                                lines.push(code);
                            }
                        }
                        self.ctx.fn_body_depth -= 1;
                        self.ctx.indent_level = old_indent;
                        lines.push(format!("{}}}", self.indent()));
                        lines.join("\n")
                    };
                    let rendered =
                        format!("{}({}){} {}", class_name, params_str, init_str, body_str);
                    self.ctx.in_constructor = old_in_constructor;
                    return (access, rendered);
                } else {
                    let params = self.format_params(&m.value.params);
                    let ret = if let Some(rt) = &m.value.return_type {
                        resolve_type_annotation(
                            Some(rt),
                            "void",
                            &self.ctx.template_params,
                            false,
                            &self.ctx.class_names,
                        )
                    } else {
                        if m.value.r#async {
                            "JsValue".to_string()
                        } else if m.value.body.as_ref().map(|b| self.has_return(b)).unwrap_or(false)
                        {
                            "auto".to_string()
                        } else {
                            "void".to_string()
                        }
                    };
                    let mut final_ret = ret;
                    if m.value.r#async {
                        final_ret = self.ctx.async_result_type(&final_ret);
                    }
                    final_ret = self.wrap_type(&final_ret);
                    // A lambda has no conversion to JsValue, so a method
                    // returning only lambdas cannot honor `any`: deduce.
                    if !m.value.r#async && final_ret == "JsValue" {
                        if let Some(b) = m.value.body.as_ref() {
                            if Self::returns_only_lambdas(b) {
                                final_ret = "auto".to_string();
                            }
                        }
                    }
                    self.ctx.need(&final_ret);
                    if m.value.r#async {
                        self.ctx.is_async_fn += 1;
                    }
                    let body = m
                        .value
                        .body
                        .as_ref()
                        .map(|b| {
                            let saved_drop = self.ctx.drop_return_value;
                            self.ctx.drop_return_value = false;
                            let saved_static = self.ctx.in_static_method;
                            self.ctx.in_static_method = m.r#static;
                            // Method bodies count as function scope: nested
                            // arrows capture `this` via `[&]`, and nested
                            // declarations lower to lambdas.
                            self.ctx.fn_body_depth += 1;
                            let body =
                                self.emit_function_body_with_params(b, Some(&m.value.params));
                            self.ctx.fn_body_depth -= 1;
                            self.ctx.in_static_method = saved_static;
                            self.ctx.drop_return_value = saved_drop;
                            body
                        })
                        .unwrap_or_else(|| "{}".to_string());
                    if m.value.r#async {
                        self.ctx.is_async_fn -= 1;
                    }
                    let static_prefix = if m.r#static { "static " } else { "" };
                    return (
                        access,
                        format!("{}{} {}({}) {}", static_prefix, final_ret, name, params, body),
                    );
                }
            }
            ClassElement::PropertyDefinition(p) => {
                let name = self.property_key_to_string(&p.key).unwrap_or_default();
                let access = p
                    .accessibility
                    .map(|a| format!("{:?}", a).to_lowercase())
                    .unwrap_or_else(|| "public".to_string());
                let access = match access.as_str() {
                    "private" => "private".to_string(),
                    "protected" => "protected".to_string(),
                    _ => "public".to_string(),
                };
                let cpp_type = if let Some(ann) = &p.type_annotation {
                    resolve_type_annotation(
                        Some(ann),
                        "auto",
                        &self.ctx.template_params,
                        true,
                        &self.ctx.class_names,
                    )
                } else {
                    let key = class_field_key(&p.key).unwrap_or_else(|| name.clone());
                    self.ctx
                        .class_field_types
                        .get(class_name)
                        .and_then(|fields| fields.get(&key))
                        .cloned()
                        .unwrap_or_else(|| "JsValue".to_string())
                };
                let cpp_type = self.wrap_type(&cpp_type);
                self.ctx.need(&cpp_type);
                let prefix = if p.r#static { "static inline " } else { "" };
                if let Some(val) = &p.value {
                    let saved_static = self.ctx.in_static_method;
                    self.ctx.in_static_method = p.r#static;
                    let value = self.emit_expression(val);
                    self.ctx.in_static_method = saved_static;
                    (access, format!("{}{} {} = {};", prefix, cpp_type, name, value))
                } else {
                    (access, format!("{}{} {};", prefix, cpp_type, name))
                }
            }
            ClassElement::AccessorProperty(a) => {
                let name = self.property_key_to_string(&a.key).unwrap_or_default();
                let access = a
                    .accessibility
                    .map(|x| format!("{:?}", x).to_lowercase())
                    .unwrap_or_else(|| "public".to_string());
                (access, format!("/* accessor {} */", name))
            }
            ClassElement::StaticBlock(_) => {
                ("public".to_string(), "/* static block */".to_string())
            }
            ClassElement::TSIndexSignature(_) => {
                ("public".to_string(), "/* index signature */".to_string())
            }
        }
    }

    /// One statement inside a namespace body. Export wrappers unwrap to
    /// their declaration; bare re-exports have nothing to lower.
    fn emit_namespace_stmt(&mut self, stmt: &Statement<'a>) -> Option<String> {
        let code = match stmt {
            Statement::ExportDeclaration(e) => self.emit_declaration(&e.declaration),
            Statement::ExportDefaultDeclaration(e) => self.emit_export_default(&e.declaration),
            _ => self.emit_statement(stmt),
        };
        code.filter(|c| !c.trim().is_empty())
    }

    /// A TypeScript namespace lowers to a C++ namespace of the same name.
    /// Exported members emit as ordinary (non-static) declarations; a
    /// nested namespace recurses.
    fn emit_namespace(&mut self, decl: &TSNamespaceDeclaration<'a>) -> Option<String> {
        let name = decl.id.name.to_string();
        let mut lines = vec![format!("{}namespace {} {{", self.indent(), name)];
        let old = self.ctx.indent_level;
        self.ctx.indent_level = old + 1;
        let mut emitted_any = false;
        match &decl.body {
            TSNamespaceDeclarationBody::TSModuleBlock(block) => {
                for stmt in &block.body {
                    if let Some(code) = self.emit_namespace_stmt(stmt) {
                        lines.push(code);
                        emitted_any = true;
                    }
                }
            }
            TSNamespaceDeclarationBody::TSNamespaceDeclaration(nested) => {
                if let Some(code) = self.emit_namespace(nested) {
                    lines.push(code);
                    emitted_any = true;
                }
            }
        }
        self.ctx.indent_level = old;
        lines.push(format!("{}}}  // namespace {}", self.indent(), name));
        if emitted_any { Some(lines.join("\n")) } else { None }
    }

    fn emit_interface(&mut self, iface: &TSInterfaceDeclaration<'a>) -> String {
        let name = iface.id.name.to_string();
        self.ctx.class_names.insert(name.clone());
        let old_tp = self.ctx.template_params.clone();
        let mut lines = Vec::new();
        if let Some(tp) = &iface.type_parameters {
            let decls: Vec<String> =
                tp.params.iter().map(|p| format!("typename {}", p.name.name)).collect();
            let names: Vec<String> = tp.params.iter().map(|p| p.name.name.to_string()).collect();
            for n in &names {
                self.ctx.template_params.insert(n.clone());
            }
            lines.push(format!("template <{}>", decls.join(", ")));
        }
        lines.push(format!("class {} {{", name));
        lines.push("public:".to_string());
        let mut props_map = HashMap::new();
        for sig in &iface.body.body {
            match sig {
                TSSignature::TSPropertySignature(p) => {
                    if let Some(key) = self.property_key_to_string(&p.key) {
                        let cpp_type = if let Some(ann) = &p.type_annotation {
                            resolve_type_annotation(
                                Some(ann),
                                "auto",
                                &self.ctx.template_params,
                                false,
                                &self.ctx.class_names,
                            )
                        } else {
                            "auto".to_string()
                        };
                        let getter = format!("get{}{}", key[..1].to_uppercase(), &key[1..]);
                        lines.push(format!(
                            "{}virtual {} {}() const = 0;",
                            INDENT, cpp_type, getter
                        ));
                        props_map.insert(key, cpp_type);
                    }
                }
                TSSignature::TSMethodSignature(m) => {
                    if let Some(key) = self.property_key_to_string(&m.key) {
                        let ret = if let Some(rt) = &m.return_type {
                            resolve_type_annotation(
                                Some(rt),
                                "void",
                                &self.ctx.template_params,
                                false,
                                &self.ctx.class_names,
                            )
                        } else {
                            "void".to_string()
                        };
                        let params = self.format_params(&m.params);
                        lines.push(format!("{}virtual {} {}({}) = 0;", INDENT, ret, key, params));
                    }
                }
                _ => {}
            }
        }
        self.ctx.interface_props.insert(name.clone(), props_map);
        lines.push(format!("{}virtual ~{}() = default;", INDENT, name));
        lines.push("};".to_string());
        self.ctx.template_params = old_tp;
        lines.join("\n")
    }

    fn emit_expression(&mut self, expr: &Expression<'a>) -> String {
        match expr {
            Expression::Identifier(id) => {
                if let Some(mapped) = self.ctx.state_vars.get(id.name.as_str()) {
                    return mapped.clone();
                }
                if let Some(narrowed) = self.active_narrows.get(id.name.as_str()).cloned() {
                    return narrowed;
                }
                if id.name == "undefined" {
                    return "JsUndefined{}".to_string();
                }
                if id.name == "null" {
                    return "JsNull{}".to_string();
                }
                if id.name == "NaN" {
                    return "std::numeric_limits<double>::quiet_NaN()".to_string();
                }
                if id.name == "Infinity" {
                    return "std::numeric_limits<double>::infinity()".to_string();
                }
                if let Some(dereferenced) = self.deref_shared_scalar(id.name.as_str()) {
                    return dereferenced;
                }
                id.name.to_string()
            }
            Expression::NumericLiteral(n) => self.emit_number(n),
            Expression::StringLiteral(s) => cpp_string_literal(&s.value),
            Expression::BooleanLiteral(b) => {
                if b.value {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            Expression::NullLiteral(_) => "JsNull{}".to_string(),
            Expression::TemplateLiteral(t) => self.emit_template_literal(t),
            Expression::ArrayExpression(arr) => self.emit_array(arr),
            Expression::ObjectExpression(obj) => self.emit_object(obj),
            Expression::CallExpression(c) => self.emit_call(c),
            Expression::AwaitExpression(a) => {
                let argument = self.emit_expression(&a.argument);
                // `Response.json()` / `.text()` / `.arrayBuffer()` are
                // synchronous in the native runtime even though browser
                // code awaits them. Awaiting a plain value would emit
                // `co_await` on a non-awaitable type.
                if Self::is_sync_response_body_call(&a.argument) {
                    argument
                } else {
                    format!("co_await {argument}")
                }
            }
            Expression::BinaryExpression(b) => self.emit_binary(b),
            Expression::LogicalExpression(b) => {
                let op = b.operator.as_str();
                if op == "??" {
                    let left = self.emit_expression(&b.left);
                    let right = self.emit_expression(&b.right);
                    // Need JsValue for is_undefined/is_null checks
                    self.ctx.need("JsValue");
                    return format!(
                        "(JsValue({}).is_undefined() || JsValue({}).is_null() ? JsValue({}) : JsValue({}))",
                        left, left, right, left
                    );
                }
                if op == "&&" || op == "||" {
                    return self.emit_logical(b, op);
                }
                format!(
                    "{} {} {}",
                    self.emit_expression(&b.left),
                    op,
                    self.emit_expression(&b.right)
                )
            }
            Expression::UnaryExpression(u) => {
                if u.operator.as_str() == "!" {
                    let argument_class = self.operand_class_of(&u.argument);
                    let argument = self.emit_expression(&u.argument);
                    if Self::needs_truthy_wrapper(argument_class) {
                        self.comparison_uses.insert(ComparisonSignature::truthy(argument_class));
                        return format!("!morph::js_cmp::is_truthy({})", argument);
                    }
                    return format!("!{}", argument);
                }
                let op = u.operator.as_str();
                let argument = self.emit_expression(&u.argument);
                let needs_space = matches!(op, "delete" | "typeof" | "void" | "await");
                if needs_space {
                    format!("{} {}", op, argument)
                } else {
                    format!("{}{}", op, argument)
                }
            }
            Expression::UpdateExpression(u) => {
                if let SimpleAssignmentTarget::AssignmentTargetIdentifier(id) = &u.argument
                    && let Some(set_head) = self.signal_set_head(id.name.as_str())
                {
                    let read =
                        self.ctx.state_vars.get(id.name.as_str()).cloned().unwrap_or_default();
                    let late_signal = self
                        .ctx
                        .var_types
                        .get(id.name.as_str())
                        .is_some_and(|ty| ty.contains("JsLateValue"));
                    if late_signal {
                        let helper =
                            if u.operator.as_str() == "++" { "js_late_add" } else { "js_late_sub" };
                        return format!("{set_head}(morph::{helper}({read}, 1))");
                    }
                    let bin = if u.operator.as_str() == "++" { "+" } else { "-" };
                    return format!("{set_head}(({read}) {bin} 1)");
                }
                if let Some(rendered) = self.emit_accessor_update(u) {
                    return rendered;
                }
                let arg = self.emit_simple_target(&u.argument);
                if u.prefix {
                    format!("{}{}", u.operator.as_str(), arg)
                } else {
                    format!("{}{}", arg, u.operator.as_str())
                }
            }
            Expression::AssignmentExpression(a) => self.emit_assignment(a),
            Expression::ParenthesizedExpression(p) => {
                format!("({})", self.emit_expression(&p.expression))
            }
            Expression::ConditionalExpression(c) => {
                let test = self.emit_truthy_test(&c.test);
                format!(
                    "({} ? {} : {})",
                    test,
                    self.emit_expression(&c.consequent),
                    self.emit_expression(&c.alternate)
                )
            }
            Expression::SequenceExpression(s) => {
                s.expressions.iter().map(|e| self.emit_expression(e)).collect::<Vec<_>>().join(", ")
            }
            Expression::ThisExpression(_) => {
                if self.ctx.fn_expr_depth > 0 {
                    "_jsThis".to_string()
                } else if self.ctx.in_static_method {
                    self.ctx.class_name.clone().unwrap_or_else(|| "this".to_string())
                } else {
                    "this".to_string()
                }
            }
            Expression::Super(_) => "super".to_string(),
            Expression::NewExpression(n) => self.emit_new(n),
            Expression::ArrowFunctionExpression(f) => self.emit_arrow(f),
            Expression::FunctionExpression(f) => self.emit_function_expression(f),
            Expression::ComputedMemberExpression(m) => format!(
                "{}[{}]",
                self.emit_expression(&m.object),
                self.emit_expression(&m.expression)
            ),
            Expression::StaticMemberExpression(m) => self.emit_static_member(m),
            Expression::PrivateFieldExpression(p) => {
                let obj = self.emit_expression(&p.object);
                let field_name = p.field.name.to_string();
                self.private_field_access(&p.object, &field_name, &obj)
            }
            Expression::ChainExpression(chain) => self.emit_chain(chain),
            Expression::TaggedTemplateExpression(t) => format!(
                "{}({})",
                self.emit_expression(&t.tag),
                self.emit_template_literal(&t.quasi)
            ),
            Expression::ImportExpression(_) => "/* import */".to_string(),
            Expression::YieldExpression(y) => {
                if let Some(arg) = &y.argument {
                    format!("co_yield {}", self.emit_expression(arg))
                } else {
                    "co_yield".to_string()
                }
            }
            Expression::PrivateInExpression(p) => {
                format!("{} in {}", p.left.name, self.emit_expression(&p.right))
            }
            Expression::JSXElement(_) | Expression::JSXFragment(_) => "/* jsx */".to_string(),
            Expression::TSAsExpression(a) => self.emit_expression(&a.expression),
            Expression::TSSatisfiesExpression(s) => self.emit_expression(&s.expression),
            Expression::TSTypeAssertion(a) => self.emit_expression(&a.expression),
            Expression::TSNonNullExpression(n) => self.emit_expression(&n.expression),
            Expression::TSInstantiationExpression(e) => self.emit_expression(&e.expression),
            Expression::V8IntrinsicExpression(_) => "/* v8 intrinsic */".to_string(),
            Expression::NewTarget(_) => {
                // Only constructors observe a target; the lowering for
                // `new` installs it for the most-derived class. Plain
                // functions, methods and field initializers see undefined.
                if self.ctx.in_constructor {
                    self.ctx.need("JsValue");
                    "morph::js_new_target()".to_string()
                } else {
                    "JsUndefined{}".to_string()
                }
            }
            Expression::ImportMeta(_) => "/* import.meta */".to_string(),
            Expression::RegExpLiteral(r) => self.span_text(r.span).to_string(),
            Expression::BigIntLiteral(b) => {
                if let Some(raw) = &b.raw {
                    let s = raw.as_str();
                    s.strip_suffix('n').unwrap_or(s).to_string()
                } else {
                    self.span_text(b.span).to_string()
                }
            }
            _ => "/* unhandled expression */".to_string(),
        }
    }

    fn emit_number(&self, n: &NumericLiteral<'a>) -> String {
        if let Some(raw) = n.raw {
            let raw_str = raw.as_str();
            if raw_str.starts_with("0x")
                || raw_str.starts_with("0X")
                || raw_str.starts_with("0o")
                || raw_str.starts_with("0b")
            {
                return raw_str.to_string();
            }
            if raw_str.contains('.') || raw_str.to_ascii_lowercase().contains('e') {
                let val = n.value;
                if val.fract() == 0.0 {
                    return format!("{:.1}", val);
                }
                return raw_str.to_string();
            }
            if n.value.fract() == 0.0 && n.value.abs() < i64::MAX as f64 {
                return (n.value as i64).to_string();
            }
            return raw_str.to_string();
        }
        let val = n.value;
        if val.fract() == 0.0 { format!("{}", val as i64) } else { format!("{}", val) }
    }

    fn emit_template_literal(&mut self, t: &TemplateLiteral<'a>) -> String {
        if t.expressions.is_empty() {
            let raw: String = t.quasis.iter().map(|q| q.value.raw.as_str().to_string()).collect();
            return format!("\"{}\"", escape_cpp_template_raw(&raw));
        }
        let mut fmt_str = String::new();
        let mut args = Vec::new();
        for (i, quasi) in t.quasis.iter().enumerate() {
            let text =
                quasi.value.cooked.as_ref().map(|c| c.as_str()).unwrap_or(quasi.value.raw.as_str());
            fmt_str.push_str(&text.replace('{', "{{").replace('}', "}}"));
            if let Some(expr) = t.expressions.get(i) {
                fmt_str.push_str("{}");
                args.push(self.emit_expression(expr));
            }
        }
        let esc = escape_cpp_text(&fmt_str);
        if args.is_empty() {
            return format!("\"{}\"", esc);
        }
        self.ctx.need("std::format");
        // Formatter for Js* types is provided by js_types.h (which includes
        // js_value_format.h by default), so no need to add js_value_format.h here.
        format!("std::format(\"{}\", {})", esc, args.join(", "))
    }

    fn emit_array(&mut self, arr: &ArrayExpression<'a>) -> String {
        if arr.elements.is_empty() {
            self.ctx.need("JsArray");
            return "JsArray{}".to_string();
        }
        let has_spread =
            arr.elements.iter().any(|e| matches!(e, ArrayExpressionElement::SpreadElement(_)));
        if !has_spread {
            let elems: Vec<String> = arr
                .elements
                .iter()
                .filter_map(|e| match e {
                    ArrayExpressionElement::SpreadElement(s) => {
                        Some(self.emit_expression(&s.argument))
                    }
                    ArrayExpressionElement::Elision(_) => None,
                    _ => e.as_expression().map(|ex| self.emit_expression(ex)),
                })
                .collect();
            self.ctx.need("JsArray");
            return format!("JsArray{{{}}}", elems.join(", "));
        }
        self.ctx.need("JsArray");
        self.ctx.need("int64_t");
        let mut lines = vec!["[&]() {".to_string(), "    JsArray __a{};".to_string()];
        for (idx, el) in arr.elements.iter().enumerate() {
            match el {
                ArrayExpressionElement::SpreadElement(s) => {
                    let mut arg = self.emit_expression(&s.argument);
                    if let Expression::Identifier(id) = &s.argument {
                        if let Some(t) = self.ctx.var_types.get(id.name.as_str()) {
                            if t.starts_with("std::shared_ptr<")
                                || t.starts_with("std::unique_ptr<")
                            {
                                arg = format!("(*{})", arg);
                            }
                        }
                    }
                    lines.push(format!("    JsArray __sp_{} = {};", idx, arg));
                    lines.push(format!("    for (int64_t __i_{} = 0; __i_{} < (int64_t)__sp_{}.length(); ++__i_{})", idx, idx, idx, idx));
                    lines.push(format!("        __a.push(__sp_{}[__i_{}]);", idx, idx));
                }
                ArrayExpressionElement::Elision(_) => {}
                _ => {
                    if let Some(ex) = el.as_expression() {
                        lines.push(format!("    __a.push({});", self.emit_expression(ex)));
                    }
                }
            }
        }
        lines.push("    return __a;".to_string());
        lines.push("}()".to_string());
        lines.join("\n")
    }

    fn emit_object(&mut self, obj: &ObjectExpression<'a>) -> String {
        if obj.properties.is_empty() {
            self.ctx.need("JsObject");
            return "JsObject{}".to_string();
        }
        self.ctx.need("JsObject");
        let runtime_build = obj.properties.iter().any(|prop| match prop {
            ObjectPropertyKind::SpreadProperty(_) => true,
            ObjectPropertyKind::ObjectProperty(p) => p.computed,
        });
        if !runtime_build {
            let mut pairs = Vec::new();
            for prop in &obj.properties {
                let ObjectPropertyKind::ObjectProperty(p) = prop else {
                    continue;
                };
                if let Some(key) = self.property_key_to_string(&p.key) {
                    let val = self.emit_expression(&p.value);
                    pairs.push(format!("{{{}, {}}}", cpp_string_literal(&key), val));
                }
            }
            return format!("JsObject{{{}}}", pairs.join(", "));
        }
        let mut lines = vec!["[&]() {".to_string(), "    JsObject __o{};".to_string()];
        for prop in &obj.properties {
            match prop {
                ObjectPropertyKind::ObjectProperty(p) => {
                    let key = if p.computed {
                        let Some(key_expr) = p.key.as_expression() else {
                            continue;
                        };
                        let key = self.emit_expression(key_expr);
                        format!("morph::js_to_string({key}).to_std_string()")
                    } else if let Some(key) = self.property_key_to_string(&p.key) {
                        cpp_string_literal(&key)
                    } else {
                        continue;
                    };
                    let val = self.emit_expression(&p.value);
                    lines.push(format!("    __o.set({key}, {val});"));
                }
                ObjectPropertyKind::SpreadProperty(s) => {
                    let source = self.emit_expression(&s.argument);
                    lines.push(format!("    morph::js_object_assign(__o, {source});"));
                }
            }
        }
        lines.push("    return __o;".to_string());
        lines.push("}()".to_string());
        lines.join("\n")
    }

    /// `new Window` config object with ownership keys pre-lowered.
    /// `parent`/`modal`/`role` become int-yielding C++ expressions so no
    /// strings reach the runtime; every other key emits normally. Type
    /// violations are C++ `static_assert`s (this file's hard-error idiom —
    /// see `Window.ready`), so the build fails with the message inline.
    fn emit_window_opts(&mut self, obj: &ObjectExpression<'a>) -> String {
        if obj.properties.is_empty() {
            return "JsObject{}".to_string();
        }
        let mut pairs = Vec::new();
        for prop in &obj.properties {
            let ObjectPropertyKind::ObjectProperty(p) = prop else {
                continue;
            };
            let Some(key) = self.property_key_to_string(&p.key) else {
                continue;
            };
            match key.as_str() {
                "parent" => {
                    let lowered = self.lower_parent_value(&p.value);
                    pairs.push(format!("{{\"parent\", {lowered}}}"));
                }
                "modal" => {
                    let lowered = self.lower_modal_value(&p.value);
                    pairs.push(format!("{{\"modal\", {lowered}}}"));
                }
                "role" => {
                    let lowered = Self::lower_role_value(&p.value);
                    pairs.push(format!("{{\"role\", {lowered}}}"));
                }
                _ => {
                    let val = self.emit_expression(&p.value);
                    pairs.push(format!("{{\"{key}\", {val}}}"));
                }
            }
        }
        format!("JsObject{{{}}}", pairs.join(", "))
    }

    /// Lower a `parent` value to an int-yielding C++ expression.
    /// Handles (WID ints) and arbitrary int expressions pass through a
    /// normalizing `JsValue(…).as_int()`; strings resolve by form.
    fn lower_parent_value(&mut self, value: &Expression<'a>) -> String {
        match value {
            Expression::StringLiteral(s) => match s.value.as_str() {
                "" => "kInvalidWid".to_string(),
                "auto" => "WindowManager::get().focusedWid()".to_string(),
                id if id.starts_with('/') => {
                    format!("__morph_route_wid(\"{id}\")")
                }
                id => format!("WindowManager::get().widForAlias(\"{id}\")"),
            },
            Expression::NullLiteral(_) => "kInvalidWid".to_string(),
            other => {
                // Handles (WID ints or JS-visible JsWindowHandle values)
                // narrow through window_wid: JsValue(undefined).as_int()
                // would otherwise silently name WID 0, a live window.
                if let Some(handle) = self.window_handle_expr(other) {
                    return handle;
                }
                format!("JsValue({}).as_int()", self.emit_expression(other))
            }
        }
    }

    /// Lower a `modal` value: bool literals pass through; anything
    /// non-bool is a build error (silent truthiness would lie).
    fn lower_modal_value(&mut self, value: &Expression<'a>) -> String {
        match value {
            Expression::BooleanLiteral(_) => self.emit_expression(value),
            _ => "[] { static_assert(sizeof(\"Window modal\") == 0, \"mx-windowconfig-type: `modal` must be a boolean\"); return false; }()".to_string(),
        }
    }

    /// Lower a `role` literal to its interned int (unknown roles fail the
    /// build — roles are a closed set, so anything else is a typo).
    fn lower_role_value(value: &Expression<'a>) -> String {
        match value {
            Expression::StringLiteral(s) => {
                match morph_config::WindowRole::parse(s.value.as_str()) {
                    Ok(role) => role.as_int().to_string(),
                    Err(_) => "[] { static_assert(sizeof(\"Window role\") == 0, \"mx-windowconfig-type: unknown window role (expected \\\"default\\\", \\\"dialog\\\", or \\\"popup\\\")\"); return 0; }()".to_string(),
                }
            }
            _ => "[] { static_assert(sizeof(\"Window role\") == 0, \"mx-windowconfig-type: `role` must be a string literal\"); return 0; }()".to_string(),
        }
    }

    /// Dereference a `shared_ptr` variable holding a scalar value.
    ///
    /// Escape analysis wraps captured scalars in `shared_ptr`, but use sites
    /// must see the value: `total + x` with `shared_ptr<int64_t> total` does
    /// not compile. Class and container pointees keep pointer semantics.
    fn deref_shared_scalar(&self, name: &str) -> Option<String> {
        let var_type = self.ctx.var_types.get(name)?;
        let inner = var_type.strip_prefix("std::shared_ptr<")?.strip_suffix('>')?;
        if Self::is_scalar_pointee(inner.trim()) { Some(format!("(*{})", name)) } else { None }
    }

    /// True when member access on an emitted object must use `->`.
    ///
    /// Consults both pointer sets and, for deduced holders such as call
    /// results, the recorded `var_types` entry behind a plain identifier.
    fn ptr_arrow_access(&self, obj_node: &Expression<'a>, obj_str: &str) -> bool {
        if self.ctx.shared_ptr_vars.contains(obj_str) || self.ctx.unique_ptr_vars.contains(obj_str)
        {
            return true;
        }
        if let Expression::Identifier(id) = obj_node {
            // Scalar pointees are already dereferenced at the use site by
            // `deref_shared_scalar`; `(*x)->field` would be ill-formed.
            if self.deref_shared_scalar(id.name.as_str()).is_some() {
                return false;
            }
            return self
                .ctx
                .var_types
                .get(id.name.as_str())
                .map(|var_type| {
                    var_type.starts_with("std::shared_ptr<")
                        || var_type.starts_with("std::unique_ptr<")
                })
                .unwrap_or(false);
        }
        false
    }

    /// True for pointees that read and write as plain values.
    fn is_scalar_pointee(inner_type: &str) -> bool {
        matches!(
            inner_type,
            "bool"
                | "char"
                | "float"
                | "double"
                | "std::string"
                | "JsNumber"
                | "JsBoolean"
                | "JsString"
                | "JsValue"
                | "JsWindowHandle"
                | "JsLateValue"
                | "JsNull"
                | "JsUndefined"
                | "WID"
        ) || inner_type.starts_with("int")
            || inner_type.starts_with("uint")
            || inner_type.starts_with("long")
            || inner_type.starts_with("short")
            || inner_type == "size_t"
    }

    /// Heap ownership only pays off for class instances and vectors. Scalars
    /// and `Js*` values copy cheaply, so single-owner escapes stay on stack.
    fn needs_heap_for_escape(cpp_type: &str, class_names: &HashSet<String>) -> bool {
        class_names.contains(cpp_type) || cpp_type.starts_with("std::vector<")
    }

    /// Classify an operand into the C++ value class its emitted code has.
    ///
    /// Mirrors the analyzer-side classification but uses the exact types chosen
    /// during emission, so routing decisions always match the generated code.
    fn operand_class_of(&self, expr: &Expression<'a>) -> OperandClass {
        match expr {
            Expression::ParenthesizedExpression(p) => self.operand_class_of(&p.expression),
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
                    .ctx
                    .var_types
                    .get(name)
                    .map(|cpp_type| cpp_type_to_class(cpp_type))
                    .unwrap_or(OperandClass::Other),
            },
            Expression::ArrayExpression(_) => self
                .infer_type_from_init(expr)
                .map(|cpp_type| cpp_type_to_class(&cpp_type))
                .unwrap_or(OperandClass::JsArray),
            Expression::ObjectExpression(_) => OperandClass::JsObject,
            Expression::NewTarget(_) => OperandClass::JsValue,
            Expression::CallExpression(call) => self.call_result_class(call),
            Expression::AwaitExpression(_)
            | Expression::YieldExpression(_)
            | Expression::TaggedTemplateExpression(_) => OperandClass::JsValue,
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
                } else if binary.operator.as_str() == "**" {
                    OperandClass::Float
                } else {
                    let left_class = self.operand_class_of(&binary.left);
                    let right_class = self.operand_class_of(&binary.right);
                    if left_class == OperandClass::Float || right_class == OperandClass::Float {
                        OperandClass::Float
                    } else if left_class.is_textual() || right_class.is_textual() {
                        OperandClass::Text
                    } else {
                        OperandClass::Integer
                    }
                }
            }
            Expression::LogicalExpression(logical) => {
                let left_class = self.operand_class_of(&logical.left);
                let right_class = self.operand_class_of(&logical.right);
                if left_class == OperandClass::Boolean && right_class == OperandClass::Boolean {
                    OperandClass::Boolean
                } else if left_class == right_class {
                    left_class
                } else {
                    OperandClass::Boolean
                }
            }
            Expression::ConditionalExpression(conditional) => {
                let consequent_class = self.operand_class_of(&conditional.consequent);
                let alternate_class = self.operand_class_of(&conditional.alternate);
                if consequent_class == alternate_class {
                    consequent_class
                } else {
                    OperandClass::Other
                }
            }
            Expression::ComputedMemberExpression(member) => {
                if let Expression::Identifier(object_id) = &member.object {
                    if let Some(object_type) = self.ctx.var_types.get(object_id.name.as_str()) {
                        if object_type == "JsArray" {
                            return OperandClass::JsValue;
                        }
                        if let Some(element_type) = object_type
                            .strip_prefix("std::vector<")
                            .and_then(|rest| rest.strip_suffix('>'))
                        {
                            return cpp_type_to_class(element_type.trim());
                        }
                    }
                }
                OperandClass::JsValue
            }
            Expression::StaticMemberExpression(member) => {
                if let Expression::Identifier(object_id) = &member.object {
                    if object_id.name.as_str() == "Math"
                        && is_math_constant(member.property.name.as_str())
                    {
                        return OperandClass::Float;
                    }
                }
                if member.property.name.as_str() == "length" {
                    OperandClass::Integer
                } else if matches!(member.property.name.as_str(), "status")
                    && Self::is_response_obj(&member.object, &self.ctx.var_types)
                {
                    OperandClass::Integer
                } else if matches!(member.property.name.as_str(), "statusText")
                    && Self::is_response_obj(&member.object, &self.ctx.var_types)
                {
                    OperandClass::Text
                } else {
                    OperandClass::Other
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.operand_class_of(&parenthesized.expression)
            }
            Expression::TSAsExpression(cast) => self.operand_class_of(&cast.expression),
            Expression::TSSatisfiesExpression(cast) => self.operand_class_of(&cast.expression),
            Expression::TSNonNullExpression(cast) => self.operand_class_of(&cast.expression),
            Expression::TSTypeAssertion(cast) => self.operand_class_of(&cast.expression),
            _ => OperandClass::Other,
        }
    }

    /// Class identifier operand in a comparison lowers to its constructor
    /// value (`new.target === Foo`). Everywhere else class names keep their
    /// native spelling (`new Foo()`, `Foo.bar`).
    fn comparison_operand_class(&self, expr: &Expression<'a>) -> OperandClass {
        match Self::unwrap_expr(expr) {
            Expression::NewTarget(_) => OperandClass::JsValue,
            Expression::Identifier(id)
                if self.ctx.class_names.contains(id.name.as_str())
                    && !self.ctx.var_types.contains_key(id.name.as_str()) =>
            {
                OperandClass::JsValue
            }
            other => self.operand_class_of(other),
        }
    }

    fn emit_comparison_operand(&mut self, expr: &Expression<'a>) -> String {
        if let Expression::Identifier(id) = Self::unwrap_expr(expr)
            && self.ctx.class_names.contains(id.name.as_str())
            && !self.ctx.var_types.contains_key(id.name.as_str())
        {
            self.ctx.need("JsValue");
            let name = id.name.as_str();
            return format!("morph::js_constructor_value<{name}>(\"{name}\")");
        }
        self.emit_expression(expr)
    }

    fn is_response_obj(
        object: &Expression<'a>,
        var_types: &std::collections::HashMap<String, String>,
    ) -> bool {
        if let Expression::Identifier(id) = object {
            return var_types
                .get(id.name.as_str())
                .map(|t| t.contains("Response"))
                .unwrap_or(false);
        }
        false
    }

    /// True when `expr` evaluates to a fetch `Response`: a binding declared
    /// from `await fetch(...)`, an inline `fetch(...)`, or an awaited one.
    /// Used to type body readers (`.json()`, `.text()`) on call results.
    fn is_response_like_expr(&self, expr: &Expression<'a>) -> bool {
        match expr {
            Expression::ParenthesizedExpression(p) => self.is_response_like_expr(&p.expression),
            Expression::Identifier(id) => {
                self.ctx.var_types.get(id.name.as_str()).is_some_and(|t| t.contains("Response"))
            }
            Expression::CallExpression(call) => {
                matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "fetch")
            }
            Expression::AwaitExpression(a) => self.is_response_like_expr(&a.argument),
            _ => false,
        }
    }

    /// Bare class name behind a pointer/reference spelling.
    fn bare_pointer_type(cpp_type: &str) -> &str {
        let trimmed = cpp_type.trim();
        for wrap in ["std::shared_ptr<", "std::unique_ptr<"] {
            if let Some(inner) = trimmed.strip_prefix(wrap).and_then(|s| s.strip_suffix('>')) {
                return inner.trim();
            }
        }
        trimmed.trim_end_matches('*').trim()
    }

    /// True when `obj` should lower a general Array.prototype method to the
    /// runtime helpers. Known non-array receivers (classes, strings,
    /// numbers, objects) keep their own member/method emission; everything
    /// array-shaped or dynamic goes through the helpers.
    fn is_array_method_receiver(&self, obj: &Expression<'a>) -> bool {
        if matches!(obj, Expression::ArrayExpression(_)) {
            return true;
        }
        if let Expression::Identifier(id) = obj {
            let Some(ty) = self.ctx.var_types.get(id.name.as_str()) else {
                return true;
            };
            let bare = Self::bare_pointer_type(ty);
            if self.ctx.class_names.contains(bare) {
                return false;
            }
            if matches!(
                bare,
                "JsObject"
                    | "JsString"
                    | "JsNumber"
                    | "JsBoolean"
                    | "JsUndefined"
                    | "JsNull"
                    | "bool"
                    | "char"
                    | "int"
                    | "int32_t"
                    | "int64_t"
                    | "double"
                    | "float"
                    | "size_t"
                    | "uint32_t"
                    | "uint64_t"
                    | "int16_t"
                    | "uint16_t"
                    | "int8_t"
                    | "uint8_t"
            ) || bare.starts_with("std::string")
            {
                return false;
            }
            return true;
        }
        !matches!(
            self.operand_class_of(obj),
            OperandClass::Text
                | OperandClass::Boolean
                | OperandClass::Integer
                | OperandClass::Float
                | OperandClass::Null
                | OperandClass::Undefined
                | OperandClass::JsString
                | OperandClass::JsNumber
                | OperandClass::JsBoolean
                | OperandClass::JsObject
        )
    }

    /// Classify a call result: known string/number/boolean methods keep their
    /// native class, everything else is treated as dynamic.
    fn call_result_class(&self, call: &CallExpression<'a>) -> OperandClass {
        if let Expression::StaticMemberExpression(member) = &call.callee {
            let method_name = member.property.name.as_str();
            if JS_ARRAY_METHODS.contains(&method_name)
                && self.is_array_method_receiver(&member.object)
            {
                return match method_name {
                    "map" | "filter" | "splice" | "fill" | "flat" | "flatMap" | "sort"
                    | "reverse" => OperandClass::JsArray,
                    "findIndex" | "push" | "unshift" => OperandClass::Integer,
                    "some" | "every" => OperandClass::Boolean,
                    _ => OperandClass::JsValue,
                };
            }
            if matches!(method_name, "indexOf" | "lastIndexOf" | "search" | "localeCompare") {
                return OperandClass::Integer;
            }
            if matches!(
                method_name,
                "includes" | "startsWith" | "endsWith" | "has" | "isArray" | "isFinite" | "isNaN"
            ) {
                return OperandClass::Boolean;
            }
            if StringMethodHandler::is_string_method(method_name) {
                return OperandClass::Text;
            }
            // Response.text() resolves the body string (sync in morph's API).
            if method_name == "text" {
                if let Expression::StaticMemberExpression(member) = &call.callee {
                    if Self::is_response_obj(&member.object, &self.ctx.var_types) {
                        return OperandClass::Text;
                    }
                }
            }
        }
        OperandClass::JsValue
    }

    fn record_comparison_use(
        &mut self,
        kind: ComparisonKind,
        left: OperandClass,
        right: OperandClass,
    ) {
        self.comparison_uses.insert(ComparisonSignature::binary(kind, left, right));
    }

    /// True when a truthiness test needs an explicit `is_truthy` wrapper.
    ///
    /// Booleans flow through untouched, `JsBoolean`/`JsValue` already convert
    /// to `bool` with JavaScript semantics, and unknown types keep today's
    /// direct emission.
    fn needs_truthy_wrapper(operand_class: OperandClass) -> bool {
        matches!(
            operand_class,
            OperandClass::Integer
                | OperandClass::Float
                | OperandClass::Text
                | OperandClass::Null
                | OperandClass::Undefined
                | OperandClass::JsNumber
                | OperandClass::JsString
                | OperandClass::JsArray
                | OperandClass::JsObject
        )
    }

    /// Emit a truthiness test, wrapping with `is_truthy` only when the static
    /// operand class needs JavaScript semantics.
    fn emit_truthy_test(&mut self, expr: &Expression<'a>) -> String {
        let operand_class = self.operand_class_of(expr);
        let emitted = self.emit_expression(expr);
        if Self::needs_truthy_wrapper(operand_class) {
            self.comparison_uses.insert(ComparisonSignature::truthy(operand_class));
            format!("morph::js_cmp::is_truthy({})", emitted)
        } else {
            emitted
        }
    }

    fn is_raw_string_operand(expr: &Expression) -> bool {
        match expr {
            Expression::StringLiteral(_) => true,
            Expression::TemplateLiteral(literal) => literal.expressions.is_empty(),
            _ => false,
        }
    }

    /// Decide whether a comparison must go through `morph::js_cmp`.
    ///
    /// Same-class native pairs already match JavaScript semantics, so they
    /// keep direct operators. Everything else routes to the helpers.
    fn should_use_js_comparison(
        kind: ComparisonKind,
        left_expr: &Expression<'a>,
        right_expr: &Expression<'a>,
        left_class: OperandClass,
        right_class: OperandClass,
    ) -> bool {
        if left_class == OperandClass::Other
            || right_class == OperandClass::Other
            || left_class == OperandClass::Vector
            || right_class == OperandClass::Vector
        {
            return false;
        }
        if left_class == right_class {
            match left_class {
                OperandClass::Boolean | OperandClass::Integer | OperandClass::Float => {
                    return false;
                }
                OperandClass::Text => {
                    return Self::is_raw_string_operand(left_expr)
                        && Self::is_raw_string_operand(right_expr);
                }
                OperandClass::JsBoolean
                | OperandClass::JsNumber
                | OperandClass::JsString
                | OperandClass::Null
                | OperandClass::Undefined => {
                    return false;
                }
                _ => {}
            }
        }
        let text_js_string_pair = matches!(left_class, OperandClass::Text)
            && matches!(right_class, OperandClass::JsString)
            || matches!(left_class, OperandClass::JsString)
                && matches!(right_class, OperandClass::Text);
        if text_js_string_pair {
            if kind.is_loose()
                && !Self::is_raw_string_operand(left_expr)
                && !Self::is_raw_string_operand(right_expr)
            {
                return false;
            }
            return true;
        }
        true
    }

    /// True for operands that can be evaluated twice without changing meaning.
    fn is_pure_operand(expr: &Expression) -> bool {
        match expr {
            Expression::Identifier(_)
            | Expression::NumericLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::BigIntLiteral(_) => true,
            Expression::ParenthesizedExpression(parenthesized) => {
                Self::is_pure_operand(&parenthesized.expression)
            }
            Expression::TemplateLiteral(literal) => literal.expressions.is_empty(),
            _ => false,
        }
    }

    /// Emit `&&` / `||` with JavaScript value semantics.
    ///
    /// Boolean pairs keep direct operators. Same-class pure operands use a
    /// value-preserving ternary (short-circuit included). Anything else falls
    /// back to a boolean combination, which is documented in the plan.
    fn emit_logical(&mut self, b: &LogicalExpression<'a>, operator: &str) -> String {
        let left_class = self.operand_class_of(&b.left);
        let right_class = self.operand_class_of(&b.right);
        let left = self.emit_expression(&b.left);
        let right = self.emit_expression(&b.right);
        if left_class == OperandClass::Boolean && right_class == OperandClass::Boolean {
            return format!("{} {} {}", left, operator, right);
        }
        if Self::needs_truthy_wrapper(left_class) {
            self.comparison_uses.insert(ComparisonSignature::truthy(left_class));
        }
        if Self::needs_truthy_wrapper(right_class) {
            self.comparison_uses.insert(ComparisonSignature::truthy(right_class));
        }
        let same_class = left_class == right_class
            && left_class != OperandClass::Other
            && left_class != OperandClass::Vector;
        if same_class && Self::is_pure_operand(&b.left) && Self::is_pure_operand(&b.right) {
            if operator == "&&" {
                return format!("(morph::js_cmp::is_truthy({}) ? ({}) : ({}))", left, right, left);
            }
            return format!("(morph::js_cmp::is_truthy({}) ? ({}) : ({}))", left, left, right);
        }
        if operator == "&&" {
            let left_test = Self::wrap_logical_operand(left, left_class);
            let right_test = Self::wrap_logical_operand(right, right_class);
            return format!("({} && {})", left_test, right_test);
        }
        let left_test = Self::wrap_logical_operand(left, left_class);
        let right_test = Self::wrap_logical_operand(right, right_class);
        format!("({} || {})", left_test, right_test)
    }

    /// Wrap one `&&` / `||` operand, leaving already-boolean sides untouched.
    fn wrap_logical_operand(emitted: String, operand_class: OperandClass) -> String {
        if Self::needs_truthy_wrapper(operand_class) {
            format!("morph::js_cmp::is_truthy({})", emitted)
        } else {
            emitted
        }
    }

    /// Wrap a non-string `+` operand for JsString concatenation:
    /// natives via morph::str, JsNumber via its string constructor.
    fn string_concat_operand(&self, emitted: String, expr: &Expression<'a>) -> String {
        match self.operand_class_of(expr) {
            OperandClass::Integer | OperandClass::Float | OperandClass::Boolean => {
                format!("JsString(morph::str({}))", emitted)
            }
            OperandClass::JsNumber => format!("JsString({})", emitted),
            _ => emitted,
        }
    }

    fn emit_binary(&mut self, b: &BinaryExpression<'a>) -> String {
        let op = b.operator.as_str();

        // Exponentiation coerces both sides with ToNumber and always yields
        // a JS number, so it must run before the integer narrowing below.
        if op == "**" {
            let left = self.emit_expression(&b.left);
            let right = self.emit_expression(&b.right);
            return self.emit_pow(&left, &right);
        }

        // Handle special operators that don't want conversion
        if op == "instanceof" {
            let left = self.emit_expression(&b.left);
            let right = self.emit_expression(&b.right);
            self.ctx.need("JsValue");
            return format!("morph::js_instanceof({}, {})", left, right);
        }
        if op == "in" {
            let left = self.emit_expression(&b.left);
            let right = self.emit_expression(&b.right);
            self.ctx.need("JsObject");
            return format!("morph::js_has_property({}, {})", right, left);
        }

        if let Some(comparison_kind) = ComparisonKind::from_binary_operator(op) {
            let left_class = self.comparison_operand_class(&b.left);
            let right_class = self.comparison_operand_class(&b.right);
            if Self::should_use_js_comparison(
                comparison_kind,
                &b.left,
                &b.right,
                left_class,
                right_class,
            ) {
                let left = self.emit_comparison_operand(&b.left);
                let right = self.emit_comparison_operand(&b.right);
                self.record_comparison_use(comparison_kind, left_class, right_class);
                return format!(
                    "morph::js_cmp::{}({}, {})",
                    comparison_kind.dispatcher_name(),
                    left,
                    right
                );
            }
        }

        // Late-assigned values keep JS operator semantics: `undefined + 1`
        // is NaN, strings concatenate, and objects coerce through ToNumber
        // instead of requiring the numeric variant of the union.
        let late_value = self.is_late_value_expr(&b.left) || self.is_late_value_expr(&b.right);
        // Check if we need to convert JsValue to number for arithmetic
        let needs_left_convert =
            !late_value && (self.is_jsarray_index(&b.left) || self.is_jsvalue_var(&b.left));
        let needs_right_convert =
            !late_value && (self.is_jsarray_index(&b.right) || self.is_jsvalue_var(&b.right));

        let mut left = self.emit_expression(&b.left);
        let mut right = self.emit_expression(&b.right);

        if needs_left_convert {
            left = format!("std::get<JsNumber>({}.inner).as_int()", left);
        }
        if needs_right_convert {
            right = format!("std::get<JsNumber>({}.inner).as_int()", right);
        }

        let is_left_str = matches!(&b.left, Expression::StringLiteral(_));
        let is_right_str = matches!(&b.right, Expression::StringLiteral(_));
        if is_left_str {
            left = format!("JsString({})", left);
        }
        if is_right_str {
            right = format!("JsString({})", right);
        }
        // `"text" + number` (e.g. `"HTTP error " + r.status`): convert the
        // native side so JsString operator+ resolves. Textual other sides
        // keep today's direct emission.
        if op == "+" {
            if is_left_str && !is_right_str {
                right = self.string_concat_operand(right, &b.right);
            } else if is_right_str && !is_left_str {
                left = self.string_concat_operand(left, &b.left);
            }
        }

        // Late-assigned values coerce at runtime: `+` concatenates when
        // either side is a string, other arithmetic uses ToNumber.
        if late_value {
            let helper = match op {
                "+" => Some("js_late_add"),
                "-" => Some("js_late_sub"),
                "*" => Some("js_late_mul"),
                "/" => Some("js_late_div"),
                "%" => Some("js_late_mod"),
                _ => None,
            };
            if let Some(helper) = helper {
                self.ctx.need("JsValue");
                return format!("morph::{helper}({left}, {right})");
            }
        }

        // For division, use double to match JS semantics (float division).
        // Late values carry that semantics in their runtime operators.
        if op == "/" && !late_value {
            // Check if operands are integer types that need float promotion
            let left_is_int = self.is_integer_type(&b.left);
            let right_is_int = self.is_integer_type(&b.right);
            if left_is_int || right_is_int {
                left = format!("static_cast<double>({})", left);
                right = format!("static_cast<double>({})", right);
            }
        }

        let op_mapped = match op {
            "===" => "==",
            "!==" => "!=",
            ">>>" => "/* >>> */",
            _ => op,
        };
        if op == "??" {
            return format!(
                "(JsValue({}).is_undefined() || JsValue({}).is_null() ? JsValue({}) : JsValue({}))",
                left, left, right, left
            );
        }
        format!("{} {} {}", left, op_mapped, right)
    }

    fn is_jsvalue_var(&self, expr: &Expression<'a>) -> bool {
        // Check if expression is a variable that holds a JsValue (e.g., from array element)
        if let Expression::Identifier(id) = expr {
            if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                return var_type == "JsValue";
            }
        }
        // Computed member on JsArray/JsObject returns JsValue (needs int conversion in numeric context)
        if let Expression::ComputedMemberExpression(m) = expr {
            if let Expression::Identifier(id) = &m.object {
                if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                    return var_type == "JsArray"
                        || var_type == "JsObject"
                        || var_type == "JsValue";
                }
            }
        }
        false
    }

    /// True when the expression reads a late-assigned JS value binding.
    /// Those use `JsLateValue` so every operator keeps JS semantics.
    fn is_late_value_expr(&self, expr: &Expression<'a>) -> bool {
        if let Expression::Identifier(id) = expr {
            if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                return var_type.contains("JsLateValue");
            }
        }
        false
    }

    fn is_integer_type(&self, expr: &Expression<'a>) -> bool {
        // Check if expression is a numeric literal or variable with integer type
        match expr {
            Expression::NumericLiteral(n) => n.value.fract() == 0.0,
            Expression::Identifier(id) => {
                if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                    matches!(var_type.as_str(), "int32_t" | "int64_t" | "int" | "long" | "short")
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn is_jsarray_index(&self, expr: &Expression<'a>) -> bool {
        // Check if expression is a computed member access on a JsArray variable
        if let Expression::ComputedMemberExpression(m) = expr {
            if let Expression::Identifier(id) = &m.object {
                if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                    return var_type == "JsArray";
                }
            }
        }
        false
    }

    /// Decide whether a string-method call should use morph::strutil:: helpers (native)
    /// Returns Some(true) = use helper, Some(false) = use direct Js call, None = not applicable
    fn str_helper_decision(&self, recv: &Expression<'a>) -> Option<bool> {
        match recv {
            Expression::Identifier(id) => {
                let var_type =
                    self.ctx.var_types.get(id.name.as_str()).cloned().unwrap_or_default();
                // Class instances have their own toString() - don't intercept
                if var_type.starts_with("std::shared_ptr<")
                    || self.ctx.class_names.contains(var_type.as_str())
                {
                    return None;
                }
                if var_type == "JsString" || var_type == "JsValue" || var_type == "JsLateValue" {
                    return Some(false);
                }
                if var_type == "JsArray" || var_type == "JsObject" {
                    return Some(false);
                }
                if matches!(
                    var_type.as_str(),
                    "int32_t"
                        | "int64_t"
                        | "int"
                        | "i32"
                        | "i64"
                        | "double"
                        | "float"
                        | "f64"
                        | "f32"
                ) {
                    return Some(true);
                }
                if var_type == "std::string"
                    || var_type.starts_with("std::string")
                    || var_type == "auto"
                    || var_type.is_empty()
                {
                    return Some(true);
                }
                // Unknown native-like types default to helper in infer mode
                if matches!(self.type_mode, TypeMode::Infer) {
                    return Some(true);
                }
                None
            }
            Expression::CallExpression(inner) => {
                // Chaining: receiver is result of another call.
                // If inner is a string helper (returns std::string / vector<string>) -> keep native chain.
                if let Expression::StaticMemberExpression(inner_m) = &inner.callee {
                    let inner_method = inner_m.property.name.as_str();
                    if StringMethodHandler::is_string_method(inner_method) {
                        // Recurse to base to decide domain
                        if let Some(inner_decision) = self.str_helper_decision(&inner_m.object) {
                            return Some(inner_decision);
                        }
                        // If inner base unknown but inner was emitted as helper, stay native
                        return Some(true);
                    }
                }
                // Unknown call (e.g. greet("Bob") returning JsString) -> direct
                None
            }
            Expression::ComputedMemberExpression(m) => {
                // arr[0], obj["name"], split(...)[1]
                match &m.object {
                    Expression::Identifier(base_id) => {
                        let bt = self
                            .ctx
                            .var_types
                            .get(base_id.name.as_str())
                            .cloned()
                            .unwrap_or_default();
                        if bt.starts_with("std::vector<std::string") || bt == "std::string" {
                            return Some(true);
                        }
                        if bt.starts_with("std::vector<") {
                            // vector<JsValue> or other -> element is JsValue -> direct (JsValue has methods)
                            return Some(false);
                        }
                        if matches!(bt.as_str(), "JsArray" | "JsObject" | "JsValue" | "JsLateValue")
                        {
                            return Some(false);
                        }
                        // Unknown base: if base name looks like split result? default native for chaining
                        return Some(true);
                    }
                    Expression::CallExpression(_) => {
                        // e.g. split(text,",")[1] -> split returns vector<string> -> element std::string
                        return Some(true);
                    }
                    _ => return Some(true),
                }
            }
            Expression::ParenthesizedExpression(p) => self.str_helper_decision(&p.expression),
            Expression::StringLiteral(_) => Some(true),
            Expression::NumericLiteral(_) => Some(true),
            Expression::TemplateLiteral(_) => Some(true),
            _ => None,
        }
    }

    /// True when `target = source` can move: both sides share one recorded
    /// type (so the move-assign compiles) and the source dies here.
    fn last_use_assignable(&self, target: &str, source: &IdentifierReference<'a>) -> bool {
        let target_type = self.ctx.var_types.get(target).cloned().unwrap_or_default();
        let source_type = self.ctx.var_types.get(source.name.as_str()).cloned().unwrap_or_default();
        if target_type.is_empty() || target_type != source_type {
            return false;
        }
        self.move_eligible_at(source.name.as_str(), source.span.start)
    }

    /// `Some("__st_x.set")` when `name` maps to a signal read
    /// (`__st_x.get()` / `__st_x.peek()` / `ctx->x.get()`); reactive-const
    /// thunks (`name()`) and plain values keep their default lowering.
    fn signal_set_head(&self, name: &str) -> Option<String> {
        let mapped = self.ctx.state_vars.get(name)?;
        let head = mapped.strip_suffix(".get()").or_else(|| mapped.strip_suffix(".peek()"))?;
        Some(format!("{head}.set"))
    }

    fn emit_assignment(&mut self, a: &AssignmentExpression<'a>) -> String {
        if matches!(
            &a.left,
            AssignmentTarget::ArrayAssignmentTarget(_)
                | AssignmentTarget::ObjectAssignmentTarget(_)
        ) {
            return self.emit_destructuring_assignment(a);
        }
        // A plain assignment to a signal-backed binding — `x = ...` where
        // `x` maps to `__st_x.get()` in the ambient state map — lowers to
        // its setter (`__st_x.set(...)`); compound forms read back through
        // the getter (`x += 1` → `set(get() + 1)`). Body-level flow and
        // inner closures write mutable locals this way.
        if let AssignmentTarget::AssignmentTargetIdentifier(left_id) = &a.left
            && let Some(set_head) = self.signal_set_head(left_id.name.as_str())
        {
            let mut right = self.emit_expression(&a.right);
            let op = a.operator.as_str();
            if op == "=" {
                if let Some(left_type) = self.ctx.var_types.get(left_id.name.as_str()).cloned() {
                    right = self.convert_number_value(&left_type, &a.right, right);
                }
                return format!("{set_head}({right})");
            }
            if let Some(bin) = compound_assign_binop(op) {
                let read =
                    self.ctx.state_vars.get(left_id.name.as_str()).cloned().unwrap_or_default();
                let late_signal = self
                    .ctx
                    .var_types
                    .get(left_id.name.as_str())
                    .is_some_and(|ty| ty.contains("JsLateValue"));
                if late_signal {
                    let helper = match op {
                        "+=" => Some("js_late_add"),
                        "-=" => Some("js_late_sub"),
                        "*=" => Some("js_late_mul"),
                        "/=" => Some("js_late_div"),
                        "%=" => Some("js_late_mod"),
                        _ => None,
                    };
                    if let Some(helper) = helper {
                        return format!("{set_head}(morph::{helper}({read}, {right}))");
                    }
                }
                return format!("{set_head}(({read}) {bin} ({right}))");
            }
            // Exponentiation goes through ToNumber like JS; logical
            // assignments keep the read on both sides so the right side
            // only evaluates when the condition selects it.
            if op == "**=" {
                let read =
                    self.ctx.state_vars.get(left_id.name.as_str()).cloned().unwrap_or_default();
                let value = self.emit_pow(&format!("({read})"), &right);
                return format!("{set_head}({value})");
            }
            if matches!(op, "&&=" | "||=" | "??=") {
                let read =
                    self.ctx.state_vars.get(left_id.name.as_str()).cloned().unwrap_or_default();
                let late_signal = self
                    .ctx
                    .var_types
                    .get(left_id.name.as_str())
                    .is_some_and(|ty| ty.contains("JsLateValue"));
                let value = self.emit_logical_assign_value(op, &read, &right, late_signal);
                return format!("{set_head}({value})");
            }
            // `??=` / `&&=` / `||=` keep their default lowering.
        }
        // Getter/setter properties read and write through their accessor
        // methods, not as data members.
        if let Some(rendered) = self.emit_accessor_assignment(a) {
            return rendered;
        }
        // `win.title = x` on any Window-handle receiver lowers to a
        // registry write. Handles may be tracked locals, `Window`-typed
        // bindings, lazy component-body placeholders, or direct
        // `useWindow(...)` / `new Window(...)` expressions.
        if a.operator.as_str() == "=" {
            if let AssignmentTarget::StaticMemberExpression(mem) = &a.left {
                if mem.property.name.as_str() == "title" {
                    if let Some(obj) = self.window_handle_expr(&mem.object) {
                        let rhs = self.emit_expression(&a.right);
                        return format!("__morph_win_set_title({obj}, {rhs})");
                    }
                }
            }
        }
        if let AssignmentTarget::AssignmentTargetIdentifier(left_id) = &a.left {
            let var_name = left_id.name.to_string();
            if a.operator.as_str() == "=" {
                if let Some((narrowed_name, narrowed_type)) =
                    self.narrow_index.get(&(var_name.clone(), a.span.start)).cloned()
                {
                    let rhs = self.emit_expression(&a.right);
                    self.active_narrows.insert(var_name, narrowed_name.clone());
                    self.ctx.var_types.insert(narrowed_name.clone(), narrowed_type.clone());
                    self.ctx.needed.insert("<cstdint>".to_string());
                    let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
                    return format!(
                        "{}{}{} {} = {}",
                        self.indent(),
                        prefix,
                        narrowed_type,
                        narrowed_name,
                        rhs
                    );
                }
                self.active_narrows.remove(&var_name);
            }
        }
        if a.operator.as_str() == "=" {
            if let AssignmentTarget::AssignmentTargetIdentifier(left_id) = &a.left {
                if let Expression::Identifier(right_id) = &a.right {
                    if !self.active_narrows.contains_key(right_id.name.as_str())
                        && self.deref_shared_scalar(left_id.name.as_str()).is_none()
                        && self.last_use_assignable(left_id.name.as_str(), right_id)
                    {
                        let left = self.emit_assignment_target(&a.left);
                        return format!("({} = std::move({}))", left, right_id.name);
                    }
                }
            }
        }
        // Exponentiation and logical assignment keep JS value semantics:
        // the right side is only evaluated when the condition selects it.
        if matches!(
            a.operator,
            AssignmentOperator::Exponential
                | AssignmentOperator::LogicalAnd
                | AssignmentOperator::LogicalOr
                | AssignmentOperator::LogicalNullish
        ) {
            let left = self.emit_assignment_target(&a.left);
            let right = self.emit_expression(&a.right);
            if a.operator == AssignmentOperator::Exponential {
                let value = self.emit_pow(&left, &right);
                return format!("({left} = {value})");
            }
            // A late binding holds a JsValue, so both ternary branches must
            // stay JsValue even when the other side is a native literal
            // (`let y = null; y ||= "set"`).
            let late_target = matches!(
                &a.left,
                AssignmentTarget::AssignmentTargetIdentifier(id)
                    if self
                        .ctx
                        .var_types
                        .get(id.name.as_str())
                        .is_some_and(|ty| ty.contains("JsLateValue"))
            );
            let value =
                self.emit_logical_assign_value(a.operator.as_str(), &left, &right, late_target);
            return format!("({left} = {value})");
        }
        let left = self.emit_assignment_target(&a.left);
        let right = self.emit_expression(&a.right);
        // Check if we need to convert JsValue from array element to int.
        // A `this.field = <JsValue>` assignment is left alone: class fields
        // are Js* runtime types and their operator= accepts JsValue, while
        // coercing to int makes the assignment ambiguous (notably for a
        // destructured constructor binding).
        let assign_this_field = matches!(
            &a.left,
            AssignmentTarget::StaticMemberExpression(mem)
                if matches!(&mem.object, Expression::ThisExpression(_))
        );
        // Assigning a JS value to a late binding stays a value copy; the
        // numeric unwrap is only for native numeric targets.
        let assign_late_value = matches!(
            &a.left,
            AssignmentTarget::AssignmentTargetIdentifier(id)
                if self
                    .ctx
                    .var_types
                    .get(id.name.as_str())
                    .is_some_and(|ty| ty.contains("JsLateValue"))
        );
        let right = if self.is_jsvalue_var(&a.right) && !assign_this_field && !assign_late_value {
            format!("std::get<JsNumber>({}.inner).as_int()", right)
        } else if let AssignmentTarget::AssignmentTargetIdentifier(left_id) = &a.left {
            match self.ctx.var_types.get(left_id.name.as_str()).cloned() {
                Some(left_type) => self.convert_number_value(&left_type, &a.right, right),
                None => right,
            }
        } else {
            right
        };
        let op = match a.operator.as_str() {
            "&&=" => "/* &&= */",
            "||=" => "/* ||= */",
            "??=" => "/* ??= */",
            x => x,
        };
        format!("({} {} {})", left, op, right)
    }

    /// Lower an assignment whose target is an accessor property. Returns
    /// None for plain fields, unknown receivers, or setter-less accessors.
    ///
    /// The receiver is evaluated once into a lambda local so compound and
    /// update forms never call it twice; setters return void, so the lambda
    /// returns the assigned value like a JS assignment expression.
    fn emit_accessor_assignment(&mut self, a: &AssignmentExpression<'a>) -> Option<String> {
        let AssignmentTarget::StaticMemberExpression(member) = &a.left else {
            return None;
        };
        let prop = member.property.name.to_string();
        let (class, is_static) = self.accessor_receiver_class(&member.object)?;
        let (getter, setter) = self.accessor_halves(&class, &prop, is_static)?;
        if !setter {
            return None;
        }
        let obj_code = self.emit_expression(&member.object);
        let sep = self.accessor_member_sep(&member.object, &obj_code, is_static);
        let bind_receiver =
            if is_static { String::new() } else { format!("auto&& __morph_recv = ({obj_code}); ") };
        let read = if is_static {
            format!("{obj_code}{sep}{prop}()")
        } else {
            format!("__morph_recv{sep}{prop}()")
        };
        let set_call = |value: &str| {
            if is_static {
                format!("{obj_code}{sep}{prop}({value});")
            } else {
                format!("__morph_recv{sep}{prop}({value});")
            }
        };
        let op = a.operator.as_str();
        if op == "=" {
            let rhs = self.emit_expression(&a.right);
            let set = set_call("__morph_value");
            return Some(format!(
                "([&]() {{ {bind_receiver}auto __morph_value = ({rhs}); {set} return __morph_value; }})()"
            ));
        }
        if !getter {
            return None;
        }
        let rhs = self.emit_expression(&a.right);
        if matches!(op, "&&=" | "||=" | "??=") {
            // Both helpers live in js_convert.h.
            self.ctx.need("JsValue");
            let test = match op {
                "&&=" => "morph::js_to_boolean(__morph_value)".to_string(),
                "||=" => "!morph::js_to_boolean(__morph_value)".to_string(),
                _ => "morph::js_is_nullish(__morph_value)".to_string(),
            };
            let set = set_call("__morph_value");
            return Some(format!(
                "([&]() {{ {bind_receiver}auto __morph_value = ({read}); \
                 if ({test}) {{ __morph_value = ({rhs}); {set} }} return __morph_value; }})()"
            ));
        }
        let value = if op == "**=" {
            self.emit_pow(&read, &rhs)
        } else if let Some(bin) = compound_assign_binop(op) {
            format!("({read}) {bin} ({rhs})")
        } else {
            return None;
        };
        let set = set_call("__morph_value");
        Some(format!(
            "([&]() {{ {bind_receiver}auto __morph_value = {value}; {set} return __morph_value; }})()"
        ))
    }

    /// Lower `a.value++` / `--a.value` through the getter and setter,
    /// preserving the prefix/postfix result value.
    fn emit_accessor_update(&mut self, u: &UpdateExpression<'a>) -> Option<String> {
        let SimpleAssignmentTarget::StaticMemberExpression(member) = &u.argument else {
            return None;
        };
        let prop = member.property.name.to_string();
        let (class, is_static) = self.accessor_receiver_class(&member.object)?;
        let (getter, setter) = self.accessor_halves(&class, &prop, is_static)?;
        if !getter || !setter {
            return None;
        }
        let obj_code = self.emit_expression(&member.object);
        let sep = self.accessor_member_sep(&member.object, &obj_code, is_static);
        let bind_receiver =
            if is_static { String::new() } else { format!("auto&& __morph_recv = ({obj_code}); ") };
        let read = if is_static {
            format!("{obj_code}{sep}{prop}()")
        } else {
            format!("__morph_recv{sep}{prop}()")
        };
        let set_call = |value: &str| {
            if is_static {
                format!("{obj_code}{sep}{prop}({value});")
            } else {
                format!("__morph_recv{sep}{prop}({value});")
            }
        };
        let bin = if u.operator.as_str() == "++" { "+" } else { "-" };
        if u.prefix {
            let set = set_call("__morph_next");
            Some(format!(
                "([&]() {{ {bind_receiver}auto __morph_next = ({read}) {bin} 1; {set} return __morph_next; }})()"
            ))
        } else {
            let set = set_call(&format!("(__morph_old {bin} 1)"));
            Some(format!(
                "([&]() {{ {bind_receiver}auto __morph_old = ({read}); {set} return __morph_old; }})()"
            ))
        }
    }

    /// Emit JS exponentiation (`a ** b`): both operands go through ToNumber,
    /// matching `2 ** "3"` and `(-1) ** 0.5` (NaN).
    fn emit_pow(&mut self, left: &str, right: &str) -> String {
        self.ctx.need("JsValue");
        format!(
            "std::pow(morph::js_to_number({left}).as_double(), \
             morph::js_to_number({right}).as_double())"
        )
    }

    /// Emit the value selected by a logical assignment.
    ///
    /// The read appears in both the condition and the fallback, matching JS
    /// short-circuiting: `a &&= b` writes `b` only when `a` is truthy.
    fn emit_logical_assign_value(
        &mut self,
        op: &str,
        read: &str,
        right: &str,
        wrap_js_value: bool,
    ) -> String {
        // Both helpers live in js_convert.h; a native-only app still needs
        // the include even when it uses no `Js*` type.
        self.ctx.need("JsValue");
        let read_value = if wrap_js_value { format!("JsValue({read})") } else { read.to_string() };
        let right_value =
            if wrap_js_value { format!("JsValue({right})") } else { right.to_string() };
        if op == "??=" {
            return format!("(morph::js_is_nullish({read}) ? ({right_value}) : ({read_value}))");
        }
        let test = format!("morph::js_to_boolean({read})");
        if op == "&&=" {
            format!("({test} ? ({right_value}) : ({read_value}))")
        } else {
            format!("({test} ? ({read_value}) : ({right_value}))")
        }
    }

    fn emit_assignment_target(&mut self, target: &AssignmentTarget<'a>) -> String {
        match target {
            AssignmentTarget::AssignmentTargetIdentifier(id) => {
                if let Some(narrowed) = self.active_narrows.get(id.name.as_str()).cloned() {
                    return narrowed;
                }
                if let Some(dereferenced) = self.deref_shared_scalar(id.name.as_str()) {
                    return dereferenced;
                }
                id.name.to_string()
            }
            AssignmentTarget::ComputedMemberExpression(m) => {
                let obj = self.emit_expression(&m.object);
                let prop = self.emit_expression(&m.expression);
                // check shared_ptr
                if self.ptr_arrow_access(&m.object, &obj) {
                    format!("{}->{}[{}]", obj, "/*computed*/", prop) // fallback
                } else {
                    format!("{}[{}]", obj, prop)
                }
            }
            AssignmentTarget::StaticMemberExpression(m) => {
                let obj_str = self.emit_expression(&m.object);
                let prop = m.property.name.to_string();
                // this.x -> this->x (check both via type and string fallback)
                if let Expression::ThisExpression(_) = &m.object {
                    if self.ctx.in_static_method {
                        let class =
                            self.ctx.class_name.clone().unwrap_or_else(|| "this".to_string());
                        return format!("{}::{}", class, prop);
                    }
                    return format!("this->{}", prop);
                }
                if obj_str == "this" || obj_str == "this->" {
                    return format!("this->{}", prop);
                }
                if let Expression::Super(_) = &m.object {
                    if let Some(cls) = &self.ctx.class_name {
                        return format!("{}::{}", cls, prop);
                    }
                    return format!("/* super */{}", prop);
                }
                // For shared_ptr variables, use ->
                if self.ptr_arrow_access(&m.object, &obj_str) {
                    return format!("{}->{}", obj_str, prop);
                }
                // For JsObject bracket vs dot
                if self.should_use_bracket(&m.object, &obj_str) {
                    return format!("{}[\"{}\"]", obj_str, prop);
                }
                // Check class names for static access
                if let Expression::Identifier(id) = &m.object {
                    if self.ctx.class_names.contains(id.name.as_str()) {
                        return format!("{}::{}", obj_str, prop);
                    }
                }
                // Default dot
                format!("{}.{}", obj_str, prop)
            }
            AssignmentTarget::PrivateFieldExpression(p) => {
                let obj = self.emit_expression(&p.object);
                let field_name = p.field.name.to_string();
                self.private_field_access(&p.object, &field_name, &obj)
            }
            AssignmentTarget::ArrayAssignmentTarget(a) => self.span_text(a.span).to_string(),
            AssignmentTarget::ObjectAssignmentTarget(o) => self.span_text(o.span).to_string(),
            _ => self.span_text(target.span()).to_string(),
        }
    }

    fn emit_simple_target(&mut self, target: &SimpleAssignmentTarget<'a>) -> String {
        match target {
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
                if let Some(narrowed) = self.active_narrows.get(id.name.as_str()).cloned() {
                    return narrowed;
                }
                if let Some(dereferenced) = self.deref_shared_scalar(id.name.as_str()) {
                    return dereferenced;
                }
                id.name.to_string()
            }
            SimpleAssignmentTarget::ComputedMemberExpression(m) => format!(
                "{}[{}]",
                self.emit_expression(&m.object),
                self.emit_expression(&m.expression)
            ),
            SimpleAssignmentTarget::StaticMemberExpression(m) => {
                let obj_str = self.emit_expression(&m.object);
                let prop = m.property.name.to_string();
                if let Expression::ThisExpression(_) = &m.object {
                    return format!("this->{}", prop);
                }
                if self.ptr_arrow_access(&m.object, &obj_str) {
                    return format!("{}->{}", obj_str, prop);
                }
                format!("{}.{}", obj_str, prop)
            }
            SimpleAssignmentTarget::PrivateFieldExpression(p) => {
                let obj = self.emit_expression(&p.object);
                let field_name = p.field.name.to_string();
                self.private_field_access(&p.object, &field_name, &obj)
            }
            _ => self.span_text(target.span()).to_string(),
        }
    }

    /// JS builtin namespace calls (Math/JSON/Number.is*/Object.keys/
    /// Array.isArray), number formatting and variadic concat. Returns None
    /// to fall through to default emission (which the checker gates).
    fn emit_js_builtin(
        &mut self,
        root: &str,
        method: &str,
        recv: &Expression<'a>,
        call: &CallExpression<'a>,
    ) -> Option<String> {
        // Split fixed args from spreads. A lone spread expands the array
        // at runtime; mixed fixed+spread chains per group, staying exact.
        let mut fixed: Vec<String> = Vec::new();
        let mut spread_count = 0;
        for a in &call.arguments {
            if matches!(a, Argument::SpreadElement(_)) {
                spread_count += 1;
            } else {
                fixed.push(self.emit_argument(a));
            }
        }
        match (root, method) {
            ("Math", "min" | "max") => {
                self.ctx.need("JsValue");
                let fname =
                    if method == "min" { "morph::js_math::min" } else { "morph::js_math::max" };
                if spread_count == 0 {
                    return Some(format!("{fname}({{{}}})", fixed.join(", ")));
                }
                // max(...a, b, ...c) == max(max(a), b, max(c)).
                let mut parts = fixed;
                for a in &call.arguments {
                    if let Argument::SpreadElement(s) = a {
                        parts.push(format!("{fname}({})", self.emit_expression(&s.argument)));
                    }
                }
                Some(format!("{fname}({{{}}})", parts.join(", ")))
            }
            (
                "Math",
                "abs" | "floor" | "ceil" | "round" | "sqrt" | "trunc" | "sign" | "cbrt" | "exp"
                | "expm1" | "log" | "log2" | "log10" | "log1p" | "sin" | "cos" | "tan" | "asin"
                | "acos" | "atan" | "sinh" | "cosh" | "tanh" | "fround",
            ) => {
                self.ctx.need("JsValue");
                let arg =
                    fixed.first().cloned().unwrap_or_else(|| "JsValue(JsUndefined{})".to_string());
                Some(format!("morph::js_math::{method}({arg})"))
            }
            ("Math", "pow" | "atan2") => {
                self.ctx.need("JsValue");
                let x =
                    fixed.first().cloned().unwrap_or_else(|| "JsValue(JsUndefined{})".to_string());
                let y =
                    fixed.get(1).cloned().unwrap_or_else(|| "JsValue(JsUndefined{})".to_string());
                Some(format!("morph::js_math::{method}({x}, {y})"))
            }
            ("Math", "hypot") => {
                // Fixed arity only for now: a spread would collapse into a
                // single array argument. The checker gates that form.
                if spread_count > 0 {
                    return None;
                }
                self.ctx.need("JsValue");
                Some(format!("morph::js_math::hypot({{{}}})", fixed.join(", ")))
            }
            ("Math", "random") => {
                self.ctx.need("JsValue");
                Some("morph::js_math::random()".to_string())
            }
            ("JSON", "parse") => {
                // js_types.h (pulled by need("JsValue")) includes js_json.h.
                // A lone JSON.parse previously emitted `morph::json_parse`
                // without that header.
                self.ctx.need("JsValue");
                let arg =
                    fixed.first().cloned().unwrap_or_else(|| "JsValue(JsUndefined{})".to_string());
                Some(format!("morph::json_parse(JsValue({arg}))"))
            }
            ("JSON", "stringify") => {
                self.ctx.need("JsValue");
                let arg =
                    fixed.first().cloned().unwrap_or_else(|| "JsValue(JsUndefined{})".to_string());
                Some(format!("morph::json_stringify(JsValue({arg}))"))
            }
            ("Number", "isFinite") => Some(self.emit_js_unary("morph::js_is_finite", call)),
            ("Number", "isNaN") => Some(self.emit_js_unary("morph::js_is_nan", call)),
            ("Number", "isInteger") => Some(self.emit_js_unary("morph::js_is_integer", call)),
            ("Object", "keys") => {
                // Sorted enumeration (kill-runtime-strings decision): see
                // docs/javascript/types.md — deterministic, not insertion order.
                let arg = fixed.first().cloned().unwrap_or_else(|| "JsObject{}".to_string());
                Some(format!("JsArray(JsValue({arg}).sorted_keys())"))
            }
            ("Object", "values" | "entries") => {
                let arg = fixed.first().cloned().unwrap_or_else(|| "JsObject{}".to_string());
                Some(format!("JsValue({arg}).{method}()"))
            }
            ("Array", "isArray") => {
                let arg =
                    fixed.first().cloned().unwrap_or_else(|| "JsValue(JsUndefined{})".to_string());
                Some(format!("morph::js_is_array({arg})"))
            }
            (_, "toFixed" | "toPrecision" | "toExponential") => {
                // Numeric receivers only (ints, doubles, JsNumber, JsValue):
                // strings keep their own lowering, classes are untouched.
                if !matches!(
                    self.operand_class_of(recv),
                    OperandClass::Integer
                        | OperandClass::Float
                        | OperandClass::JsNumber
                        | OperandClass::JsValue
                ) {
                    return None;
                }
                let obj = self.emit_expression(recv);
                let args: Vec<String> =
                    call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                Some(format!("JsNumber({obj}).{method}({})", args.join(", ")))
            }
            (_, "concat") => {
                // Array receivers only; strings have no concat here. Array
                // literals infer as vectors but emit as JsArray, so they
                // count too.
                let is_arr = matches!(
                    self.operand_class_of(recv),
                    OperandClass::JsArray | OperandClass::JsValue
                ) || matches!(recv, Expression::ArrayExpression(_));
                if !is_arr {
                    return None;
                }
                let obj = self.emit_expression(recv);
                if spread_count == 0 {
                    return Some(format!("{obj}.concat({{{}}})", fixed.join(", ")));
                }
                // a.concat(x, ...s, y) == a.concat({x}).concat(s).concat({y}).
                let mut out = obj;
                let mut group: Vec<String> = Vec::new();
                for a in &call.arguments {
                    if let Argument::SpreadElement(s) = a {
                        if !group.is_empty() {
                            out = format!("{out}.concat({{{}}})", group.join(", "));
                            group.clear();
                        }
                        out = format!("{out}.concat({})", self.emit_expression(&s.argument));
                    } else {
                        group.push(self.emit_argument(a));
                    }
                }
                if !group.is_empty() {
                    out = format!("{out}.concat({{{}}})", group.join(", "));
                }
                Some(out)
            }
            _ => None,
        }
    }

    fn array_args_have_spread(args: &[Argument<'a>]) -> bool {
        args.iter().any(|arg| matches!(arg, Argument::SpreadElement(_)))
    }

    /// Comma-separated `JsValue(...)` items for an `initializer_list`.
    fn emit_array_items(&mut self, args: &[Argument<'a>]) -> String {
        self.ctx.need("JsValue");
        args.iter()
            .filter_map(|arg| match arg {
                Argument::SpreadElement(_) => None,
                _ => Some(format!("JsValue({})", self.emit_argument(arg))),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Build a `JsArray` from a mixed argument list, flattening spreads.
    /// `[a, ...b, c]` has the same result as the array literal spelling.
    fn emit_array_spread_args(&mut self, args: &[Argument<'a>]) -> String {
        self.ctx.need("JsArray");
        self.ctx.need("JsValue");
        let mut lines = vec!["[&]() {".to_string(), "    JsArray __args{};".to_string()];
        for arg in args {
            match arg {
                Argument::SpreadElement(spread) => {
                    let source = self.emit_expression(&spread.argument);
                    lines.push(format!("    morph::js_array_extend(__args, {source});"));
                }
                _ => {
                    let value = self.emit_argument(arg);
                    lines.push(format!("    __args.push(JsValue({value}));"));
                }
            }
        }
        lines.push("    return __args;".to_string());
        lines.push("}()".to_string());
        lines.join("\n")
    }

    fn emit_array_argument(&mut self, arg: &Argument<'a>) -> Option<String> {
        if matches!(arg, Argument::SpreadElement(_)) {
            return None;
        }
        Some(self.emit_argument(arg))
    }

    fn emit_array_index_argument(&mut self, arg: &Argument<'a>) -> Option<String> {
        self.emit_array_argument(arg).map(|value| format!("morph::js_to_integer(JsValue({value}))"))
    }

    /// Lower one general Array.prototype method call. `None` keeps the
    /// default member-call emission for non-array receivers and shapes the
    /// dedicated `push` fast path already handles.
    fn emit_array_method(
        &mut self,
        member: &StaticMemberExpression<'a>,
        call: &CallExpression<'a>,
    ) -> Option<String> {
        let method = member.property.name.as_str();
        if !JS_ARRAY_METHODS.contains(&method) || !self.is_array_method_receiver(&member.object) {
            return None;
        }
        // The single-value `push` path above handles vectors with
        // push_back and JsArray with push.
        if method == "push"
            && call.arguments.len() == 1
            && !matches!(call.arguments[0], Argument::SpreadElement(_))
        {
            return None;
        }
        self.ctx.need("JsArray");
        self.ctx.need("JsValue");
        let object = self.emit_expression(&member.object);
        match method {
            "map" | "filter" | "forEach" | "find" | "findIndex" | "some" | "every" | "flatMap" => {
                let callback = self.emit_array_argument(call.arguments.first()?)?;
                let helper = match method {
                    "map" => "js_array_map",
                    "filter" => "js_array_filter",
                    "forEach" => "js_array_for_each",
                    "find" => "js_array_find",
                    "findIndex" => "js_array_find_index",
                    "some" => "js_array_some",
                    "every" => "js_array_every",
                    _ => "js_array_flat_map",
                };
                Some(format!("morph::{helper}({object}, {callback})"))
            }
            "reduce" => {
                let callback = self.emit_array_argument(call.arguments.first()?)?;
                if let Some(initial) = call.arguments.get(1) {
                    let initial = self.emit_array_argument(initial)?;
                    Some(format!("morph::js_array_reduce({object}, {callback}, {initial})"))
                } else {
                    Some(format!("morph::js_array_reduce({object}, {callback})"))
                }
            }
            "sort" => {
                if let Some(callback) = call.arguments.first() {
                    let callback = self.emit_array_argument(callback)?;
                    Some(format!("morph::js_array_sort({object}, {callback})"))
                } else {
                    Some(format!("morph::js_array_sort({object})"))
                }
            }
            "reverse" => Some(format!("morph::js_array_reverse({object})")),
            "pop" => Some(format!("morph::js_array_pop({object})")),
            "shift" => Some(format!("morph::js_array_shift({object})")),
            "push" | "unshift" => {
                let helper = if method == "push" { "js_array_push" } else { "js_array_unshift" };
                let items = if Self::array_args_have_spread(&call.arguments) {
                    self.emit_array_spread_args(&call.arguments)
                } else {
                    format!("{{{}}}", self.emit_array_items(&call.arguments))
                };
                Some(format!("morph::{helper}({object}, {items})"))
            }
            "splice" => {
                let start = self.emit_array_index_argument(call.arguments.first()?)?;
                if call.arguments.len() == 1 {
                    return Some(format!("morph::js_array_splice({object}, {start})"));
                }
                let delete = self.emit_array_index_argument(&call.arguments[1])?;
                if call.arguments.len() == 2 {
                    return Some(format!("morph::js_array_splice({object}, {start}, {delete})"));
                }
                let items = &call.arguments[2..];
                let items = if Self::array_args_have_spread(items) {
                    self.emit_array_spread_args(items)
                } else {
                    format!("{{{}}}", self.emit_array_items(items))
                };
                Some(format!("morph::js_array_splice({object}, {start}, {delete}, {items})"))
            }
            "fill" => {
                let value = self.emit_array_argument(call.arguments.first()?)?;
                let value = format!("JsValue({value})");
                match call.arguments.len() {
                    0 | 1 => Some(format!("morph::js_array_fill({object}, {value})")),
                    2 => {
                        let start = self.emit_array_index_argument(&call.arguments[1])?;
                        Some(format!("morph::js_array_fill({object}, {value}, {start})"))
                    }
                    _ => {
                        let start = self.emit_array_index_argument(&call.arguments[1])?;
                        let end = self.emit_array_index_argument(&call.arguments[2])?;
                        Some(format!("morph::js_array_fill({object}, {value}, {start}, {end})"))
                    }
                }
            }
            "flat" => {
                if let Some(depth) = call.arguments.first() {
                    let depth = self.emit_array_index_argument(depth)?;
                    Some(format!("morph::js_array_flat({object}, {depth})"))
                } else {
                    Some(format!("morph::js_array_flat({object})"))
                }
            }
            _ => None,
        }
    }

    /// Single-argument builtin call with an undefined default. The arg is
    /// wrapped in JsValue so overload resolution stays deterministic.
    fn emit_js_unary(&mut self, fname: &str, call: &CallExpression<'a>) -> String {
        let arg = call
            .arguments
            .first()
            .map_or_else(|| "JsValue(JsUndefined{})".to_string(), |a| self.emit_argument(a));
        format!("{fname}(JsValue({arg}))")
    }

    fn emit_call(&mut self, call: &CallExpression<'a>) -> String {
        let callee_str = self.emit_expression(&call.callee);
        let type_args_str = if let Some(ta) = &call.type_arguments {
            let targs: Vec<String> = ta
                .params
                .iter()
                .map(|t| {
                    resolve_type(
                        Some(t),
                        "auto",
                        &self.ctx.template_params,
                        false,
                        &self.ctx.class_names,
                    )
                })
                .collect();
            if targs.is_empty() { String::new() } else { format!("<{}>", targs.join(", ")) }
        } else {
            String::new()
        };
        if let Expression::Identifier(id) = &call.callee {
            // `useWindow()` — current window (codegen fills the WID per
            // emission site); `useWindow(id-or-route)` — registry lookup
            // (codegen resolves `/route` literals to RIDs, ids lower to a
            // runtime alias lookup).
            if id.name.as_str() == "useWindow" {
                if call.arguments.is_empty() {
                    return "__morph_current_window()".to_string();
                }
                let arg = self.emit_argument(&call.arguments[0]);
                return format!("__morph_use_window({arg})");
            }
            if id.name.as_str() == "require" {
                self.ctx.need("JsUndefined");
                return "[] { static_assert(sizeof(\"morph require\") == 0, \"morph: require calls are not supported (use ES module imports)\"); return JsUndefined{}; }()".to_string();
            }
            if id.name.as_str() == "fetch" {
                self.ctx.needed.insert("\"../../runtime/cpp/net/net.h\"".to_string());
                let args: Vec<String> =
                    call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                return format!("morph::net::fetch({})", args.join(", "));
            }
            if matches!(
                id.name.as_str(),
                "setTimeout" | "setInterval" | "clearTimeout" | "clearInterval"
            ) {
                self.ctx.needed.insert("\"../../runtime/cpp/reactivity/task.h\"".to_string());
                if id.name.as_str() == "clearTimeout" || id.name.as_str() == "clearInterval" {
                    let args: Vec<String> =
                        call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                    return format!("morph::clear_timer({})", args.join(", "));
                } else {
                    let fn_arg = call
                        .arguments
                        .first()
                        .map(|a| self.emit_argument(a))
                        .unwrap_or_else(|| "[](){}".to_string());
                    let delay = call
                        .arguments
                        .get(1)
                        .map(|a| self.emit_argument(a))
                        .unwrap_or_else(|| "0".to_string());
                    let cpp_fn = if id.name.as_str() == "setTimeout" {
                        "morph::set_timeout"
                    } else {
                        "morph::set_interval"
                    };
                    return format!("{}(std::function<void()>({}), {})", cpp_fn, fn_arg, delay);
                }
            }
            // JS value conversions: String(x) / Number(x) / Boolean(x) lower
            // to the morph::js_to_* helpers (JsValue semantics, never throw).
            // Zero-arg forms convert undefined, matching JS.
            if matches!(id.name.as_str(), "String" | "Number" | "Boolean") {
                let helper = match id.name.as_str() {
                    "String" => "morph::js_to_string",
                    "Number" => "morph::js_to_number",
                    _ => "morph::js_to_boolean",
                };
                if call.arguments.is_empty() {
                    return format!("{helper}()");
                }
                let arg = self.emit_argument(&call.arguments[0]);
                return format!("{helper}(JsValue({arg}))");
            }
            if id.name.as_str() == "parseInt" {
                let mut args: Vec<String> =
                    call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                if args.is_empty() {
                    args.push("JsValue(JsUndefined{})".to_string());
                } else {
                    args[0] = format!("JsValue({})", args[0]);
                }
                return format!("morph::js_parse_int({})", args.join(", "));
            }
            if id.name.as_str() == "parseFloat" {
                if call.arguments.is_empty() {
                    return "morph::js_parse_float(JsValue(JsUndefined{}))".to_string();
                }
                let arg = self.emit_argument(&call.arguments[0]);
                return format!("morph::js_parse_float(JsValue({arg}))");
            }
            if let Some(mapped) = self.ctx.state_vars.get(id.name.as_str()).cloned() {
                let args: Vec<String> =
                    call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                if mapped.trim_end().ends_with(')') {
                    // Mapping is already a complete call (signal `.get()`,
                    // reactive-const `name()`): extra args would be dropped
                    // exactly as before.
                    return mapped;
                }
                // Otherwise the mapping is a callable head (signal `.set`,
                // namespaced function): append the call parentheses, even
                // when empty, so zero-arg calls keep theirs.
                return format!("{}({})", mapped, args.join(", "));
            }
        }
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if let Expression::Identifier(obj) = &m.object {
                if obj.name.as_str() == "console"
                    && matches!(m.property.name.as_str(), "log" | "warn" | "error" | "info")
                {
                    self.ctx.needed.insert("<print>".to_string());
                    // Formatter for Js* types is provided by js_types.h (which includes
                    // js_value_format.h by default), so no explicit insert needed here.
                    let is_warn = m.property.name.as_str() == "warn";
                    let is_error = m.property.name.as_str() == "error";
                    // Fast path: single template literal -> use println directly, no <format> needed
                    if call.arguments.len() == 1 {
                        if let Some(expr) = call.arguments[0].as_expression() {
                            if let Expression::TemplateLiteral(t) = expr {
                                let (fstr, targs) = self.extract_template_parts(t);
                                if targs.is_empty() {
                                    let esc = escape_cpp_text(&fstr);
                                    if is_warn || is_error {
                                        return format!("std::println(stderr, \"{}\")", esc);
                                    }
                                    return format!("std::println(\"{}\")", esc);
                                }
                                let esc = escape_cpp_text(&fstr);
                                if is_warn || is_error {
                                    return format!(
                                        "std::println(stderr, \"{}\", {})",
                                        esc,
                                        targs.join(", ")
                                    );
                                }
                                return format!("std::println(\"{}\", {})", esc, targs.join(", "));
                            }
                            if let Expression::StringLiteral(s) = expr {
                                let esc = escape_cpp_text(&s.value);
                                if is_warn || is_error {
                                    return format!("std::println(stderr, \"{}\")", esc);
                                }
                                return format!("std::println(\"{}\")", esc);
                            }
                        }
                    }
                    let mut args: Vec<String> = Vec::new();
                    for a in &call.arguments {
                        if let Some(expr) = a.as_expression() {
                            args.push(self.emit_expression(expr));
                        } else if let Argument::SpreadElement(s) = a {
                            args.push(self.emit_expression(&s.argument));
                        }
                    }
                    if args.is_empty() {
                        if is_warn || is_error {
                            return "std::println(stderr, \"\")".to_string();
                        }
                        return "std::println(\"\")".to_string();
                    }
                    // Single arg handling (non-template/string already handled above, but keep for safety)
                    if args.len() == 1 {
                        if let Some(expr) = call.arguments[0].as_expression() {
                            if let Expression::StringLiteral(s) = expr {
                                let esc = escape_cpp_text(&s.value);
                                if is_warn || is_error {
                                    return format!("std::println(stderr, \"{}\")", esc);
                                }
                                return format!("std::println(\"{}\")", esc);
                            }
                            if let Expression::TemplateLiteral(t) = expr {
                                let (fstr, targs) = self.extract_template_parts(t);
                                if targs.is_empty() {
                                    let esc = escape_cpp_text(&fstr);
                                    if is_warn || is_error {
                                        return format!("std::println(stderr, \"{}\")", esc);
                                    }
                                    return format!("std::println(\"{}\")", esc);
                                }
                                let esc = escape_cpp_text(&fstr);
                                if is_warn || is_error {
                                    return format!(
                                        "std::println(stderr, \"{}\", {})",
                                        esc,
                                        targs.join(", ")
                                    );
                                }
                                return format!("std::println(\"{}\", {})", esc, targs.join(", "));
                            }
                        }
                        if is_warn || is_error {
                            return format!("std::println(stderr, \"{{}}\", {})", args[0]);
                        }
                        return format!("std::println(\"{{}}\", {})", args[0]);
                    }
                    // Multiple args: space-separated like JS console.log
                    let fmt = args.iter().map(|_| "{}").collect::<Vec<_>>().join(" ");
                    if is_warn || is_error {
                        return format!("std::println(stderr, \"{}\", {})", fmt, args.join(", "));
                    }
                    return format!("std::println(\"{}\", {})", fmt, args.join(", "));
                }
            }
        }
        // Window-handle methods (`win.navigate/close/show/hide/on`) —
        // only for trackable Window handles (tracked locals, `Window`-typed
        // bindings, lazy placeholders, or direct `useWindow()` /
        // `new Window()`), so user objects with same-named methods keep
        // default emission.
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if let Some(obj) = self.window_handle_expr(&m.object) {
                if let Some(lowered) = self.emit_window_method(&obj, m.property.name.as_str(), call)
                {
                    return lowered;
                }
            }
        }
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if m.property.name.as_str() == "push"
                && call.arguments.len() == 1
                && !matches!(call.arguments[0], Argument::SpreadElement(_))
            {
                let obj = self.emit_expression(&m.object);
                let arg = self.emit_moved_argument(&call.arguments[0], true);
                // std::vector uses push_back, JsArray uses push.
                // this->member is a JsArray class field -> push.
                if !obj.contains("->") {
                    if let Expression::Identifier(id) = &m.object {
                        if self
                            .ctx
                            .var_types
                            .get(id.name.as_str())
                            .map(|t| t.starts_with("std::vector<"))
                            .unwrap_or(false)
                        {
                            return format!("{}.push_back({})", obj, arg);
                        }
                    }
                }
                return format!("{}.push({})", obj, arg);
            }
        }
        // General Array.prototype methods (`map`, `filter`, `reduce`, ...)
        // lower to morph::js_array_* helpers. JSX list `.map` calls never
        // reach here: the walker rewrites them into list nodes.
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if let Some(lowered) = self.emit_array_method(m, call) {
                return lowered;
            }
        }
        // JS builtin namespaces (Math/JSON/Number.is*/Object.keys/Array.isArray),
        // number formatting and variadic concat. Anything else on these roots
        // falls through verbatim and stays a checker error (mx-js-member).
        if let Expression::StaticMemberExpression(m) = &call.callee {
            let root =
                if let Expression::Identifier(obj) = &m.object { obj.name.as_str() } else { "" };
            if let Some(lowered) =
                self.emit_js_builtin(root, m.property.name.as_str(), &m.object, call)
            {
                return lowered;
            }
        }
        // Handle string methods on native types (std::string and number types)
        // Supports chaining: morph::strutil::outer(morph::strutil::inner(...))
        if let Expression::StaticMemberExpression(m) = &call.callee {
            let method = m.property.name.as_str();
            if StringMethodHandler::is_string_method(method) {
                if let Some(use_helper) = self.str_helper_decision(&m.object) {
                    if use_helper {
                        let obj_expr = self.emit_expression(&m.object);
                        let args: Vec<String> =
                            call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                        return StringMethodHandler::translate_method(
                            &mut self.ctx,
                            &obj_expr,
                            method,
                            &args,
                            false,
                        );
                    }
                    // use_helper == false -> fall through to direct Js call below
                }
            }
        }
        let allow_move = !matches!(
            &call.callee,
            Expression::StaticMemberExpression(m)
                if matches!(m.property.name.as_str(), "log" | "warn" | "error" | "info")
        );
        let args: Vec<String> =
            call.arguments.iter().map(|a| self.emit_moved_argument(a, allow_move)).collect();
        if let Expression::Super(_) = &call.callee {
            return format!("super{}({})", type_args_str, args.join(", "));
        }
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if !m.optional {
                let obj_str = self.emit_expression(&m.object);
                if self.ctx.shared_ptr_vars.contains(&obj_str) {
                    return format!("{}{}({})", callee_str, type_args_str, args.join(", "));
                }
                if !self.is_js_object_type(&m.object) {
                    return format!("{}{}({})", callee_str, type_args_str, args.join(", "));
                }
                return format!(
                    "{}{}({}{})",
                    callee_str,
                    type_args_str,
                    obj_str,
                    if args.is_empty() { String::new() } else { format!(", {}", args.join(", ")) }
                );
            }
        }
        format!("{}{}({})", callee_str, type_args_str, args.join(", "))
    }

    fn emit_argument(&mut self, arg: &Argument<'a>) -> String {
        match arg {
            Argument::SpreadElement(s) => {
                format!("/* spread */ {}", self.emit_expression(&s.argument))
            }
            _ => {
                if let Some(expr) = arg.as_expression() {
                    self.emit_expression(expr)
                } else {
                    "/* arg */".to_string()
                }
            }
        }
    }

    /// True when the variable's value can move at the use starting at
    /// `span_start`.
    ///
    /// Sound by construction: the use is the variable's last read in the
    /// enclosing function (the analyzer maps last-read spans once per
    /// function, keyed like captures), the variable is not global, never
    /// captured, never touched in a loop, and the type is expensive to
    /// copy. Anything unproven keeps copying.
    fn move_eligible_at(&self, name: &str, span_start: u32) -> bool {
        let Some(analysis) = self.analysis.as_ref() else {
            return false;
        };
        let Some(info) = analysis.var_infos.get(name) else {
            return false;
        };
        if info.escape_kind == EscapeKind::Global || info.has_loop_use {
            return false;
        }
        if analysis.closure_captures.values().any(|captured| captured.contains(name)) {
            return false;
        }
        let scope = self.ctx.current_fn.clone().unwrap_or_default();
        let last_read =
            analysis.last_read_spans.get(&scope).and_then(|reads| reads.get(name)).copied();
        if last_read != Some(span_start) {
            return false;
        }
        let cpp_type = self.ctx.var_types.get(name).map(String::as_str).unwrap_or("");
        Self::is_move_worthy_type(cpp_type, &self.ctx.class_names)
    }

    /// True for types where moving beats copying. `unique_ptr` must move:
    /// copying one is a compile error, so call sites are required to move it.
    fn is_move_worthy_type(cpp_type: &str, class_names: &HashSet<String>) -> bool {
        cpp_type.starts_with("std::vector<")
            || cpp_type.starts_with("std::unique_ptr<")
            || cpp_type == "JsObject"
            || cpp_type == "JsArray"
            || cpp_type == "std::string"
            || cpp_type == "JsString"
            || cpp_type == "JsValue"
            || cpp_type.starts_with("std::shared_ptr<")
            || class_names.contains(cpp_type)
    }

    /// Emit a call argument, moving the value at its last read.
    fn emit_moved_argument(&mut self, arg: &Argument<'a>, allow_move: bool) -> String {
        if allow_move {
            if let Some(Expression::Identifier(id)) = arg.as_expression() {
                if self.move_eligible_at(id.name.as_str(), id.span.start) {
                    return format!("std::move({})", id.name);
                }
            }
        }
        self.emit_argument(arg)
    }

    /// True for array literals containing a spread element (`[...a, x]`).
    fn is_spread_array(init: &Expression<'_>) -> bool {
        if let Expression::ArrayExpression(arr) = init {
            arr.elements.iter().any(|e| matches!(e, ArrayExpressionElement::SpreadElement(_)))
        } else {
            false
        }
    }

    fn vector_inner_type(vec_type: &str) -> Option<&str> {
        let t = vec_type.trim();
        let prefix = "std::vector<";
        if t.starts_with(prefix) && t.ends_with('>') {
            Some(&t[prefix.len()..t.len() - 1])
        } else {
            None
        }
    }

    fn emit_std_vector_literal(&mut self, init: &Expression<'a>) -> String {
        self.emit_vector_literal_typed(init, None)
    }

    /// Emit `{...}` for an array literal, recursing into nested arrays when the
    /// expected element type is itself a `std::vector<...>` (otherwise nested
    /// arrays would emit as `JsArray{...}` and fail to convert).
    fn emit_vector_literal_typed(
        &mut self,
        init: &Expression<'a>,
        vec_type: Option<&str>,
    ) -> String {
        if let Expression::ArrayExpression(arr) = init {
            let inner = vec_type.and_then(Self::vector_inner_type);
            let elems: Vec<String> = arr
                .elements
                .iter()
                .filter_map(|e| match e {
                    ArrayExpressionElement::Elision(_) => None,
                    _ => e.as_expression().map(|ex| {
                        if let (Some(inner_t), Expression::ArrayExpression(_)) = (inner, ex) {
                            if inner_t.trim_start().starts_with("std::vector<") {
                                return self.emit_vector_literal_typed(ex, Some(inner_t));
                            }
                        }
                        self.emit_expression(ex)
                    }),
                })
                .collect();
            format!("{{{}}}", elems.join(", "))
        } else {
            self.emit_expression(init)
        }
    }

    fn emit_char_literal(&mut self, init: &Expression<'a>) -> String {
        if let Expression::StringLiteral(s) = init {
            let ch = s.value.as_str();
            if ch.len() == 1 {
                return cpp_char_literal(ch);
            }
        }
        self.emit_expression(init)
    }

    fn is_js_object_type(&self, obj: &Expression<'a>) -> bool {
        if let Expression::Identifier(id) = obj {
            return self
                .ctx
                .var_types
                .get(id.name.as_str())
                .map(|t| t == "JsObject")
                .unwrap_or(false);
        }
        // For `a.b` or `a[i]` where `a` is JsObject, check via var_types or obj_str
        // Don't return true for generic ComputedMemberExpression like `vector[0]` which is JsString
        false
    }

    /// Class whose accessors apply to a member receiver, plus whether the
    /// receiver is the class object itself (`A.value`) or an instance.
    fn accessor_receiver_class(&self, object: &Expression<'a>) -> Option<(String, bool)> {
        match object {
            Expression::ThisExpression(_) => {
                // Inside a function expression `this` is the JS `_jsThis`
                // object, not the enclosing C++ instance.
                if self.ctx.fn_expr_depth > 0 {
                    return None;
                }
                let class = self.ctx.class_name.clone()?;
                Some((class, self.ctx.in_static_method))
            }
            Expression::Identifier(id) => {
                let name = id.name.as_str();
                if self.ctx.class_names.contains(name) {
                    Some((name.to_string(), true))
                } else {
                    let ty = self.ctx.var_types.get(name)?;
                    let class = self.class_name_from_cpp_type(ty)?;
                    Some((class, false))
                }
            }
            Expression::NewExpression(n) => match &n.callee {
                Expression::Identifier(id) if self.ctx.class_names.contains(id.name.as_str()) => {
                    Some((id.name.to_string(), false))
                }
                _ => None,
            },
            Expression::ParenthesizedExpression(p) => self.accessor_receiver_class(&p.expression),
            Expression::TSAsExpression(a) => self.accessor_receiver_class(&a.expression),
            Expression::TSSatisfiesExpression(s) => self.accessor_receiver_class(&s.expression),
            Expression::TSNonNullExpression(n) => self.accessor_receiver_class(&n.expression),
            _ => None,
        }
    }

    /// Class name inside a C++ storage type (`std::shared_ptr<C>`, `C&`, ...).
    /// Ambient class bindings may carry a qualified C++ path
    /// (`::app::other::Counter`); metadata is keyed by the local name.
    fn class_name_from_cpp_type(&self, ty: &str) -> Option<String> {
        let mut ty = ty.trim();
        if let Some(inner) = ty.strip_prefix("std::shared_ptr<") {
            ty = inner.strip_suffix('>')?.trim();
        } else if let Some(inner) = ty.strip_prefix("std::unique_ptr<") {
            ty = inner.strip_suffix('>')?.trim();
        }
        let ty = ty.trim_start_matches("const ").trim_end_matches('&').trim();
        if self.ctx.class_names.contains(ty) {
            return Some(ty.to_string());
        }
        let bare = ty.rsplit("::").next().unwrap_or(ty);
        if bare != ty && self.ctx.class_names.contains(bare) {
            return Some(bare.to_string());
        }
        None
    }

    /// C++ spelling of a class name. Snippets spliced into another
    /// translation unit see classes through a qualified ambient binding
    /// (`Counter` → `::app::app::Counter`); full-file translations emit
    /// classes in the current scope and keep the bare name.
    fn class_cpp_name(&self, name: &str) -> String {
        if let Some(mapped) = self.ctx.state_vars.get(name) {
            let mapped = mapped.trim();
            if mapped.starts_with("::") && !mapped.contains('(') && !mapped.contains('.') {
                return mapped.to_string();
            }
        }
        name.to_string()
    }

    fn accessor_member_sep(
        &self,
        object: &Expression<'a>,
        obj_code: &str,
        is_static: bool,
    ) -> &'static str {
        if is_static {
            "::"
        } else if matches!(object, Expression::ThisExpression(_))
            || matches!(object, Expression::NewExpression(_))
            || self.ptr_arrow_access(object, obj_code)
        {
            "->"
        } else {
            "."
        }
    }

    fn accessor_halves(&self, class: &str, prop: &str, is_static: bool) -> Option<(bool, bool)> {
        let flags = self.ctx.class_accessors.get(class)?.get(prop)?;
        Some((flags.has_getter(is_static), flags.has_setter(is_static)))
    }

    /// Lower a getter read (`a.value` -> `a->value()`), or None when the
    /// receiver's class is unknown or the property is not an accessor.
    fn accessor_read(&self, object: &Expression<'a>, prop: &str, obj_code: &str) -> Option<String> {
        let (class, is_static) = self.accessor_receiver_class(object)?;
        let (getter, _) = self.accessor_halves(&class, prop, is_static)?;
        if !getter {
            return None;
        }
        let sep = self.accessor_member_sep(object, obj_code, is_static);
        Some(format!("{obj_code}{sep}{prop}()"))
    }

    /// Mangled private-field access with the right member operator.
    fn private_field_access(&self, object: &Expression<'a>, field: &str, obj_code: &str) -> String {
        let mangled = match &self.ctx.class_name {
            Some(class) => format!("__private_{}_{}", class, field),
            None => format!("__private_{}", field),
        };
        let sep = if matches!(object, Expression::ThisExpression(_)) && self.ctx.in_static_method {
            "::"
        } else if matches!(object, Expression::ThisExpression(_))
            || matches!(object, Expression::NewExpression(_))
            || self.ptr_arrow_access(object, obj_code)
        {
            "->"
        } else if let Expression::Identifier(id) = object {
            if self.ctx.class_names.contains(id.name.as_str()) { "::" } else { "." }
        } else {
            "."
        };
        format!("{obj_code}{sep}{mangled}")
    }

    fn emit_static_member(&mut self, m: &StaticMemberExpression<'a>) -> String {
        let obj = self.emit_expression(&m.object);
        let prop = m.property.name.to_string();
        if let Expression::Identifier(id) = &m.object {
            if id.name.as_str() == "Math" && is_math_constant(&prop) {
                self.ctx.need("JsValue");
                return format!("morph::js_math::{prop}");
            }
        }
        if let Some(read) = self.accessor_read(&m.object, &prop, &obj) {
            return read;
        }
        if let Expression::ThisExpression(_) = &m.object {
            if self.ctx.fn_expr_depth > 0 {
                return format!("_jsThis[\"{}\"]", prop);
            }
            if self.ctx.in_static_method {
                let class = self.ctx.class_name.clone().unwrap_or_else(|| "this".to_string());
                return format!("{}::{}", class, prop);
            }
            return format!("this->{}", prop);
        }
        if let Expression::Super(_) = &m.object {
            if let Some(cls) = &self.ctx.class_name {
                return format!("{}::{}", cls, prop);
            }
            return format!("/* super */{}", prop);
        }
        // Window-handle properties: `closed` and `title` lower to registry
        // reads on every handle shape; anything else keeps default emission
        // (wrong handle members fail the C++ compile loudly).
        if let Some(obj) = self.window_handle_expr(&m.object) {
            match prop.as_str() {
                "closed" => return format!("__morph_win_closed({obj})"),
                "title" => return format!("__morph_win_title({obj})"),
                _ => {}
            }
        }
        // Response.ok is a method, not a field: r.ok -> r.ok()
        if prop == "ok" {
            // Heuristic: if obj is Response (from fetch), call ok()
            let is_response = self
                .ctx
                .var_types
                .get(obj.as_str())
                .map(|t| t.contains("Response"))
                .unwrap_or(false)
                || obj.contains("Response")
                || obj == "r"
                || obj.contains("fetch")
                || self.ctx.needed.iter().any(|h| h.contains("net.h"));
            if is_response {
                // Bare member: emit_call appends the argument parens, so
                // `r.ok()` renders exactly once (not `r.ok()()`).
                return format!("{}.ok", obj);
            }
        }
        if prop == "length" {
            // .size() works universally now: std::vector/std::string have size(),
            // JsArray/JsString gained size(), JsValue forwards size()->length().
            // Only JsObject keeps .length() fallback (no size concept).
            let is_js_object = if let Expression::Identifier(id) = &m.object {
                self.ctx.var_types.get(id.name.as_str()).map(|t| t == "JsObject").unwrap_or(false)
            } else {
                false
            };
            if is_js_object {
                return format!("(int)({}.length())", obj);
            }
            return format!("(int)({}.size())", obj);
        }
        // Known JsString/JsArray methods should use dot, not bracket
        if matches!(
            prop.as_str(),
            "toUpperCase"
                | "toLowerCase"
                | "charAt"
                | "indexOf"
                | "substring"
                | "slice"
                | "trim"
                | "replace"
                | "split"
                | "startsWith"
                | "endsWith"
                | "includes"
                | "toString"
                | "length"
                | "push"
                | "pop"
        ) {
            // Use dot
        } else if self.should_use_bracket(&m.object, &obj) {
            return format!("{}[\"{}\"]", obj, prop);
        }
        if self.ptr_arrow_access(&m.object, &obj) {
            return format!("{}->{}", obj, prop);
        }
        if let Expression::Identifier(id) = &m.object {
            if self.ctx.class_names.contains(id.name.as_str()) {
                return format!("{}::{}", obj, prop);
            }
        }
        // `new C()` yields a shared_ptr, so members go through `->`.
        if matches!(&m.object, Expression::NewExpression(_)) {
            return format!("{}->{}", obj, prop);
        }
        format!("{}.{}", obj, prop)
    }

    fn should_use_bracket(&self, obj_node: &Expression<'a>, obj_str: &str) -> bool {
        if let Expression::Identifier(id) = obj_node {
            if self.ctx.js_object_params.contains(id.name.as_str()) {
                return true;
            }
            if let Some(ty) = self.ctx.var_types.get(id.name.as_str()) {
                // Boxed JS values are read through `(*x)`, so member access
                // is a bracket lookup on the dereferenced value.
                if let Some(inner) =
                    ty.strip_prefix("std::shared_ptr<").and_then(|value| value.strip_suffix('>'))
                {
                    return matches!(inner.trim(), "JsValue" | "JsLateValue");
                }
                return matches!(ty.as_str(), "JsObject" | "JsValue" | "JsLateValue");
            }
            return false;
        }
        // For chained access like `a.b.c` or `arr[i].prop`, infer the
        // receiver type. `users[0]` on `std::vector<JsObject>` is a
        // JsObject and must bracket; `arr[i]` on a vector<string> stays
        // dot for string methods.
        if let Some(t) = self.infer_type_from_init(obj_node) {
            if matches!(t.as_str(), "JsObject" | "JsValue" | "JsLateValue") {
                return true;
            }
        }
        if let Some(t) = self.ctx.var_types.get(obj_str) {
            return matches!(t.as_str(), "JsObject" | "JsValue" | "JsLateValue");
        }
        // Also check if obj_str looks like an object access (contains ["), then likely JsObject
        if obj_str.contains("[\"") {
            return true;
        }
        false
    }

    fn emit_chain(&mut self, chain: &ChainExpression<'a>) -> String {
        match &chain.expression {
            ChainElement::CallExpression(c) => self.emit_call(c),
            ChainElement::StaticMemberExpression(m) => self.emit_static_member(m),
            ChainElement::ComputedMemberExpression(m) => format!(
                "{}[{}]",
                self.emit_expression(&m.object),
                self.emit_expression(&m.expression)
            ),
            ChainElement::PrivateFieldExpression(p) => {
                let obj = self.emit_expression(&p.object);
                let field_name = p.field.name.to_string();
                self.private_field_access(&p.object, &field_name, &obj)
            }
            _ => "/* chain */".to_string(),
        }
    }

    /// True for bindings carrying a window handle: locals tracked from a
    /// `useWindow()` / `new Window()` initializer, late-assigned handles,
    /// and `Window`-typed bindings (parameters, props, annotated locals).
    fn is_window_handle_var(&self, name: &str) -> bool {
        self.ctx.window_vars.contains(name)
            || self.late_window_vars.contains(name)
            || self
                .ctx
                .var_types
                .get(name)
                .is_some_and(|ty| ty == "WID" || ty.contains("JsWindowHandle"))
    }

    /// Resolve a window-handle receiver to its WID expression.
    ///
    /// Handles arrive in several shapes: tracked locals, `Window`-typed
    /// bindings, lazy component-body placeholders (`__morph_current_window()`
    /// / `__morph_use_window(...)`), and direct `useWindow(...)` /
    /// `new Window(...)` expressions. Parentheses and TS wrappers are
    /// unwrapped first because lazy const substitution inserts a call
    /// expression at the use site.
    fn window_handle_expr(&mut self, obj: &Expression<'a>) -> Option<String> {
        let obj = Self::unwrap_expr(obj);
        if let Expression::Identifier(id) = obj {
            let name = id.name.as_str();
            if !self.is_window_handle_var(name) {
                return None;
            }
            let emitted = self.emit_expression(obj);
            let ty = self.ctx.var_types.get(name).map(String::as_str).unwrap_or_default();
            // Late handles are `JsValue`s (possibly closure-wrapped, in
            // which case `emit_expression` already dereferenced them).
            // Narrow to the WID only at the native call boundary.
            if ty.contains("JsWindowHandle")
                || (self.late_window_vars.contains(name) && ty != "WID")
            {
                return Some(format!("window_wid({emitted})"));
            }
            return Some(emitted);
        }
        if let Expression::CallExpression(call) = obj {
            if let Expression::Identifier(callee) = &call.callee {
                if matches!(
                    callee.name.as_str(),
                    "__morph_current_window" | "__morph_use_window" | "useWindow"
                ) {
                    return Some(self.emit_expression(obj));
                }
            }
            return None;
        }
        if let Expression::NewExpression(n) = obj {
            if matches!(&n.callee, Expression::Identifier(id) if id.name.as_str() == "Window")
                && !n.arguments.is_empty()
            {
                return Some(self.emit_expression(obj));
            }
        }
        None
    }

    /// Lower a method call on a tracked Window handle (WID int) to a
    /// placeholder (route-carrying ops, resolved at codegen) or a direct
    /// `WindowManager` call. `None` falls through to default emission
    /// (fails C++ compilation loudly on the int handle).
    fn emit_window_method(
        &mut self,
        obj: &str,
        method: &str,
        call: &CallExpression<'a>,
    ) -> Option<String> {
        match method {
            "navigate" => {
                let route =
                    call.arguments.first().map(|a| self.emit_argument(a)).unwrap_or_default();
                let props = call
                    .arguments
                    .get(1)
                    .map(|a| self.emit_argument(a))
                    .unwrap_or_else(|| "JsObject{}".to_string());
                Some(format!("__morph_win_navigate({obj}, {route}, {props})"))
            }
            "close" => Some(format!("WindowManager::get().close({obj})")),
            "show" => Some(format!("WindowManager::get().open({obj})")),
            "hide" => Some(format!("WindowManager::get().hide({obj})")),
            "on" => {
                let event =
                    call.arguments.first().map(|a| self.emit_argument(a)).unwrap_or_default();
                let handler = call
                    .arguments
                    .get(1)
                    .map(|a| self.emit_argument(a))
                    .unwrap_or_else(|| "[](const JsValue&){}".to_string());
                if event.trim_matches('"').trim_matches('\'') == "close" {
                    Some(format!("WindowManager::get().onClose({obj}, {handler})"))
                } else {
                    Some(format!(
                        "static_assert(sizeof({obj}) == 0, \"Window.on only supports 'close'\")"
                    ))
                }
            }
            "ready" => Some(format!(
                "static_assert(sizeof({obj}) == 0, \"Window.ready() is not built yet\")"
            )),
            _ => None,
        }
    }

    fn emit_new(&mut self, n: &NewExpression<'a>) -> String {
        let callee = self.emit_expression(&n.callee);
        let args: Vec<String> = n.arguments.iter().map(|a| self.emit_argument(a)).collect();
        // `new Window(routeId, config?)` — Window handles lower to WID
        // ints; the route string resolves at codegen (manifest). Requires
        // a route argument (a zero-arg user class named Window keeps
        // working); non-literal routes fail later with mx-route-unknown.
        if callee == "Window" && !args.is_empty() {
            let opts = match n.arguments.get(1).and_then(|a| a.as_expression()) {
                Some(Expression::ObjectExpression(obj)) => self.emit_window_opts(obj),
                _ => args.get(1).cloned().unwrap_or_else(|| "JsObject{}".to_string()),
            };
            return format!("__morph_new_window({}, {})", args[0], opts);
        }
        if callee == "Error" {
            let msg = args.first().cloned().unwrap_or_else(|| "JsString(\"\")".to_string());
            self.ctx.need("JsObject");
            let pairs = format!("{{\"name\", JsString(\"Error\")}}, {{\"message\", {}}}", msg);
            return format!("JsObject{{{}}}", pairs);
        }
        // Handle Promise constructor specially
        if callee == "Promise" {
            self.ctx.need("morph::Result");
            // Promise<T> constructor with resolver callback
            // Pattern: new Promise<T>((resolve) => { resolve(value); })
            // Get the type argument from Promise<T>
            let promise_type_arg = if let Some(ta) = &n.type_arguments {
                ta.params
                    .iter()
                    .map(|t| {
                        resolve_type(
                            Some(t),
                            "auto",
                            &self.ctx.template_params,
                            false,
                            &self.ctx.class_names,
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            } else {
                "JsValue".to_string()
            };
            if let Some(arg) = n.arguments.first() {
                if let Some(resolver_arg_expr) = arg.as_expression() {
                    if let Expression::ArrowFunctionExpression(arrow) = resolver_arg_expr {
                        // Check both expression body and block statement body
                        if let Some(body_expr) = arrow.body.as_expression() {
                            if let Expression::CallExpression(call) = body_expr {
                                if let Expression::Identifier(id) = &call.callee {
                                    if id.name.as_str() == "resolve" && !call.arguments.is_empty() {
                                        let resolved_value = self.emit_argument(&call.arguments[0]);
                                        return format!(
                                            "morph::Result<{}>::resolved({})",
                                            promise_type_arg, resolved_value
                                        );
                                    }
                                }
                            }
                        } else if let Some(body_block) = arrow.body.as_function_body() {
                            // Check block statement body for resolve(value) call
                            for stmt in &body_block.statements {
                                if let Statement::ExpressionStatement(es) = stmt {
                                    if let Expression::CallExpression(call) = &es.expression {
                                        if let Expression::Identifier(id) = &call.callee {
                                            if id.name.as_str() == "resolve"
                                                && !call.arguments.is_empty()
                                            {
                                                let resolved_value =
                                                    self.emit_argument(&call.arguments[0]);
                                                return format!(
                                                    "morph::Result<{}>::resolved({})",
                                                    promise_type_arg, resolved_value
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return format!(
                        "morph::Result<{}>::resolved(/* from Promise */ 0)",
                        promise_type_arg
                    );
                }
            }
            return format!("morph::Result<{}>::pending()", promise_type_arg);
        }
        let type_args = if let Some(ta) = &n.type_arguments {
            let args: Vec<String> = ta
                .params
                .iter()
                .map(|t| {
                    resolve_type(
                        Some(t),
                        "auto",
                        &self.ctx.template_params,
                        false,
                        &self.ctx.class_names,
                    )
                })
                .collect();
            format!("<{}>", args.join(", "))
        } else {
            String::new()
        };
        // Class construction goes through the runtime helper so the
        // constructor (including inherited constructors) can read
        // `new.target`. Constructor arguments are evaluated before the
        // target guard is installed, matching JS evaluation order.
        if let Expression::Identifier(id) = Self::unwrap_expr(&n.callee)
            && self.ctx.class_names.contains(id.name.as_str())
        {
            self.ctx.need("JsValue");
            let name = id.name.as_str();
            let mut all_args = vec![format!("\"{name}\"")];
            all_args.extend(args);
            return format!("morph::js_new<{}{}>({})", callee, type_args, all_args.join(", "));
        }
        self.ctx.need("std::make_shared");
        format!("std::make_shared<{}{}>({})", callee, type_args, args.join(", "))
    }

    /// Lower a statement-level `fetch(...).then(cb).catch(cb)` chain to an
    /// awaited fetch plus immediately-invoked callbacks. Returns `None`
    /// for any other expression so normal emission (and the linter's
    /// rejection) applies.
    fn emit_then_chain_statement(&mut self, expr: &Expression<'a>) -> Option<String> {
        if self.ctx.is_async_fn == 0 {
            return None;
        }
        let (fetch_call, then_cb, catch_cb) = parse_then_chain(expr)?;
        self.ctx.needed.insert("\"../../runtime/cpp/net/net.h\"".to_string());
        let args: Vec<String> =
            fetch_call.arguments.iter().map(|a| self.emit_argument(a)).collect();
        let await_expr = format!("co_await morph::net::fetch_response({})", args.join(", "));
        let base = self.indent();
        let inner = format!("{base}{INDENT}");
        let mut lines = Vec::new();
        if let Some(catch_cb) = catch_cb {
            lines.push(format!("{base}try {{"));
            lines.push(format!("{inner}{};", self.emit_then_step(&await_expr, then_cb)));
            self.ctx.need("JsValue");
            lines.push(format!("{base}}} catch (const JsValue& __morph_then_error) {{"));
            let lambda = self.emit_then_callback_scoped(catch_cb, "JsValue");
            let call = if catch_cb.has_param() {
                format!("({lambda})(__morph_then_error)")
            } else {
                format!("({lambda})()")
            };
            lines.push(format!("{inner}{call};"));
            lines.push(format!("{base}}}"));
        } else {
            lines.push(format!("{base}{};", self.emit_then_step(&await_expr, then_cb)));
        }
        Some(lines.join("\n"))
    }

    fn emit_then_step<'b>(
        &mut self,
        await_expr: &str,
        then_cb: Option<ThenCallback<'b, 'a>>,
    ) -> String {
        let Some(cb) = then_cb else {
            return await_expr.to_string();
        };
        let lambda = self.emit_then_callback_scoped(cb, "morph::net::Response");
        if cb.has_param() {
            format!("({lambda})({await_expr})")
        } else {
            // A zero-parameter callback cannot receive the response, but the
            // fetch must still run and suspend before the callback.
            let inner = format!("{}{}", self.indent(), INDENT);
            format!("{await_expr};\n{inner}({lambda})()")
        }
    }

    /// Emit the callback lambda with the enclosing async function hidden:
    /// a `return` inside the callback must return from the lambda, not
    /// `co_return` from the surrounding coroutine.
    fn emit_then_callback_scoped<'b>(
        &mut self,
        cb: ThenCallback<'b, 'a>,
        param_type: &str,
    ) -> String {
        let saved_async = self.ctx.is_async_fn;
        let saved_drop = self.ctx.drop_return_value;
        self.ctx.is_async_fn = 0;
        self.ctx.drop_return_value = false;
        let out = self.emit_then_callback(cb, param_type);
        self.ctx.is_async_fn = saved_async;
        self.ctx.drop_return_value = saved_drop;
        out
    }

    fn emit_then_callback<'b>(&mut self, cb: ThenCallback<'b, 'a>, param_type: &str) -> String {
        if let Some(p) = cb.params().items.first() {
            self.ctx.contextual_param_types.insert(p.span.start, param_type.to_string());
        }
        let out = match cb {
            ThenCallback::Arrow(f) => self.emit_arrow(f),
        };
        if let Some(p) = cb.params().items.first() {
            self.ctx.contextual_param_types.remove(&p.span.start);
        }
        out
    }

    fn emit_arrow(&mut self, f: &ArrowFunctionExpression<'a>) -> String {
        let capture = self.lambda_capture(&f.params);
        // Nested arrows capture enclosing arrow locals (e.g. an effect's
        // timer handle read by its cleanup closure): `lambda_capture`
        // consults this depth, so it is raised before the body emits.
        // The current arrow's own capture above still sees the outer depth.
        self.ctx.arrow_depth += 1;
        let out = self.emit_arrow_body(f, &capture);
        self.ctx.arrow_depth -= 1;
        out
    }

    fn emit_arrow_body(&mut self, f: &ArrowFunctionExpression<'a>, capture: &str) -> String {
        let is_async = f.r#async;
        let params = self.format_params(&f.params);
        let mut ret = if let Some(rt) = &f.return_type {
            resolve_type_annotation(
                Some(rt),
                "auto",
                &self.ctx.template_params,
                false,
                &self.ctx.class_names,
            )
        } else {
            "auto".to_string()
        };
        if is_async {
            ret = self.ctx.async_result_type(&ret);
        }
        self.ctx.need(&ret);
        let is_expr = f.body.as_expression().is_some();
        if self.ctx.event_handler {
            self.ctx.need("JsObject");
            let event_params = if f.params.items.is_empty() {
                "JsObject".to_string()
            } else {
                let mut ps = Vec::new();
                for p in &f.params.items {
                    let (name, _) = self.binding_to_identifier(&p.pattern);
                    self.ctx.var_types.insert(name.clone(), "JsObject".to_string());
                    ps.push(format!("JsObject {}", name));
                }
                ps.join(", ")
            };
            if is_expr {
                if let Some(expr) = f.body.as_expression() {
                    let e = self.emit_expression(expr);
                    return format!("{}( {} ) -> void {{ {}; }}", capture, event_params, e);
                }
                return format!("{}( {} ) -> void {{ }}", capture, event_params);
            } else {
                let saved_drop = self.ctx.drop_return_value;
                self.ctx.drop_return_value = false;
                let body = self.emit_function_body_with_params(
                    f.body.as_function_body().unwrap(),
                    Some(&f.params),
                );
                self.ctx.drop_return_value = saved_drop;
                return format!("{}( {} ) -> void {}", capture, event_params, body);
            }
        }
        if is_expr {
            if let Some(expr) = f.body.as_expression() {
                // A destructured parameter still needs its binding prelude
                // before the expression; compute it inside the lambda scope.
                let prelude = {
                    let old = self.ctx.indent_level;
                    self.ctx.indent_level = old + 1;
                    let prelude = self.destructured_param_prelude(&f.params);
                    self.ctx.indent_level = old;
                    prelude
                };
                let e = self.emit_expression(expr);
                // `() => clearInterval(id)` lowers to the void runtime call
                // `morph::clear_timer(id)`: `-> auto { return <void>; }` is
                // ill-formed, so these arrows are `-> void` with a bare
                // call instead (docs/dev/bugs#4).
                if !is_async && is_void_timer_call(expr) {
                    if let Some(prelude) = prelude {
                        let inner = format!("{}{}", self.indent(), INDENT);
                        return format!(
                            "{capture}({params}) -> void {{\n{prelude}\n{inner}{e};\n{}}}",
                            self.indent()
                        );
                    }
                    return format!("{capture}({params}) -> void {{ {e}; }}");
                }
                let kw = if is_async { "co_return" } else { "return" };
                if let Some(prelude) = prelude {
                    let inner = format!("{}{}", self.indent(), INDENT);
                    return format!(
                        "{capture}({params}) -> {ret} {{\n{prelude}\n{inner}{kw} {e};\n{}}}",
                        self.indent()
                    );
                }
                return format!("{}({}) -> {} {{ {} {}; }}", capture, params, ret, kw, e);
            }
            return format!("{}({}) -> {} {{}}", capture, params, ret);
        } else {
            let saved_drop = self.ctx.drop_return_value;
            self.ctx.drop_return_value = false;
            let body = if is_async {
                self.ctx.is_async_fn += 1;
                let b = self.emit_function_body_with_params(
                    f.body.as_function_body().unwrap(),
                    Some(&f.params),
                );
                self.ctx.is_async_fn -= 1;
                b
            } else {
                self.emit_function_body_with_params(
                    f.body.as_function_body().unwrap(),
                    Some(&f.params),
                )
            };
            self.ctx.drop_return_value = saved_drop;
            return format!("{}({}) -> {} {}", capture, params, ret, body);
        }
    }

    /// True when a closure in the current function scope captures `name`.
    /// Loop lowering uses this to decide whether a `let`/`const` binding
    /// needs a fresh per-iteration box.
    fn is_captured_name(&self, name: &str) -> bool {
        let scope = self.ctx.current_fn.clone().unwrap_or_default();
        self.analysis
            .as_ref()
            .and_then(|analysis| analysis.closure_captures.get(&scope))
            .is_some_and(|captured| captured.contains(name))
    }

    /// Capture list for a lambda. Shared locals the enclosing function
    /// captures are held by value (`[&, count]`) so an escaping closure
    /// keeps its state alive; everything else keeps today's default.
    /// Names are sorted so output is deterministic across runs. File-scope
    /// lambdas stay `[]` unless nested inside another arrow body, where
    /// enclosing arrow locals (an effect's timer handle, say) must capture
    /// like any function-local lambda (docs/dev/bugs#4). Namespace scope
    /// forbids captures, and file statics are visible without them.
    /// Parameters shadowing an outer name are never captured: the body
    /// reads the parameter.
    fn lambda_capture(&self, params: &FormalParameters<'a>) -> String {
        if self.ctx.fn_body_depth == 0 && self.ctx.arrow_depth == 0 {
            return "[]".to_string();
        }
        let scope = self.ctx.current_fn.clone().unwrap_or_default();
        let mut held: Vec<String> = self
            .analysis
            .as_ref()
            .and_then(|analysis| analysis.closure_captures.get(&scope).cloned())
            .unwrap_or_default()
            .into_iter()
            .filter(|var| self.ctx.shared_ptr_vars.contains(var))
            .filter(|var| !Self::params_bind_name(params, var))
            .collect();
        held.sort();
        if held.is_empty() {
            return "[&]".to_string();
        }
        format!("[&, {}]", held.join(", "))
    }

    fn emit_function_expression(&mut self, f: &Function<'a>) -> String {
        self.ctx.fn_expr_depth += 1;
        // Function expressions own their `new.target`: Morph has no `new`
        // lowering for them, so the target is always undefined here.
        let old_in_constructor = self.ctx.in_constructor;
        self.ctx.in_constructor = false;
        let is_async = f.r#async;
        let params = self.format_params(&f.params);
        let ret = if is_async {
            let base = f
                .return_type
                .as_ref()
                .map(|rt| {
                    resolve_type_annotation(
                        Some(rt),
                        "JsValue",
                        &self.ctx.template_params,
                        false,
                        &self.ctx.class_names,
                    )
                })
                .unwrap_or_else(|| "JsValue".to_string());
            self.ctx.async_result_type(&base)
        } else if let Some(rt) = &f.return_type {
            resolve_type_annotation(
                Some(rt),
                "auto",
                &self.ctx.template_params,
                false,
                &self.ctx.class_names,
            )
        } else {
            "JsValue".to_string()
        };

        self.ctx.need(&ret);
        let saved_drop = self.ctx.drop_return_value;
        self.ctx.drop_return_value = false;
        let body = if let Some(b) = &f.body {
            if is_async {
                self.ctx.is_async_fn += 1;
                let code = self.emit_function_body_with_params(b, Some(&f.params));
                self.ctx.is_async_fn -= 1;
                code
            } else {
                self.emit_function_body_with_params(b, Some(&f.params))
            }
        } else {
            "{}".to_string()
        };
        self.ctx.drop_return_value = saved_drop;
        self.ctx.fn_expr_depth -= 1;
        self.ctx.in_constructor = old_in_constructor;
        self.ctx.need("JsValue");
        let lambda_params = if params.is_empty() {
            "JsValue _jsThis".to_string()
        } else {
            format!("{}, JsValue _jsThis", params)
        };
        if is_async {
            format!("+[]({}) -> {} {}", lambda_params, ret, body)
        } else {
            format!("+[]({}) -> {} {}", lambda_params, ret, body)
        }
    }

    fn extract_template_parts(&mut self, t: &TemplateLiteral<'a>) -> (String, Vec<String>) {
        let mut fmt_str = String::new();
        let mut args = Vec::new();
        for (i, quasi) in t.quasis.iter().enumerate() {
            let text =
                quasi.value.cooked.as_ref().map(|c| c.as_str()).unwrap_or(quasi.value.raw.as_str());
            fmt_str.push_str(&text.replace('{', "{{").replace('}', "}}"));
            if let Some(expr) = t.expressions.get(i) {
                fmt_str.push_str("{}");
                args.push(self.emit_expression(expr));
            }
        }
        (escape_cpp_text(&fmt_str), args)
    }

    fn property_key_to_string(&self, key: &PropertyKey<'a>) -> Option<String> {
        match key {
            PropertyKey::StaticIdentifier(id) => Some(id.name.to_string()),
            PropertyKey::StringLiteral(s) => Some(s.value.to_string()),
            PropertyKey::NumericLiteral(n) => Some(n.value.to_string()),
            PropertyKey::PrivateIdentifier(p) => Some(p.name.to_string()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate_stmts(source: &str) -> String {
        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        let allocator = Allocator::default();
        let source_type =
            oxc_span::SourceType::from_path("file.ts").unwrap_or_default().with_typescript(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        assert!(parsed.diagnostics.is_empty());
        let mut t = CppTranslator::new(source, 0, crate::codegen::context::TypeMode::Infer, None);
        let mut out = Vec::new();
        for stmt in &parsed.program.body {
            if let Some(code) = t.emit_statement(stmt) {
                out.push(code);
            }
        }
        out.join("\n")
    }

    /// Like [`translate_stmts`], but runs the whole-program analysis passes
    /// (late Window handles, escape analysis, array receivers).
    fn translate_analyzed(source: &str) -> String {
        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        use oxc_span::SourceType;
        let allocator = Allocator::default();
        let source_type =
            SourceType::from_path("file.ts").unwrap_or_default().with_typescript(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        assert!(parsed.diagnostics.is_empty());
        let mut t = CppTranslator::new(source, 0, TypeMode::Infer, None);
        t.analyze_program(&parsed.program);
        let mut out = Vec::new();
        for stmt in &parsed.program.body {
            if let Some(code) = t.emit_statement(stmt) {
                out.push(code);
            }
        }
        out.join("\n")
    }

    #[test]
    fn object_spread_and_computed_keys_build_at_runtime() {
        let out = translate_stmts(
            "const base = { a: 1 };\n\
             const key = \"b\";\n\
             const merged = { ...base, z: 3, [key]: 2 };",
        );
        assert!(out.contains("morph::js_object_assign(__o, base)"), "{out}");
        assert!(out.contains("__o.set(\"z\", 3)"), "{out}");
        assert!(out.contains("morph::js_to_string(key).to_std_string()"), "{out}");
    }

    #[test]
    fn exponentiation_and_logical_assignment_lower_with_js_semantics() {
        let out = translate_stmts(
            "let n = 2;\n\
             n **= 3;\n\
             n &&= 4;\n\
             n ||= 5;\n\
             n ??= 6;\n\
             const p = 2 ** 8;",
        );
        assert!(out.contains("std::pow(morph::js_to_number(n)"), "{out}");
        assert!(out.contains("morph::js_to_boolean(n)"), "{out}");
        assert!(out.contains("morph::js_is_nullish(n)"), "{out}");
        assert!(out.contains("std::pow(morph::js_to_number(2)"), "{out}");
    }

    #[test]
    fn math_constants_and_extended_functions_lower() {
        let out = translate_stmts(
            "const pi = Math.PI;\n\
             const e = Math.E;\n\
             const fixed = Math.PI.toFixed(2);\n\
             const t = Math.trunc(1.7);\n\
             const s = Math.sign(-3);\n\
             const l = Math.log2(8);\n\
             const a = Math.atan2(1, 1);\n\
             const h = Math.hypot(3, 4, 12);\n\
             const r = Math.random();",
        );
        assert!(out.contains("morph::js_math::PI"), "{out}");
        assert!(out.contains("morph::js_math::E"), "{out}");
        assert!(out.contains("JsNumber(morph::js_math::PI).toFixed(2)"), "{out}");
        assert!(out.contains("morph::js_math::trunc(1.7)"), "{out}");
        assert!(out.contains("morph::js_math::sign(-3)"), "{out}");
        assert!(out.contains("morph::js_math::log2(8)"), "{out}");
        assert!(out.contains("morph::js_math::atan2(1, 1)"), "{out}");
        assert!(out.contains("morph::js_math::hypot({3, 4, 12})"), "{out}");
        assert!(out.contains("morph::js_math::random()"), "{out}");
    }

    #[test]
    fn then_chain_lowers_to_awaited_fetch() {
        let out = translate_stmts(
            "async function load() {\n\
                 fetch(\"http://example.com\").then((response) => console.log(response.text())).catch((error) => console.log(error));\n\
             }",
        );
        assert!(
            out.contains("co_await morph::net::fetch_response"),
            "awaited fetch response: {out}"
        );
        assert!(out.contains("morph::net::Response response"), "then param typed: {out}");
        assert!(out.contains("catch (const JsValue& __morph_then_error)"), "catch clause: {out}");
        assert!(out.contains("JsValue error"), "catch param typed: {out}");
        assert!(!out.contains(".then("), "no then call: {out}");
        assert!(!out.contains(".catch("), "no catch call: {out}");
        // The awaited fetch is the IIFE argument, so it completes before the
        // callback body runs; the catch clause wraps both.
        assert!(
            out.contains(")(co_await morph::net::fetch_response"),
            "fetch resolves before the then callback: {out}"
        );
        let then_at = out.find("response.text()").expect("then body");
        let catch_at = out.find("catch (const JsValue&").expect("catch clause");
        assert!(then_at < catch_at, "catch after then: {out}");
    }

    #[test]
    fn then_chain_callbacks_return_from_the_lambda() {
        let out = translate_stmts(
            "async function load() {\n\
                 fetch(\"http://example.com\").then((r) => { console.log(r.text()); return 1; });\n\
             }",
        );
        assert!(out.contains("return 1;"), "lambda return: {out}");
        assert!(!out.contains("co_return 1;"), "not a coroutine return: {out}");
        assert!(!out.contains("try {"), "then-only chain needs no catch: {out}");
    }

    #[test]
    fn then_chain_zero_param_callback_still_awaits_fetch() {
        let out = translate_stmts(
            "async function load() {\n\
                 fetch(\"http://example.com\").then(() => console.log(\"ok\"));\n\
             }",
        );
        assert!(out.contains("co_await morph::net::fetch_response"), "awaited fetch: {out}");
        assert!(out.contains("})()"), "zero-param callback called: {out}");
    }

    #[test]
    fn then_chain_with_on_rejected_callback_is_not_lowered() {
        // The second `.then` argument is the onRejected callback; the inline
        // lowering would drop it, so it must not parse as a supported chain.
        let out = translate_stmts(
            "async function load() {\n\
                 fetch(\"http://example.com\").then((r) => console.log(r.text()), (e) => console.log(e));\n\
             }",
        );
        assert!(!out.contains("__morph_then_error"), "not lowered as a chain: {out}");
        assert!(out.contains(".then("), "left for the linter rejection: {out}");
    }

    #[test]
    fn catch_only_chain_awaits_fetch_in_try() {
        let out = translate_stmts(
            "async function load() {\n\
                 fetch(\"http://example.com\").catch((error) => console.log(error));\n\
             }",
        );
        assert!(out.contains("try {"), "try block: {out}");
        assert!(out.contains("co_await morph::net::fetch_response"), "awaited fetch: {out}");
        assert!(out.contains("catch (const JsValue& __morph_then_error)"), "catch clause: {out}");
        assert!(!out.contains("morph::net::Response"), "no then param: {out}");
    }

    #[test]
    fn response_json_requests_full_response() {
        let out = translate_stmts(
            "async function load() {\n\
                 const response = await fetch(\"http://example.com\");\n\
                 const data = response.json();\n\
                 console.log(data);\n\
             }",
        );
        assert!(
            out.contains("morph::net::Response response = co_await morph::net::fetch_response"),
            "{out}"
        );
        assert!(out.contains("response.json()"), "{out}");
    }

    #[test]
    fn awaited_response_json_is_synchronous() {
        let out = translate_stmts(
            "async function load() {\n\
                 const response = await fetch(\"http://example.com\");\n\
                 const data = await response.json();\n\
                 console.log(data);\n\
             }",
        );
        assert!(out.contains("co_await morph::net::fetch"), "{out}");
        assert!(!out.contains("co_await response.json()"), "{out}");
        assert!(out.contains("response.json()"), "{out}");
    }

    #[test]
    fn json_parse_dot_access_lowers_to_bracket() {
        let out = translate_stmts(
            "const inline = JSON.parse(\"{\\\"name\\\":\\\"Ada\\\"}\").name;\n\
             const parsed = JSON.parse(\"{\\\"name\\\":\\\"Ada\\\"}\");\n\
             console.log(inline, parsed.name, parsed[\"name\"]);",
        );
        assert!(out.contains("morph::json_parse"), "{out}");
        assert!(out.contains("[\"name\"]"), "{out}");
        assert!(!out.contains(".name"), "{out}");
    }

    #[test]
    fn response_json_dot_access_lowers_to_bracket() {
        let out = translate_stmts(
            "async function load() {\n\
                 const response = await fetch(\"http://example.com\");\n\
                 const data = await response.json();\n\
                 console.log(response.json().name, data.name);\n\
             }",
        );
        assert!(out.contains("response.json()[\"name\"]"), "{out}");
        assert!(out.contains("data[\"name\"]"), "{out}");
        assert!(!out.contains("data.name"), "{out}");
    }

    #[test]
    fn lone_json_parse_pulls_the_runtime_header() {
        let out = crate::translate_default(
            "const data = JSON.parse(\"{\\\"name\\\":\\\"Ada\\\"}\");\nconsole.log(data);",
            "main.ts",
        )
        .expect("translate");
        assert!(out.contains("morph::json_parse"), "{out}");
        assert!(out.contains("js_types.h") || out.contains("js_json.h"), "{out}");
    }

    #[test]
    fn window_placeholders_lower() {
        let out = translate_stmts(
            "const w = new Window(\"/settings\", { width: 500 });\n\
             const c = useWindow();\n\
             const d = useWindow(\"login-a\");\n\
             w.navigate(\"/auth/login\", { x: 1 });\n\
             w.close();\n\
             w.show();\n\
             w.hide();\n\
             w.on(\"close\", () => {});\n\
             const shut = w.closed;\n\
             const title = w.title;\n\
             w.title = \"hi\";",
        );
        assert!(out.contains("__morph_new_window(\"/settings\""), "{out}");
        assert!(out.contains("__morph_current_window()"), "{out}");
        assert!(out.contains("__morph_use_window(\"login-a\")"), "{out}");
        assert!(out.contains("__morph_win_navigate(w,"), "{out}");
        assert!(out.contains("WindowManager::get().close(w)"), "{out}");
        assert!(out.contains("WindowManager::get().open(w)"), "{out}");
        assert!(out.contains("WindowManager::get().hide(w)"), "{out}");
        assert!(out.contains("WindowManager::get().onClose(w,"), "{out}");
        assert!(out.contains("__morph_win_closed(w)"), "{out}");
        assert!(out.contains("__morph_win_title(w)"), "{out}");
        assert!(out.contains("__morph_win_set_title(w,"), "{out}");
    }

    #[test]
    fn late_window_handles_keep_js_values_and_narrow_at_use_sites() {
        let out = translate_analyzed(
            "let win;\n\
             console.log(win);\n\
             win = new Window(\"/settings\");\n\
             win.title = \"ready\";\n\
             win.close();\n\
             const shut = win.closed;\n\
             const child = new Window(\"/login\", { parent: win });",
        );
        assert!(out.contains("JsWindowHandle win{}"), "{out}");
        assert!(out.contains("std::println(\"{}\", win)"), "{out}");
        assert!(out.contains("win = __morph_new_window(\"/settings\""), "{out}");
        assert!(out.contains("__morph_win_set_title(window_wid(win), \"ready\")"), "{out}");
        assert!(out.contains("WindowManager::get().close(window_wid(win))"), "{out}");
        assert!(out.contains("__morph_win_closed(window_wid(win))"), "{out}");
        assert!(out.contains("{\"parent\", window_wid(win)}"), "{out}");
    }

    #[test]
    fn late_window_nullish_and_typed_declarations_use_js_handles() {
        for source in [
            "let win: Window;\nwin = useWindow();\nwin.title = \"typed\";",
            "let win = null;\nwin = useWindow();\nwin.title = \"nullish\";",
            "let win = undefined;\nwin = new Window(\"/settings\");\nwin.close();",
        ] {
            let out = translate_analyzed(source);
            assert!(out.contains("JsWindowHandle win"), "{source}: {out}");
            assert!(out.contains("window_wid(win)"), "{source}: {out}");
            assert!(!out.contains("WID win"), "{source}: {out}");
        }
    }

    #[test]
    fn late_values_keep_js_semantics() {
        let out = translate_analyzed(
            "let n;\n\
             console.log(n);\n\
             n = 5;\n\
             console.log(n + 1);\n\
             let s;\n\
             s = \"hi\";\n\
             console.log(s + \"!\");\n\
             console.log(s.toUpperCase());\n\
             let obj;\n\
             obj = { name: \"ada\" };\n\
             console.log(obj.name);\n\
             let z = null;\n\
             z = 7;\n\
             console.log(z - 2);",
        );
        assert!(out.contains("JsLateValue n{}"), "{out}");
        assert!(out.contains("JsLateValue s{}"), "{out}");
        assert!(out.contains("JsLateValue obj{}"), "{out}");
        assert!(out.contains("JsLateValue z = JsNull{}"), "{out}");
        assert!(out.contains("std::println(\"{}\", n)"), "{out}");
        assert!(out.contains("(n = 5)"), "{out}");
        assert!(out.contains("morph::js_late_add(n, 1)"), "{out}");
        assert!(!out.contains("std::get<JsNumber>(n.inner)"), "{out}");
        assert!(!out.contains("std::get<JsNumber>(s.inner)"), "{out}");
        assert!(out.contains("morph::js_late_add(s, JsString(\"!\"))"), "{out}");
        assert!(out.contains("morph::js_late_sub(z, 2)"), "{out}");
        assert!(out.contains("obj[\"name\"]"), "{out}");
    }

    #[test]
    fn conflicting_assignment_classes_widen_to_late_values() {
        let out = translate_analyzed(
            "let x = 0;\n\
             x = \"hi\";\n\
             console.log(x + \"!\");",
        );
        assert!(out.contains("JsLateValue x = 0"), "{out}");
        assert!(out.contains("x = \"hi\""), "{out}");
        assert!(out.contains("morph::js_late_add(x, JsString(\"!\"))"), "{out}");
    }

    #[test]
    fn compound_assignments_widen_when_the_result_class_changes() {
        let out = translate_analyzed(
            "let label = 0;\n\
             label += \"!\";\n\
             let n = 0;\n\
             n += 1.5;\n\
             console.log(label, n);",
        );
        assert!(out.contains("JsLateValue label = 0"), "{out}");
        assert!(out.contains("JsNumber n = 0"), "{out}");
    }

    #[test]
    fn dynamic_annotations_widen_even_with_initializers() {
        let out = translate_analyzed(
            "let u: number | string = 0;\n\
             u = \"hi\";\n\
             let a: any = 1;\n\
             a = false;\n\
             console.log(u, a);",
        );
        assert!(out.contains("JsLateValue u = 0"), "{out}");
        assert!(out.contains("JsLateValue a = 1"), "{out}");
    }

    #[test]
    fn numeric_assignment_widening_keeps_fractional_values() {
        let out = translate_analyzed(
            "let n = 0;\n\
             n = 1.5;\n\
             console.log(n);",
        );
        assert!(out.contains("JsNumber n = 0"), "{out}");
        assert!(!out.contains("int64_t n"), "{out}");
    }

    #[test]
    fn late_value_operators_lower_without_numeric_narrowing() {
        let out = translate_analyzed(
            "let x;\n\
             x = 5;\n\
             x++;\n\
             x += 2;\n\
             x *= 3;\n\
             x &= 4;\n\
             console.log(-x);\n\
             console.log(~x);",
        );
        assert!(out.contains("JsLateValue x{}"), "{out}");
        assert!(out.contains("x++"), "{out}");
        assert!(out.contains("(x += 2)"), "{out}");
        assert!(out.contains("std::println(\"{}\", -x)"), "{out}");
        assert!(out.contains("std::println(\"{}\", ~x)"), "{out}");
        assert!(!out.contains("std::get<JsNumber>"), "{out}");
    }

    #[test]
    fn late_value_logical_assignments_keep_js_value_type() {
        let out = translate_analyzed(
            "let y = null;\n\
             y ||= \"set\";\n\
             let z;\n\
             z ??= 9;\n\
             z++;\n\
             console.log(z);",
        );
        assert!(out.contains("JsLateValue y = JsNull{}"), "{out}");
        assert!(out.contains("JsLateValue z{}"), "{out}");
        assert!(out.contains("JsValue(\"set\")"), "{out}");
        assert!(out.contains("JsValue(9)"), "{out}");
        assert!(out.contains("z++"), "{out}");
    }

    #[test]
    fn module_level_late_window_handles_lower() {
        let out = translate_analyzed(
            "let win;\n\
             console.log(win);\n\
             win = new Window(\"/settings\");\n\
             win.title = \"late\";\n\
             win.close();",
        );
        assert!(out.contains("JsWindowHandle win{}"), "{out}");
        assert!(out.contains("std::println(\"{}\", win)"), "{out}");
        assert!(out.contains("__morph_win_set_title(window_wid(win), \"late\")"), "{out}");
        assert!(out.contains("WindowManager::get().close(window_wid(win))"), "{out}");
    }

    #[test]
    fn direct_window_initializers_keep_wid_lowering() {
        let out = translate_analyzed(
            "const current = useWindow();\n\
             current.title = \"hi\";\n\
             const fresh = new Window(\"/settings\");\n\
             fresh.close();",
        );
        assert!(out.contains("WID current = __morph_current_window()"), "{out}");
        assert!(out.contains("WID fresh = __morph_new_window(\"/settings\""), "{out}");
        assert!(out.contains("__morph_win_set_title(current, \"hi\")"), "{out}");
        assert!(out.contains("WindowManager::get().close(fresh)"), "{out}");
        assert!(!out.contains("window_wid(current)"), "{out}");
        assert!(!out.contains("window_wid(fresh)"), "{out}");
    }

    #[test]
    fn window_lazy_placeholders_lower_reads_writes_and_calls() {
        let out = translate_stmts(
            "const current = __morph_current_window().title;\n\
             __morph_current_window().title = \"current\";\n\
             (__morph_current_window()).close();\n\
             const other = __morph_use_window(\"login-a\").closed;\n\
             __morph_use_window(\"login-a\").title = \"other\";\n\
             useWindow().title = \"direct\";\n\
             useWindow().close();\n\
             const direct = useWindow().title;\n\
             new Window(\"/settings\").title = \"fresh\";",
        );
        assert!(out.contains("__morph_win_title(__morph_current_window())"), "{out}");
        assert!(
            out.contains("__morph_win_set_title(__morph_current_window(), \"current\")"),
            "{out}"
        );
        assert!(out.contains("WindowManager::get().close(__morph_current_window())"), "{out}");
        assert!(out.contains("__morph_win_closed(__morph_use_window(\"login-a\"))"), "{out}");
        assert!(
            out.contains("__morph_win_set_title(__morph_use_window(\"login-a\"), \"other\")"),
            "{out}"
        );
        assert!(
            out.contains("__morph_win_set_title(__morph_current_window(), \"direct\")"),
            "{out}"
        );
        assert!(out.contains("WindowManager::get().close(__morph_current_window())"), "{out}");
        assert!(out.contains("__morph_win_title(__morph_current_window())"), "{out}");
        assert!(out.contains("__morph_win_set_title(__morph_new_window(\"/settings\""), "{out}");
    }

    #[test]
    fn window_typed_params_lower_handle_members() {
        let out = translate_stmts(
            "function retitle(w: Window) {\n\
                 w.title = \"hi\";\n\
                 w.navigate(\"/settings\");\n\
                 const shut = w.closed;\n\
             }\n\
             function retitleDialog(dialog: Dialog) {\n\
                 dialog.title = \"hi\";\n\
             }",
        );
        assert!(out.contains("void retitle(WID w)"), "{out}");
        assert!(out.contains("__morph_win_set_title(w, \"hi\")"), "{out}");
        assert!(out.contains("__morph_win_navigate(w,"), "{out}");
        assert!(out.contains("__morph_win_closed(w)"), "{out}");
        assert!(!out.contains("void retitleDialog(WID"), "{out}");
        assert!(out.contains("dialog.title = \"hi\""), "{out}");
    }

    #[test]
    fn window_ownership_opts_lower_to_ints() {
        let out = translate_stmts(
            "const main = useWindow();\n\
             const a = new Window(\"/settings\", { parent: main, modal: true, role: \"popup\", width: 500 });\n\
             const b = new Window(\"/settings\", { parent: \"login-a\" });\n\
             const c = new Window(\"/settings\", { parent: \"/auth/login\" });\n\
             const d = new Window(\"/settings\", { parent: \"auto\" });\n\
             const e = new Window(\"/settings\", { parent: null });\n\
             const f = new Window(\"/settings\", { role: \"dialog\" });\n",
        );
        assert!(out.contains("{\"parent\", main}"), "{out}");
        assert!(out.contains("{\"modal\", true}"), "{out}");
        assert!(out.contains("{\"role\", 2}"), "{out}");
        assert!(out.contains("{\"width\", 500}"), "{out}");
        assert!(out.contains("widForAlias(\"login-a\")"), "{out}");
        assert!(out.contains("__morph_route_wid(\"/auth/login\")"), "{out}");
        assert!(out.contains("focusedWid()"), "{out}");
        assert!(out.contains("{\"parent\", kInvalidWid}"), "{out}");
        assert!(out.contains("{\"role\", 1}"), "{out}");
    }

    #[test]
    fn window_ownership_type_violations_are_build_errors() {
        for bad in [
            "new Window(\"/s\", { modal: \"yes\" });",
            "new Window(\"/s\", { role: \"sheet\" });",
            "new Window(\"/s\", { role: 42 });",
        ] {
            let out = translate_stmts(&format!("const w = {bad}\n"));
            assert!(out.contains("static_assert"), "{bad}: {out}");
            assert!(out.contains("mx-windowconfig-type"), "{bad}: {out}");
        }
    }

    #[test]
    fn non_window_objects_keep_default_emission() {
        let out = translate_stmts("dialog.close();\nconst t = dialog.title;");
        assert!(!out.contains("WindowManager"), "{out}");
        assert!(!out.contains("__morph_"), "{out}");
    }

    #[test]
    fn for_in_over_object_iterates_sorted_keys() {
        use oxc_allocator::Allocator;
        use oxc_parser::Parser;
        let allocator = Allocator::default();
        let source = "for (const k in obj) { use(k); }";
        let source_type =
            oxc_span::SourceType::from_path("file.ts").unwrap_or_default().with_typescript(true);
        let parsed = Parser::new(&allocator, source, source_type).parse();
        assert!(parsed.diagnostics.is_empty());
        let stmt = parsed.program.body.first().expect("one statement");
        let Statement::ForInStatement(f) = stmt else {
            panic!("expected for-in, got {stmt:?}");
        };
        let mut t = CppTranslator::new(source, 0, crate::codegen::context::TypeMode::Infer, None);
        t.ctx.var_types.insert("obj".to_string(), "JsObject".to_string());
        let out = t.emit_for_in(f);
        assert!(out.contains("obj.sorted_keys()"), "{out}");
        assert!(!out.contains(".keys()"), "{out}");
    }

    #[test]
    fn for_in_over_array_iterates_indices_as_strings() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 const xs = [10, 20];\n\
                 for (const index in xs) {\n\
                     console.log(index);\n\
                 }\n\
             }",
        );
        assert!(out.contains("const auto& __morph_for_in_seq_0 = xs;"), "{out}");
        assert!(
            out.contains(
                "for (std::size_t __morph_for_in_index_0 = 0; __morph_for_in_index_0 < __morph_for_in_seq_0.size(); ++__morph_for_in_index_0) {"
            ),
            "{out}"
        );
        assert!(
            out.contains("std::string index = std::to_string(__morph_for_in_index_0);"),
            "{out}"
        );
        assert!(!out.contains("for (auto index : xs)"), "{out}");
    }

    #[test]
    fn captured_array_for_in_boxes_the_index_key() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 const xs = [10, 20];\n\
                 for (const key in xs) {\n\
                     const read = () => key;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        assert!(
            out.contains(
                "std::shared_ptr<std::string> key = std::make_shared<std::string>(std::to_string(__morph_for_in_index_0));"
            ),
            "{out}"
        );
        assert!(out.contains("auto read = [&, key]() -> auto { return (*key); };"), "{out}");
    }

    #[test]
    fn for_in_over_string_literal_iterates_index_keys() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 for (const index in \"ab\") {\n\
                     console.log(index);\n\
                 }\n\
             }",
        );
        assert!(out.contains("std::string __morph_for_in_seq_0 = \"ab\";"), "{out}");
        assert!(
            out.contains("std::string index = std::to_string(__morph_for_in_index_0);"),
            "{out}"
        );
        assert!(!out.contains("for (auto index : \"ab\")"), "{out}");
    }

    #[test]
    fn for_in_over_typed_string_iterates_index_keys() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 const label: string = \"ab\";\n\
                 for (const index in label) {\n\
                     console.log(index);\n\
                 }\n\
             }",
        );
        assert!(out.contains("std::string __morph_for_in_seq_0 = label;"), "{out}");
        assert!(!out.contains("for (auto index : label)"), "{out}");
    }

    #[test]
    fn quotes_and_backslashes_escape() {
        assert_eq!(cpp_string_literal("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(cpp_string_literal("a\\b"), "\"a\\\\b\"");
    }

    #[test]
    fn newlines_and_controls_escape() {
        assert_eq!(cpp_string_literal("a\nb"), "\"a\\nb\"");
        assert_eq!(cpp_string_literal("a\rb\tc"), "\"a\\rb\\tc\"");
        assert_eq!(cpp_string_literal("a\x01b"), "\"a\\u0001b\"");
    }

    #[test]
    fn raw_template_text_keeps_valid_escapes() {
        assert_eq!(escape_cpp_template_raw("a\\nb"), "a\\nb");
        assert_eq!(escape_cpp_template_raw("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_cpp_template_raw("a\nb"), "a\\nb");
        assert_eq!(escape_cpp_template_raw("a\\`b"), "a`b");
    }

    #[test]
    fn char_literal_escapes() {
        assert_eq!(cpp_char_literal("a"), "'a'");
        assert_eq!(cpp_char_literal("\n"), "'\\n'");
        assert_eq!(cpp_char_literal("'"), "'\\''");
    }

    #[test]
    fn clear_interval_expression_arrow_is_void() {
        // `() => clearInterval(id)` lowers to the void runtime call
        // `morph::clear_timer`: emitting `-> auto { return <void>; }`
        // would not compile (docs/dev/bugs#4).
        let out = translate_stmts("const cleanup = () => clearInterval(id);\n");
        assert!(out.contains("[]() -> void { morph::clear_timer(id); }"), "{out}");
        assert!(!out.contains("return morph::clear_timer"), "{out}");
    }

    #[test]
    fn class_fields_infer_private_and_parameter_properties() {
        let out = translate_stmts(
            "class Counter {\n\
               count = 0\n\
               label = \"hits\"\n\
               static total = 0\n\
               #secret = \"s\"\n\
               constructor(public name: string, private step: number = 1) { Counter.total += 1 }\n\
               bump(): number { this.count += this.step; return this.count }\n\
               reveal(): string { return this.#secret }\n\
               static get all(): number { return Counter.total }\n\
               static set all(v: number) { Counter.total = v }\n\
             }",
        );
        // `this.count += this.step` makes count a JsNumber; `total` widens
        // because the static setter accepts a number.
        assert!(out.contains("JsNumber count = 0;"), "{out}");
        assert!(out.contains("std::string label = \"hits\";"), "{out}");
        assert!(out.contains("static inline JsNumber total = 0;"), "{out}");
        assert!(out.contains("JsString name;"), "{out}");
        assert!(out.contains("JsNumber step;"), "{out}");
        assert!(out.contains("JsNumber p_step = 1"), "{out}");
        assert!(out.contains("std::string __private_Counter_secret = \"s\";"), "{out}");
        assert!(out.contains("return this->__private_Counter_secret;"), "{out}");
    }

    #[test]
    fn new_target_lowers_in_constructors() {
        let out = translate_stmts(
            "class Base {\n\
               same = false\n\
               constructor(label: string = new.target === Base ? \"base\" : \"other\") {\n\
                 this.same = new.target === Base\n\
               }\n\
             }\n\
             class Derived extends Base {}\n\
             const b = new Base()\n\
             const d = new Derived()",
        );
        assert!(out.contains("morph::js_new<Base>(\"Base\")"), "{out}");
        assert!(out.contains("morph::js_new<Derived>(\"Derived\")"), "{out}");
        assert!(out.contains("morph::js_new_target()"), "{out}");
        assert!(out.contains("morph::js_constructor_value<Base>(\"Base\")"), "{out}");
    }

    #[test]
    fn new_target_in_plain_functions_and_methods_is_undefined() {
        let out = translate_stmts(
            "class Box {\n\
               method(): any { return new.target }\n\
             }\n\
             function plain(): any { return new.target }",
        );
        assert!(out.contains("return JsUndefined{};"), "{out}");
        assert!(!out.contains("morph::js_new_target()"), "{out}");
    }

    #[test]
    fn new_target_field_initializers_are_undefined() {
        let out = translate_stmts(
            "class Box {\n\
               instance = new.target\n\
               static shared = new.target\n\
             }",
        );
        assert!(out.contains("JsValue instance = JsUndefined{};"), "{out}");
        assert!(out.contains("static inline JsValue shared = JsUndefined{};"), "{out}");
        assert!(!out.contains("morph::js_new_target()"), "{out}");
    }

    #[test]
    fn new_target_arrows_inherit_and_function_expressions_clear() {
        let arrow = translate_stmts(
            "class Box {\n\
               constructor() { const f = () => new.target }\n\
             }",
        );
        assert!(arrow.contains("[&]() -> auto { return morph::js_new_target(); }"), "{arrow}");

        let function = translate_stmts(
            "class Box {\n\
               constructor() { const f = function (): any { return new.target } }\n\
             }",
        );
        assert!(function.contains("return JsUndefined{};"), "{function}");
        assert!(!function.contains("morph::js_new_target()"), "{function}");
    }

    #[test]
    fn class_identifier_comparisons_use_constructor_values() {
        let out = translate_stmts(
            "class Box {}\nfunction same(value: any): boolean { return value === Box }",
        );
        assert!(out.contains("morph::js_constructor_value<Box>(\"Box\")"), "{out}");
        assert!(out.contains("morph::js_cmp::strict_eq(value,"), "{out}");
    }

    #[test]
    fn dynamic_field_annotations_widen_to_late_values() {
        let out = translate_stmts(
            "class Mixed {\n  value = 0\n  set(next: number | string) { this.value = next }\n}\n",
        );
        assert!(out.contains("JsLateValue value = 0;"), "{out}");
    }

    #[test]
    fn class_expressions_and_accessors_lower_through_methods() {
        let out = translate_stmts(
            "class Box {\n\
               value = 0\n\
               get current(): number { return this.value }\n\
               set current(v: number) { this.value = v }\n\
             }\n\
             const Pair = class {\n\
               left = 1\n\
               right = 2\n\
               sum(): number { return this.left + this.right }\n\
             }\n\
             const b = new Box()\n\
             b.current = 5\n\
             b.current += 2\n\
             b.current++\n\
             const read = b.current\n\
             const pair = new Pair()",
        );
        assert!(out.contains("class Pair {"), "{out}");
        assert!(out.contains("__morph_recv->current(__morph_value)"), "{out}");
        assert!(out.contains("auto __morph_value = (__morph_recv->current()) + (2)"), "{out}");
        assert!(out.contains("auto __morph_old = (__morph_recv->current())"), "{out}");
        assert!(out.contains("auto read = b->current()"), "{out}");
        assert!(out.contains("morph::js_new<Pair>(\"Pair\")"), "{out}");
    }

    #[test]
    fn static_accessors_use_class_scope() {
        let out = translate_stmts(
            "class Config {\n\
               static value = 0\n\
               static get current(): number { return Config.value }\n\
               static set current(v: number) { Config.value = v }\n\
             }\n\
             Config.current = 3\n\
             const v = Config.current",
        );
        assert!(out.contains("static JsNumber current()"), "{out}");
        assert!(out.contains("static void current(JsNumber v)"), "{out}");
        assert!(out.contains("Config::current(__morph_value)"), "{out}");
        assert!(out.contains("Config::current()"), "{out}");
    }

    #[test]
    fn destructuring_assignment_lowers_to_staged_lambda() {
        let out = translate_analyzed(
            "let a = 0;\n\
             let b = 0;\n\
             [a, b = 5] = [1, undefined];\n\
             const user = { name: \"Ada\", email: \"a@b.c\" };\n\
             let name = \"\";\n\
             let rest = {};\n\
             ({ name, ...rest } = user);",
        );
        assert!(out.contains("([]() {"), "{out}");
        assert!(out.contains("auto __assign_0 = JsArray{1, JsUndefined{}};"), "{out}");
        assert!(out.contains("a = __assign_0[0];"), "{out}");
        assert!(out.contains("is_undefined() ? JsValue(5)"), "{out}");
        assert!(out.contains("name = __assign_1[\"name\"];"), "{out}");
        assert!(out.contains("morph::js_object_rest(__assign_1, {\"name\"})"), "{out}");
    }

    #[test]
    fn destructuring_assignment_supports_computed_and_nested_keys() {
        let out = translate_stmts(
            "const key = \"email\";\n\
             let value = \"\";\n\
             ({ [key]: value } = user);\n\
             let city = \"\";\n\
             ({ extra: { city } } = user);",
        );
        assert!(out.contains("auto __key_"), "{out}");
        assert!(out.contains("morph::js_to_string(key).to_std_string()"), "{out}");
        assert!(out.contains("[__key_"), "{out}");
        assert!(out.contains("[\"extra\"][\"city\"]"), "{out}");
    }

    #[test]
    fn destructuring_assignment_writes_member_targets() {
        let out = translate_stmts(
            "class Box { label = \"\" }\n\
             const box = new Box();\n\
             const name = \"Ada\";\n\
             [box.label] = [name];",
        );
        assert!(out.contains("box->label = __assign_0[0];"), "{out}");
    }

    #[test]
    fn for_of_patterns_lower_through_a_loop_temp() {
        let out = translate_stmts(
            "for (const { value } of items) { console.log(value); }\n\
             for ([left, right] of pairs) { console.log(left, right); }",
        );
        assert!(out.contains("for (auto __loop_item_0 : items) {"), "{out}");
        assert!(out.contains("auto value = __loop_item_0[\"value\"];"), "{out}");
        assert!(out.contains("for (auto __loop_item_1 : pairs) {"), "{out}");
        assert!(out.contains("left = __loop_item_1[0];"), "{out}");
        assert!(out.contains("right = __loop_item_1[1];"), "{out}");
    }

    #[test]
    fn string_switch_dispatches_with_strict_equality_and_falls_through() {
        let out = translate_stmts(
            "let status = \"ready\";\n\
             let label = \"\";\n\
             switch (status) {\n\
                 case \"ready\":\n\
                     label = \"r\";\n\
                 case \"done\":\n\
                     label = label + \"d\";\n\
                     break;\n\
                 default:\n\
                     label = \"?\";\n\
             }",
        );
        assert!(out.contains("JsValue __morph_switch_disc = status;"), "{out}");
        assert!(out.contains("int __morph_switch_tag = -1;"), "{out}");
        assert!(
            out.contains(
                "if (morph::js_cmp::strict_eq(__morph_switch_disc, \"ready\")) { __morph_switch_tag = 0; }"
            ),
            "{out}"
        );
        assert!(
            out.contains(
                "else if (morph::js_cmp::strict_eq(__morph_switch_disc, \"done\")) { __morph_switch_tag = 1; }"
            ),
            "{out}"
        );
        assert!(out.contains("else { __morph_switch_tag = 2; }"), "{out}");
        assert!(out.contains("switch (__morph_switch_tag) {"), "{out}");
        assert!(out.contains("case 0:"), "{out}");
        assert!(out.contains("case 1:"), "{out}");
        assert!(out.contains("default:"), "{out}");
    }

    #[test]
    fn switch_with_only_default_does_not_emit_a_dangling_else() {
        let out = translate_stmts("let x = 1;\nswitch (x) { default: x = 2; }");
        assert!(out.contains("__morph_switch_tag = 0;"), "{out}");
        assert!(!out.contains("else { __morph_switch_tag"), "{out}");
        assert!(out.contains("default:"), "{out}");
    }

    #[test]
    fn captured_classic_for_binding_reboxes_each_iteration() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 for (let i = 0; i < 3; i++) {\n\
                     const read = () => i;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        assert!(
            out.contains("std::shared_ptr<int64_t> i = std::make_shared<int64_t>(0);"),
            "{out}"
        );
        assert!(out.contains("for (; (*i) < 3; ) {"), "{out}");
        assert!(out.contains("auto read = [&, i]() -> auto { return (*i); };"), "{out}");
        let rebind =
            out.find("i = std::make_shared<int64_t>(*i);").expect("per-iteration box copy");
        let update = out.find("(*i)++;").expect("loop update");
        assert!(rebind < update, "box copy must run before the update:\n{out}");
    }

    #[test]
    fn captured_pointer_loop_binding_keeps_its_carrier() {
        let out = translate_analyzed(
            "class Box {\n\
                 value = 0\n\
             }\n\
             function main(): void {\n\
                 for (let box = new Box(); box.value < 3; box.value++) {\n\
                     const read = () => box.value;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        assert!(out.contains("for (std::shared_ptr<Box> box ="), "{out}");
        assert!(!out.contains("std::shared_ptr<std::shared_ptr<Box>>"), "{out}");
        assert!(out.contains("auto read = [&, box]() -> auto { return box->value; };"), "{out}");
    }

    #[test]
    fn captured_for_of_binding_boxes_the_loop_item() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 const values: Array<int> = [10, 20];\n\
                 for (const value of values) {\n\
                     const read = () => value;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        assert!(out.contains("for (auto __loop_item_0 : values) {"), "{out}");
        assert!(
            out.contains(
                "std::shared_ptr<int64_t> value = std::make_shared<int64_t>(__loop_item_0);"
            ),
            "{out}"
        );
        assert!(out.contains("auto read = [&, value]() -> auto { return (*value); };"), "{out}");
    }

    #[test]
    fn captured_for_in_binding_boxes_the_key() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 const obj = { a: 1 };\n\
                 for (const key in obj) {\n\
                     const read = () => key;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        assert!(out.contains("for (auto __loop_item_0 : obj.sorted_keys()) {"), "{out}");
        assert!(
            out.contains(
                "std::shared_ptr<std::string> key = std::make_shared<std::string>(__loop_item_0);"
            ),
            "{out}"
        );
        assert!(out.contains("auto read = [&, key]() -> auto { return (*key); };"), "{out}");
    }

    #[test]
    fn continue_in_captured_for_copies_binding_before_update() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 for (let i = 0; i < 3; i++) {\n\
                     if (i === 1) { continue; }\n\
                     const read = () => i;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        let continue_pos = out.find("continue;").expect("continue statement");
        let before = &out[..continue_pos];
        let rebind =
            before.rfind("i = std::make_shared<int64_t>(*i);").expect("box copy before continue");
        let update = before.rfind("(*i)++;").expect("update before continue");
        assert!(rebind < update, "continue must copy the binding then update:\n{out}");
    }

    #[test]
    fn captured_for_of_pattern_bindings_are_boxed() {
        let out = translate_analyzed(
            "function main(): void {\n\
                 for (const { value } of items) {\n\
                     const read = () => value;\n\
                     console.log(read());\n\
                 }\n\
             }",
        );
        assert!(
            out.contains(
                "std::shared_ptr<JsValue> value = std::make_shared<JsValue>(__loop_item_0[\"value\"]);"
            ),
            "{out}"
        );
        assert!(out.contains("auto read = [&, value]() -> auto { return (*value); };"), "{out}");
    }
}
