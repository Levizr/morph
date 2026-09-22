use oxc_ast::ast::*;
use oxc_span::GetSpan;
use std::collections::{HashMap, HashSet};

use super::analyzer::{AnalysisResult, EscapeAnalyzer, EscapeKind, WidenedType};
use super::context::{Ctx, INDENT, TypeMode};
use super::js_comparison::{
    ComparisonKind, ComparisonSections, ComparisonSignature, OperandClass, build_header,
    cpp_type_to_class, required_includes,
};
use super::string_methods::StringMethodHandler;
use super::type_resolver::{
    native_number_annotation_type, param_type, resolve_type, resolve_type_annotation,
    ts_type_name_to_string,
};

pub struct CppTranslator<'a> {
    source: &'a str,
    pub ctx: Ctx,
    type_mode: TypeMode,
    analysis: Option<crate::codegen::analyzer::AnalysisResult>,
    comparison_uses: HashSet<ComparisonSignature>,
    narrow_index: HashMap<(String, u32), (String, String)>,
    active_narrows: HashMap<String, String>,
    destructure_count: usize,
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
fn cpp_string_literal(value: &str) -> String {
    format!("\"{}\"", escape_cpp_text(value))
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

impl<'a> CppTranslator<'a> {
    pub fn new(
        source: &'a str,
        indent_level: usize,
        type_mode: TypeMode,
        runtime_path: Option<String>,
    ) -> Self {
        let mut ctx = Ctx::default();
        ctx.indent_level = indent_level;
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
        };
        this
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

    pub fn translate_program(&mut self, program: &Program<'a>) -> String {
        // Analysis always runs: escape analysis, type widening, comparison
        // tracking, and integer-range proofs feed the single emitter path.
        if self.analysis.is_none() {
            self.run_analysis(program);
        }
        if let Some(analysis) = self.analysis.as_ref() {
            for split in &analysis.narrow_splits {
                self.narrow_index.insert(
                    (split.var_name.clone(), split.def_span_start),
                    (split.narrowed_name.clone(), split.narrowed_type.clone()),
                );
            }
        }
        let mut lines = Vec::new();
        for stmt in &program.body {
            if self.is_main_call(stmt) {
                continue;
            }
            if let Some(code) = self.emit_statement(stmt) {
                if !code.trim().is_empty() {
                    lines.push(code);
                }
            }
        }
        let body = lines.join("\n");
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
        if js_comparison_block.is_empty() {
            if includes.is_empty() { body } else { format!("{}\n\n{}", includes, body) }
        } else if includes.is_empty() {
            format!("{}\n\n{}", js_comparison_block, body)
        } else {
            format!("{}\n\n{}\n\n{}", includes, js_comparison_block, body)
        }
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
                Some(format!("{}{};", self.indent(), self.emit_expression(&e.expression)))
            }
            Statement::ReturnStatement(r) => Some(self.emit_return(r)),
            Statement::IfStatement(i) => Some(self.emit_if(i)),
            Statement::WhileStatement(w) => Some(self.emit_while(w)),
            Statement::DoWhileStatement(d) => Some(self.emit_do_while(d)),
            Statement::ForStatement(f) => Some(self.emit_for(f)),
            Statement::ForInStatement(f) => Some(self.emit_for_in(f)),
            Statement::ForOfStatement(f) => Some(self.emit_for_of(f)),
            Statement::BreakStatement(_) => Some(format!("{}break;", self.indent())),
            Statement::ContinueStatement(_) => Some(format!("{}continue;", self.indent())),
            Statement::SwitchStatement(sw) => Some(self.emit_switch(sw)),
            Statement::TryStatement(t) => Some(self.emit_try(t)),
            Statement::ThrowStatement(t) => Some(self.emit_throw(t)),
            Statement::EmptyStatement(_) => None,
            Statement::LabeledStatement(l) => {
                let inner = self.emit_statement(&l.body).unwrap_or_default();
                Some(format!("{}// label {}:\n{}", self.indent(), l.label.name, inner))
            }
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
            return self.emit_destructured(d, kind);
        }
        let (name, _) = self.binding_to_identifier(&d.id);
        if name.starts_with("/*") {
            return Some(format!("{}/* destructuring not supported */", self.indent()));
        }
        // Track Window handles for method/property lowering: direct
        // `useWindow(...)` / `new Window(...)` initializers only.
        if let Some(init) = d.init.as_ref() {
            let is_window = match init {
                Expression::CallExpression(call) => {
                    matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "useWindow")
                }
                Expression::NewExpression(n) => {
                    matches!(&n.callee, Expression::Identifier(id) if id.name.as_str() == "Window")
                }
                _ => false,
            };
            if is_window {
                self.ctx.window_vars.insert(name.clone());
            }
        }

        self.emit_typed_variable_declarator(d, &name, kind)
    }

    /// Destructuring declarations (`const {x} = obj`, `const [a] = arr`).
    ///
    /// One `auto` per binding. Literal containers inline each element
    /// value (single evaluation, full precision); identifier sources
    /// repeat a side-effect-free root; anything else stages through one
    /// `__destructure_N` temporary. Defaults, rest elements, computed
    /// keys, and string iteration keep the comment fallback below.
    /// Bindings emit plain `auto` regardless of `kind`, matching how the
    /// typed declarator treats deduced locals, so later moves keep working.
    fn emit_destructured(&mut self, d: &VariableDeclarator<'a>, _kind: &str) -> Option<String> {
        let fallback = || Some(format!("{}/* destructuring not supported */", self.indent()));
        let Some(init) = d.init.as_ref() else {
            return fallback();
        };
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

    /// Absolute access path per bound name. `false` on anything the
    /// renderer cannot express (defaults, rest, computed keys).
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

    /// Defaults keep the comment fallback: they prove nothing statically
    /// and need runtime presence checks the renderer does not emit.
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
        const MEMBERS: &[&str] = &["ok", "status", "text", "headers", "statusText"];
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

    fn emit_typed_variable_declarator(
        &mut self,
        d: &VariableDeclarator<'a>,
        name: &str,
        kind: &str,
    ) -> Option<String> {
        // `await fetch()` whose result uses the Response API (r.ok(),
        // r.status, r.text(), ...) declares the full Response and awaits
        // fetch_response. Body-text-only awaits keep the JsString path
        // below, so existing fixtures are unaffected.
        if let Some(init) = &d.init {
            if let Expression::AwaitExpression(awaited) = init {
                if let Expression::CallExpression(call) = &awaited.argument {
                    if let Expression::Identifier(callee) = &call.callee {
                        if callee.name.as_str() == "fetch" && self.uses_response_api(name) {
                            self.ctx.needed.insert("\"../../runtime/cpp/net/net.h\"".to_string());
                            let args: Vec<String> =
                                call.arguments.iter().map(|a| self.emit_argument(a)).collect();
                            self.ctx
                                .var_types
                                .insert(name.to_string(), "morph::net::Response".to_string());
                            let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
                            return Some(format!(
                                "{}{}morph::net::Response {} = co_await morph::net::fetch_response({});",
                                self.indent(),
                                prefix,
                                name,
                                args.join(", ")
                            ));
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
        let mut cpp_type = self.apply_declaration_widening(d, name, base_type, widened);
        // Intent-based: Promise<T> annotated but init is sync plain call -> use T (match JS runtime)
        if let Some(init) = &d.init {
            if let Some(stripped) = self.strip_result_for_sync_call(&cpp_type, init) {
                cpp_type = stripped;
            }
        }
        let new_class_name = self.new_class_name_of(&cpp_type, &d.init);
        if let Some(class_name) = &new_class_name {
            cpp_type = class_name.clone();
        }
        let new_class_args =
            if new_class_name.is_some() { self.new_class_args(&d.init) } else { None };
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
        let new_class_alloc: Option<(String, String)> =
            match (&new_class_name, &new_class_args, &escape_kind) {
                (Some(class_name), Some(args), EscapeKind::Return | EscapeKind::Global) => Some((
                    format!("std::unique_ptr<{}>", class_name),
                    format!("std::make_unique<{}>({})", class_name, args),
                )),
                (
                    Some(class_name),
                    Some(args),
                    EscapeKind::ClosureCapture
                    | EscapeKind::MultipleRefs
                    | EscapeKind::AsyncBoundary
                    | EscapeKind::None,
                ) => Some((
                    // Non-escaping `new` keeps shared ownership: callers may
                    // hold the value in `shared_ptr` parameters a stack value
                    // could not satisfy.
                    format!("std::shared_ptr<{}>", class_name),
                    format!("std::make_shared<{}>({})", class_name, args),
                )),
                _ => None,
            };
        let (alloc_type, init_code) = match new_class_alloc {
            Some(pair) => pair,
            None => {
                if let Some(init) = &d.init {
                    match escape_kind {
                        EscapeKind::None => {
                            if shared_wrap_call {
                                (
                                    cpp_type.clone(),
                                    format!("{}({})", cpp_type, self.emit_expression(init)),
                                )
                            } else {
                                let init_code = if cpp_type.starts_with("std::vector<")
                                    && matches!(init, Expression::ArrayExpression(_))
                                {
                                    self.emit_vector_literal_typed(init, Some(&cpp_type))
                                } else if cpp_type == "char"
                                    && matches!(init, Expression::StringLiteral(_))
                                {
                                    self.emit_char_literal(init)
                                } else {
                                    self.emit_expression(init)
                                };
                                (cpp_type.clone(), init_code)
                            }
                        }
                        EscapeKind::Return | EscapeKind::Global => {
                            let alloc = format!("std::unique_ptr<{}>", cpp_type);
                            let init_code = if matches!(init, Expression::ArrayExpression(_)) {
                                format!(
                                    "std::make_unique<{}>(std::vector{{{}}})",
                                    cpp_type,
                                    self.emit_vector_literal_typed(init, Some(&cpp_type))
                                        .trim_start_matches('{')
                                        .trim_end_matches('}')
                                )
                            } else {
                                format!(
                                    "std::make_unique<{}>({})",
                                    cpp_type,
                                    self.emit_expression(init)
                                )
                            };
                            (alloc, init_code)
                        }
                        EscapeKind::ClosureCapture
                        | EscapeKind::MultipleRefs
                        | EscapeKind::AsyncBoundary => {
                            let alloc = format!("std::shared_ptr<{}>", cpp_type);
                            let init_code = format!(
                                "std::make_shared<{}>({})",
                                cpp_type,
                                self.emit_expression(init)
                            );
                            (alloc, init_code)
                        }
                    }
                } else if matches!(
                    escape_kind,
                    EscapeKind::ClosureCapture
                        | EscapeKind::MultipleRefs
                        | EscapeKind::AsyncBoundary
                ) {
                    (format!("std::shared_ptr<{}>", cpp_type), String::new())
                } else if matches!(escape_kind, EscapeKind::Return | EscapeKind::Global) {
                    (format!("std::unique_ptr<{}>", cpp_type), String::new())
                } else {
                    (cpp_type.clone(), String::new())
                }
            }
        };

        if alloc_type.starts_with("std::shared_ptr<") {
            self.ctx.shared_ptr_vars.insert(name.to_string());
        }
        if alloc_type.starts_with("std::unique_ptr<") {
            self.ctx.unique_ptr_vars.insert(name.to_string());
        }
        let init_code = match &d.init {
            Some(init) if Self::is_trusted_native_number(d, &widened).is_some() => {
                self.convert_number_value(&cpp_type, init, init_code)
            }
            Some(init)
                if cpp_type == "JsString"
                    && matches!(
                        self.operand_class_of(init),
                        OperandClass::Integer | OperandClass::Float
                    ) =>
            {
                format!("JsString(JsNumber({}))", init_code)
            }
            _ => init_code,
        };
        self.ctx.need(&alloc_type);
        self.ctx.var_types.insert(name.to_string(), alloc_type.clone());

        let prefix = if self.ctx.indent_level == 0 { "static " } else { "" };
        let mut final_type = alloc_type;
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

    fn infer_type_from_init(&self, node: &Expression<'a>) -> Option<String> {
        match node {
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
            Expression::Identifier(id) if id.name.as_str() == "undefined" => {
                Some("JsUndefined".to_string())
            }
            Expression::ArrayExpression(arr) => {
                if arr.elements.is_empty() {
                    Some("std::vector<JsValue>".to_string())
                } else {
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
                // Check if we're indexing into a JsArray
                if let Expression::Identifier(id) = &m.object {
                    if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                        if var_type == "JsArray" {
                            return Some("JsValue".to_string());
                        }
                    }
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
                        return Some(format!("std::shared_ptr<{}>", id.name));
                    }
                }
                Some("JsValue".to_string())
            }
            Expression::Identifier(id) => self.ctx.var_types.get(id.name.as_str()).cloned(),
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

    fn emit_while(&mut self, w: &WhileStatement<'a>) -> String {
        let cond = self.emit_truthy_test(&w.test);
        let body_code = self.emit_statement(&w.body).unwrap_or_else(|| "{}".to_string());
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
        let body = self.emit_statement(&d.body).unwrap_or_else(|| "{}".to_string());
        let cond = self.emit_truthy_test(&d.test);
        format!("{}do {} while ({});", self.indent(), body, cond)
    }

    fn emit_for(&mut self, f: &ForStatement<'a>) -> String {
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

    fn emit_for_init(&mut self, init: &ForStatementInit<'a>) -> Option<String> {
        match init {
            ForStatementInit::VariableDeclaration(d) => {
                // For init, don't add `static` and emit as `type name = init` without trailing `;` and without indent
                let kind = d.kind.as_str().to_string();
                let mut parts = Vec::new();
                for decl in &d.declarations {
                    let (name, _) = self.binding_to_identifier(&decl.id);
                    if name.starts_with("/*") {
                        continue;
                    }
                    let cpp_type = match self.type_mode {
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
                                self.infer_type_from_init(init)
                                    .unwrap_or_else(|| "auto".to_string())
                            } else {
                                "auto".to_string()
                            }
                        }
                        TypeMode::Infer => {
                            if let Some(init) = &decl.init {
                                self.infer_type_from_init(init)
                                    .unwrap_or_else(|| "auto".to_string())
                            } else {
                                "auto".to_string()
                            }
                        }
                    };
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

    fn emit_for_in(&mut self, f: &ForInStatement<'a>) -> String {
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
        let body = self.emit_statement(&f.body).unwrap_or_else(|| "{}".to_string());
        format!("{}for (auto {} : {}) {}", self.indent(), left, iter, body)
    }

    fn emit_for_of(&mut self, f: &ForOfStatement<'a>) -> String {
        let left = self.emit_for_left(&f.left);
        let right = self.emit_expression(&f.right);
        let body = self.emit_statement(&f.body).unwrap_or_else(|| "{}".to_string());
        if f.r#await {
            format!("{}for (auto {} : co_await {}) {}", self.indent(), left, right, body)
        } else {
            format!("{}for (auto {} : {}) {}", self.indent(), left, right, body)
        }
    }

    fn emit_for_left(&mut self, left: &ForStatementLeft<'a>) -> String {
        match left {
            ForStatementLeft::VariableDeclaration(d) => {
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
        let disc = self.emit_expression(&sw.discriminant);
        // Only apply .as_int() if discriminant is JsValue type
        let disc_code = if self.is_jsvalue_type(&sw.discriminant) {
            format!("({}).as_int()", disc)
        } else {
            disc
        };
        let mut lines = vec![format!("{}switch ({}) {{", self.indent(), disc_code)];
        let bi = INDENT.repeat(self.ctx.indent_level + 1);
        for case in &sw.cases {
            if let Some(test) = &case.test {
                lines.push(format!("{}case {}:", bi, self.emit_expression(test)));
            } else {
                lines.push(format!("{}default:", bi));
            }
            for stmt in &case.consequent {
                if let Some(code) = self.emit_statement(stmt) {
                    lines.push(format!("{}    {}", bi, code.trim_start()));
                }
            }
        }
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
        let params = self.format_params(&f.params);
        if name == "main" && is_async {
            let code = self.emit_async_main(f, &ret, &params);
            self.ctx.has_infinite_loop = old_has_loop || self.ctx.has_infinite_loop;
            return Some(code);
        }
        let mut final_ret = ret.clone();
        if is_async {
            final_ret = self.ctx.async_result_type(&ret);
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
        self.ctx.fn_return_types.insert(name.clone(), final_ret.clone());
        self.ctx.need(&final_ret);
        let outer_fn = self.ctx.current_fn.clone();
        self.ctx.current_fn = Some(name.clone());
        self.ctx.fn_body_depth += 1;
        if is_async {
            self.ctx.is_async_fn += 1;
        }
        // Nested functions own their returns; only the async-main wrapper
        // discards them (the old Python transpiler dropped nested ones too).
        let old_drop = self.ctx.drop_return_value;
        self.ctx.drop_return_value = false;
        let mut body = self.emit_function_body(f.body.as_ref().unwrap());
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
        let result = format!("{}{}\n{}", tp, header, body);
        self.ctx.has_infinite_loop = old_has_loop || self.ctx.has_infinite_loop;
        self.ctx.current_fn = outer_fn;
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

    fn all_returns_plain(body: &FunctionBody<'a>) -> bool {
        let mut args = Vec::new();
        Self::collect_returns(&body.statements, &mut args);
        if args.is_empty() {
            return false;
        }
        args.iter().all(|a| Self::return_arg_is_plain(a))
    }

    fn emit_async_main(&mut self, f: &Function<'a>, _ret: &str, _params: &str) -> String {
        self.ctx.needed.insert("\"../../runtime/cpp/reactivity/task.h\"".to_string());
        self.ctx.is_async_fn += 1;
        let old_drop = self.ctx.drop_return_value;
        self.ctx.drop_return_value = true;
        let outer_fn = self.ctx.current_fn.clone();
        let main_name = f.id.as_ref().map(|id| id.name.to_string()).unwrap_or_default();
        self.ctx.current_fn = Some(main_name);
        let body = self.emit_function_body(f.body.as_ref().unwrap());
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

    fn emit_function_body(&mut self, body: &FunctionBody<'a>) -> String {
        if body.statements.is_empty() {
            return "{}".to_string();
        }
        let mut lines = vec!["{".to_string()];
        let old = self.ctx.indent_level;
        self.ctx.indent_level = old + 1;
        for stmt in &body.statements {
            if let Some(code) = self.emit_statement(stmt) {
                lines.push(code);
            }
        }
        self.ctx.indent_level = old;
        lines.push(format!("{}}}", self.indent()));
        lines.join("\n")
    }

    fn format_params(&mut self, params: &FormalParameters<'a>) -> String {
        let mut parts = Vec::new();
        for p in &params.items {
            let (name, _) = self.binding_to_identifier(&p.pattern);
            if name.starts_with("/*") {
                continue;
            }
            let cpp_type = match self.type_mode {
                TypeMode::Strict => {
                    if let Some(ta) = &p.type_annotation {
                        resolve_type_annotation(
                            Some(ta),
                            "auto",
                            &self.ctx.template_params,
                            false,
                            &self.ctx.class_names,
                        )
                    } else {
                        "auto".to_string()
                    }
                }
                TypeMode::Infer => "auto".to_string(),
            };
            // initializer via p.initializer? FormalParameter has no initializer, but pattern AssignmentPattern handles default?
            // Actually FormalParameter has no initializer field in oxc? Check earlier: FormalParameter has no initializer, but we can handle AssignmentPattern in pattern
            let cpp_type = self.wrap_type(&cpp_type);
            self.ctx.need(&cpp_type);
            self.ctx.var_types.insert(name.clone(), cpp_type.clone());
            let param_cpp = param_type(&cpp_type);
            if param_cpp == "std::string_view" {
                self.ctx.need("std::string_view");
            }
            let mut decl = format!("{} {}", param_cpp, name);
            if let BindingPattern::AssignmentPattern(a) = &p.pattern {
                decl.push_str(&format!(" = {}", self.emit_expression(&a.right)));
            }
            parts.push(decl);
        }
        if let Some(rest) = &params.rest {
            let (name, _) = self.binding_to_identifier(&rest.rest.argument);
            let cpp_type = "auto".to_string();
            let cpp_type = self.wrap_type(&cpp_type);
            self.ctx.need(&cpp_type);
            self.ctx.var_types.insert(name.clone(), cpp_type.clone());
            let param_cpp = param_type(&cpp_type);
            parts.push(format!("{} {}", param_cpp, name));
        }
        parts.join(", ")
    }

    fn wrap_type(&mut self, cpp_type: &str) -> String {
        if self.ctx.template_params.contains(cpp_type) {
            self.ctx.need("std::shared_ptr");
            return format!("std::shared_ptr<{}>", cpp_type);
        }
        cpp_type.to_string()
    }

    fn emit_class(&mut self, class: &Class<'a>) -> String {
        let name = class.id.as_ref().map(|id| id.name.to_string()).unwrap_or_default();
        if !name.is_empty() {
            self.ctx.class_names.insert(name.clone());
        }
        let old_tp = self.ctx.template_params.clone();
        let old_class = self.ctx.class_name.clone();
        let old_super = self.ctx.super_class_name.clone();
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
        if let Some(heritage) = &class.heritage {
            let expr = &heritage.expression;
            if let Expression::Identifier(id) = expr {
                bases.push(format!("public {}", id.name));
            } else {
                bases.push(format!("public {}", self.emit_expression(expr)));
            }
        }
        for imp in &class.implements {
            bases.push(format!("public {}", ts_type_name_to_string(&imp.expression)));
        }
        let heritage_str =
            if bases.is_empty() { String::new() } else { format!(" : {}", bases.join(", ")) };
        lines.push(format!("class {}{} {{", name, heritage_str));
        // Increase indent for class body (like Python's sub)
        let old_indent = self.ctx.indent_level;
        self.ctx.indent_level = 1;
        let mut access_map: HashMap<String, Vec<String>> = HashMap::new();
        for el in &class.body.body {
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
        lines.join("\n")
    }

    fn format_constructor_params(&mut self, params: &FormalParameters<'a>) -> String {
        let mut parts = Vec::new();
        for p in &params.items {
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
            parts.push(format!("{} p_{}", param_type_str, name));
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
                let name = self.property_key_to_string(&m.key).unwrap_or_default();
                let access = m
                    .accessibility
                    .map(|a| format!("{:?}", a).to_lowercase())
                    .unwrap_or_else(|| "public".to_string());
                let access = match access.as_str() {
                    "private" => "private".to_string(),
                    "protected" => "protected".to_string(),
                    _ => "public".to_string(),
                };
                if m.kind == MethodDefinitionKind::Constructor {
                    // Use constructor-specific formatting with p_ prefix and initializer list (like Python)
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
                    let mut init_entries: Vec<String> = Vec::new();
                    let mut remaining: Vec<&Statement<'a>> = Vec::new();
                    let mut super_seen = false;
                    if let Some(body) = &m.value.body {
                        for stmt in &body.statements {
                            if !super_seen {
                                if let Statement::ExpressionStatement(es) = stmt {
                                    if let Expression::CallExpression(call) = &es.expression {
                                        if let Expression::Super(_) = &call.callee {
                                            super_seen = true;
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
                                            init_entries.push(format!(
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
                                                init_entries.push(format!(
                                                    "{}(std::move({}))",
                                                    prop, rhs_str
                                                ));
                                                is_this_assign = true;
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
                    let init_str = if init_entries.is_empty() {
                        String::new()
                    } else {
                        format!("\n{}{}: {}", self.indent(), INDENT, init_entries.join(", "))
                    };
                    // Build body with remaining statements
                    let body_str = if remaining.is_empty() {
                        "{}".to_string()
                    } else {
                        let mut lines = vec!["{".to_string()];
                        let old_indent = self.ctx.indent_level;
                        self.ctx.indent_level = old_indent + 1;
                        for stmt in remaining {
                            if let Some(code) = self.emit_statement(stmt) {
                                lines.push(code);
                            }
                        }
                        self.ctx.indent_level = old_indent;
                        lines.push(format!("{}}}", self.indent()));
                        lines.join("\n")
                    };
                    return (
                        access,
                        format!("{}({}){} {}", class_name, params_str, init_str, body_str),
                    );
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
                            let body = self.emit_function_body(b);
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
                    "auto".to_string()
                };
                let cpp_type = self.wrap_type(&cpp_type);
                self.ctx.need(&cpp_type);
                let prefix = if p.r#static { "static inline " } else { "" };
                if let Some(val) = &p.value {
                    (
                        access,
                        format!("{}{} {} = {};", prefix, cpp_type, name, self.emit_expression(val)),
                    )
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
                format!("co_await {}", self.emit_expression(&a.argument))
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
                format!("{}{}", u.operator.as_str(), self.emit_expression(&u.argument))
            }
            Expression::UpdateExpression(u) => {
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
                format!("{}#{}", self.emit_expression(&p.object), p.field.name)
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
            Expression::RegExpLiteral(r) => self.span_text(r.span).to_string(),
            Expression::BigIntLiteral(b) => {
                if let Some(raw) = &b.raw {
                    format!("{}n", raw.as_str())
                } else {
                    self.span_text(b.span).to_string()
                }
            }
            _ => format!("/* unhandled expr {:?} */", expr.span()),
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
            return format!("JsArray{{{}}}", elems.join(", "));
        }
        let mut lines = vec!["[&]() {".to_string(), "    JsArray __a{};".to_string()];
        for (idx, el) in arr.elements.iter().enumerate() {
            match el {
                ArrayExpressionElement::SpreadElement(s) => {
                    let arg = self.emit_expression(&s.argument);
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
            return "JsObject{}".to_string();
        }
        let mut pairs = Vec::new();
        for prop in &obj.properties {
            match prop {
                ObjectPropertyKind::ObjectProperty(p) => {
                    if let Some(key) = self.property_key_to_string(&p.key) {
                        let val = self.emit_expression(&p.value);
                        pairs.push(format!("{{\"{}\", {}}}", key, val));
                    }
                }
                ObjectPropertyKind::SpreadProperty(s) => {
                    pairs.push(format!("/* spread {} */", self.emit_expression(&s.argument)));
                }
            }
        }
        format!("JsObject{{{}}}", pairs.join(", "))
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
            other => format!("JsValue({}).as_int()", self.emit_expression(other)),
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
                | "JsNull"
                | "JsUndefined"
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

    /// Classify a call result: known string/number/boolean methods keep their
    /// native class, everything else is treated as dynamic.
    fn call_result_class(&self, call: &CallExpression<'a>) -> OperandClass {
        if let Expression::StaticMemberExpression(member) = &call.callee {
            let method_name = member.property.name.as_str();
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

        if let Some(comparison_kind) = ComparisonKind::from_binary_operator(op) {
            let left_class = self.operand_class_of(&b.left);
            let right_class = self.operand_class_of(&b.right);
            if Self::should_use_js_comparison(
                comparison_kind,
                &b.left,
                &b.right,
                left_class,
                right_class,
            ) {
                let left = self.emit_expression(&b.left);
                let right = self.emit_expression(&b.right);
                self.record_comparison_use(comparison_kind, left_class, right_class);
                return format!(
                    "morph::js_cmp::{}({}, {})",
                    comparison_kind.dispatcher_name(),
                    left,
                    right
                );
            }
        }

        // Check if we need to convert JsValue to number for arithmetic
        let needs_left_convert = self.is_jsarray_index(&b.left) || self.is_jsvalue_var(&b.left);
        let needs_right_convert = self.is_jsarray_index(&b.right) || self.is_jsvalue_var(&b.right);

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

        // For division, use double to match JS semantics (float division)
        if op == "/" {
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
            "**" => "/* pow */",
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

    fn is_jsvalue_type(&self, expr: &Expression<'a>) -> bool {
        // Check if expression is a JsValue type
        if let Expression::Identifier(id) = expr {
            if let Some(var_type) = self.ctx.var_types.get(id.name.as_str()) {
                return var_type == "JsValue" || var_type == "JsArray" || var_type == "JsObject";
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
                if var_type == "JsString" || var_type == "JsValue" {
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
                        if bt == "JsArray" || bt == "JsObject" || bt == "JsValue" {
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

    fn emit_assignment(&mut self, a: &AssignmentExpression<'a>) -> String {
        // `win.title = x` on a tracked Window handle lowers to a registry
        // write; every other member assignment keeps default emission.
        if a.operator.as_str() == "=" {
            if let AssignmentTarget::StaticMemberExpression(mem) = &a.left {
                if let Expression::Identifier(obj) = &mem.object {
                    if mem.property.name.as_str() == "title"
                        && self.ctx.window_vars.contains(obj.name.as_str())
                    {
                        let rhs = self.emit_expression(&a.right);
                        return format!("__morph_win_set_title({}, {})", obj.name, rhs);
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
        let left = self.emit_assignment_target(&a.left);
        let right = self.emit_expression(&a.right);
        // Check if we need to convert JsValue from array element to int
        let right = if self.is_jsvalue_var(&a.right) {
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
                format!("{}#{}", self.emit_expression(&p.object), p.field.name)
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
                format!("{}#{}", self.emit_expression(&p.object), p.field.name)
            }
            _ => self.span_text(target.span()).to_string(),
        }
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
        // only for locals tracked as Window handles (from `useWindow()`
        // or `new Window()`), so user objects with same-named methods
        // keep default emission.
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if let Expression::Identifier(obj) = &m.object {
                if self.ctx.window_vars.contains(obj.name.as_str()) {
                    if let Some(lowered) =
                        self.emit_window_method(obj.name.as_str(), m.property.name.as_str(), call)
                    {
                        return lowered;
                    }
                }
            }
        }
        if let Expression::StaticMemberExpression(m) = &call.callee {
            if m.property.name.as_str() == "push" && call.arguments.len() == 1 {
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

    fn emit_static_member(&mut self, m: &StaticMemberExpression<'a>) -> String {
        let obj = self.emit_expression(&m.object);
        let prop = m.property.name.to_string();
        if let Expression::ThisExpression(_) = &m.object {
            if self.ctx.fn_expr_depth > 0 {
                return format!("_jsThis[\"{}\"]", prop);
            }
            return format!("this->{}", prop);
        }
        if let Expression::Super(_) = &m.object {
            if let Some(cls) = &self.ctx.class_name {
                return format!("{}::{}", cls, prop);
            }
            return format!("/* super */{}", prop);
        }
        // Window-handle properties (tracked locals only): `closed` and
        // `title` lower to registry reads; anything else keeps default
        // emission (fails C++ compilation loudly on the int handle).
        if let Expression::Identifier(id) = &m.object {
            if self.ctx.window_vars.contains(id.name.as_str()) {
                let obj = id.name.as_str();
                match prop.as_str() {
                    "closed" => return format!("__morph_win_closed({obj})"),
                    "title" => return format!("__morph_win_title({obj})"),
                    _ => {}
                }
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
        format!("{}.{}", obj, prop)
    }

    fn should_use_bracket(&self, obj_node: &Expression<'a>, obj_str: &str) -> bool {
        if let Expression::Identifier(id) = obj_node {
            return matches!(
                self.ctx.var_types.get(id.name.as_str()).map(|s| s.as_str()),
                Some("JsObject") | Some("JsValue")
            );
        }
        // For chained access like `a.b.c` or `arr[i].prop`, be conservative:
        // Only use bracket for JsObject/JsValue property access, not for JsString/JsArray methods
        // `arr[i]` returns JsValue, but `arr[i].toUpperCase()` should be dot, not bracket
        // So don't automatically return true for Static/ComputedMemberExpression
        if let Some(t) = self.ctx.var_types.get(obj_str) {
            return t == "JsObject" || t == "JsValue";
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
                format!("{}#{}", self.emit_expression(&p.object), p.field.name)
            }
            _ => "/* chain */".to_string(),
        }
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
        self.ctx.need("std::make_shared");
        format!("std::make_shared<{}{}>({})", callee, type_args, args.join(", "))
    }

    fn emit_arrow(&mut self, f: &ArrowFunctionExpression<'a>) -> String {
        let capture = self.lambda_capture(&f.params);
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
                let body = self.emit_function_body(f.body.as_function_body().unwrap());
                self.ctx.drop_return_value = saved_drop;
                return format!("{}( {} ) -> void {}", capture, event_params, body);
            }
        }
        if is_expr {
            if let Some(expr) = f.body.as_expression() {
                let e = self.emit_expression(expr);
                let kw = if is_async { "co_return" } else { "return" };
                return format!("{}({}) -> {} {{ {} {}; }}", capture, params, ret, kw, e);
            }
            return format!("{}({}) -> {} {{}}", capture, params, ret);
        } else {
            let saved_drop = self.ctx.drop_return_value;
            self.ctx.drop_return_value = false;
            let body = if is_async {
                self.ctx.is_async_fn += 1;
                let b = self.emit_function_body(f.body.as_function_body().unwrap());
                self.ctx.is_async_fn -= 1;
                b
            } else {
                self.emit_function_body(f.body.as_function_body().unwrap())
            };
            self.ctx.drop_return_value = saved_drop;
            return format!("{}({}) -> {} {}", capture, params, ret, body);
        }
    }

    /// Capture list for a lambda. Shared locals the enclosing function
    /// captures are held by value (`[&, count]`) so an escaping closure
    /// keeps its state alive; everything else keeps today's default.
    /// Names are sorted so output is deterministic across runs. File-scope
    /// lambdas always stay `[]`: namespace scope forbids captures, and
    /// file statics are visible without them. Parameters shadowing an
    /// outer name are never captured: the body reads the parameter.
    fn lambda_capture(&self, params: &FormalParameters<'a>) -> String {
        if self.ctx.fn_body_depth == 0 {
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
            .filter(|var| {
                !params.items.iter().any(|param| {
                    let (param_name, _) = self.binding_to_identifier(&param.pattern);
                    param_name == *var
                })
            })
            .collect();
        held.sort();
        if held.is_empty() {
            return "[&]".to_string();
        }
        format!("[&, {}]", held.join(", "))
    }

    fn emit_function_expression(&mut self, f: &Function<'a>) -> String {
        self.ctx.fn_expr_depth += 1;
        let is_async = f.r#async;
        let params = self.format_params(&f.params);
        let _ = params;
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
        } else {
            "JsValue".to_string()
        };

        self.ctx.need(&ret);
        let saved_drop = self.ctx.drop_return_value;
        self.ctx.drop_return_value = false;
        let body = if let Some(b) = &f.body {
            if is_async {
                self.ctx.is_async_fn += 1;
                let code = self.emit_function_body(b);
                self.ctx.is_async_fn -= 1;
                code
            } else {
                self.emit_function_body(b)
            }
        } else {
            "{}".to_string()
        };
        self.ctx.drop_return_value = saved_drop;
        self.ctx.fn_expr_depth -= 1;
        self.ctx.need("JsValue");
        if is_async {
            format!("+[](JsValue _jsThis) -> {} {}", ret, body)
        } else {
            format!("+[](JsValue _jsThis) -> JsValue {}", body)
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
        assert!(out.contains("{\"parent\", JsValue(main).as_int()}"), "{out}");
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
}
