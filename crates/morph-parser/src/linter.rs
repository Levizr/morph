use std::collections::{HashMap, HashSet};

use crate::ast_types::{JsxNode, JsxPropValue, LintError, MxImportKind, MxSource};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_semantic::SemanticBuilder;
use oxc_span::{GetSpan, LabeledSpan};
use oxc_syntax::scope::ScopeFlags;

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
    (line, offset.saturating_sub(last) + 1)
}

/// Names usable without import or declaration (runtime-supported globals).
const NATIVE_GLOBALS: &[&str] = &[
    "undefined",
    "NaN",
    "Infinity",
    "console",
    "Promise",
    "Error",
    "fetch",
    "setTimeout",
    "setInterval",
    "clearTimeout",
    "clearInterval",
    "parseInt",
    "parseFloat",
];

/// Morph APIs that must be imported from 'morph' (bare use is an error).
const MORPH_BUILTINS: &[&str] =
    &["morphState", "morphEffect", "morphShared", "morphEvent", "useWindow", "Window", "CSS"];

/// AST positions that are never value references: JSX tag names
/// (lowercase intrinsics like `<div>`, and component tags like `<Card>` —
/// unknown components get the precise `mx-component-unknown` instead of
/// a duplicate `mx-undefined`) and type-name ranges (`x: int`, `as Foo`,
/// generics). A real variable sharing an intrinsic's name is still
/// checked — only its own tag-position span is skipped.
struct SkipSpans {
    intrinsics: HashSet<u32>,
    tags: HashSet<u32>,
    type_ranges: Vec<(u32, u32)>,
}

fn collect_skip_spans(program: &Program) -> SkipSpans {
    struct Collector {
        out: SkipSpans,
    }
    impl<'a> Visit<'a> for Collector {
        fn visit_jsx_element_name(&mut self, name: &JSXElementName<'a>) {
            // Lowercase tags (`<div>`) parse as `Identifier`, capitalized
            // ones (`<Frob>`) as `IdentifierReference` — every tag position
            // is skipped for `mx-undefined`: intrinsics never resolve, and
            // unknown components report the precise `mx-component-unknown`.
            match name {
                JSXElementName::Identifier(id) => {
                    self.out.tags.insert(id.span.start);
                    if id.name.as_str().starts_with(|c: char| c.is_ascii_lowercase()) {
                        self.out.intrinsics.insert(id.span.start);
                    }
                }
                JSXElementName::IdentifierReference(id) => {
                    self.out.tags.insert(id.span.start);
                }
                JSXElementName::NamespacedName(n) => {
                    self.out.tags.insert(n.name.span.start);
                }
                JSXElementName::MemberExpression(m) => {
                    let mut obj = &m.object;
                    loop {
                        match obj {
                            JSXMemberExpressionObject::IdentifierReference(id) => {
                                self.out.tags.insert(id.span.start);
                                break;
                            }
                            JSXMemberExpressionObject::MemberExpression(inner) => {
                                obj = &inner.object;
                            }
                            JSXMemberExpressionObject::ThisExpression(_) => break,
                        }
                    }
                }
                JSXElementName::ThisExpression(_) => {}
            }
            // Continue into member expressions (`<Foo.Bar>` resolves Foo).
            oxc_ast_visit::walk::walk_jsx_element_name(self, name);
        }
        fn visit_ts_type_reference(&mut self, ty: &TSTypeReference<'a>) {
            let span = ty.type_name.span();
            self.out.type_ranges.push((span.start, span.end));
            oxc_ast_visit::walk::walk_ts_type_reference(self, ty);
        }
    }
    let mut collector = Collector {
        out: SkipSpans {
            intrinsics: HashSet::new(),
            tags: HashSet::new(),
            type_ranges: Vec::new(),
        },
    };
    collector.visit_program(program);
    collector.out
}

/// Lowercase JSX tag positions (`<div>`, `<span>`): intrinsic elements,
/// never identifier references. Collected from the AST so a real
/// variable that merely shares a name is still checked at its own spans.
fn jsx_intrinsic_spans(program: &Program) -> HashSet<u32> {
    collect_skip_spans(program).intrinsics
}

/// `mx-undefined` / `mx-no-morph-import`: every referenced name must be
/// declared (locals, params, imports, components) or imported from
/// 'morph' (for Morph APIs) or a supported native global. Uses real
/// scope resolution — shadowing, params, and imports all count.
fn check_unresolved(program: &Program, file_path: &str, offsets: &[usize]) -> Vec<LintError> {
    let mut out = Vec::new();
    let semantic = SemanticBuilder::new().with_build_nodes(true).build(program).semantic;
    let scoping = semantic.scoping();
    let skip = collect_skip_spans(program);
    let declared: Vec<&str> = scoping.symbol_names().collect();
    let mut seen = HashSet::new();
    let mut names: Vec<(&str, Vec<Span>)> = Vec::new();
    for (atom, refs) in scoping.root_unresolved_references() {
        let mut spans = Vec::new();
        for ref_id in refs {
            let node_id = scoping.get_reference(*ref_id).node_id();
            spans.push(semantic.nodes().get_node(node_id).span());
        }
        names.push((atom, spans));
    }
    names.sort_by(|a, b| a.0.cmp(b.0));
    for (name, spans) in names {
        if name.contains('-') {
            continue;
        }
        for span in spans {
            if skip.intrinsics.contains(&span.start) {
                continue;
            }
            if skip.tags.contains(&span.start) {
                continue;
            }
            if skip.type_ranges.iter().any(|(s, e)| span.start >= *s && span.end <= *e) {
                continue;
            }
            if !seen.insert((name.to_string(), span.start)) {
                continue;
            }
            let (line, col) = offset_to_line_col_fast(offsets, span.start);
            if MORPH_BUILTINS.contains(&name) {
                out.push(LintError {
                    severity: "error".into(),
                    code: "mx-no-morph-import".into(),
                    message: format!("`{name}` is used but never imported"),
                    suggestion: Some(format!("Add `import {{ {name} }} from 'morph'`")),
                    file_path: file_path.into(),
                    line,
                    col,
                });
                continue;
            }
            if NATIVE_GLOBALS.contains(&name) {
                continue;
            }
            // Known-but-unsupported JS builtins report `mx-js-member`
            // (with the member when one is accessed) — never mx-undefined.
            if UNSUPPORTED_JS_ROOTS.contains(&name) {
                continue;
            }
            // Browser-only globals are owned by `mx-js-global` (span-precise
            // AST detection) — never mx-undefined on top.
            if UNSUPPORTED_GLOBALS.contains(&name) {
                continue;
            }
            let mut best: Option<(&str, f64)> = None;
            for candidate in
                declared.iter().chain(NATIVE_GLOBALS.iter()).chain(MORPH_BUILTINS.iter())
            {
                let score = strsim::jaro_winkler(name, candidate);
                if score > best.map_or(0.0, |(_, s)| s) {
                    best = Some((candidate, score));
                }
            }
            let suggestion = match best {
                Some((s, score)) if score > 0.7 => Some(format!("Did you mean `{s}`?")),
                _ => None,
            };
            out.push(LintError {
                severity: "error".into(),
                code: "mx-undefined".into(),
                message: format!("`{name}` is not defined here and not imported"),
                suggestion,
                file_path: file_path.into(),
                line,
                col,
            });
        }
    }
    out
}

/// Drill a member/call chain to its root identifier, through wrappers:
/// `JSON.parse`, `JSON["parse"]`, `(JSON as any).parse`, `JSON?.parse`.
/// Returns the root name plus the span start of the root identifier, so
/// locally shadowed names (which resolve, unlike true globals) are skipped.
fn member_root_name<'a>(mut cur: &'a Expression<'a>) -> Option<(&'a str, u32)> {
    loop {
        match cur {
            Expression::StaticMemberExpression(s) => cur = &s.object,
            Expression::ComputedMemberExpression(c) => cur = &c.object,
            Expression::ChainExpression(ch) => match &ch.expression {
                ChainElement::CallExpression(call) => cur = &call.callee,
                ChainElement::TSNonNullExpression(n) => cur = &n.expression,
                _ => match ch.expression.as_member_expression() {
                    Some(MemberExpression::StaticMemberExpression(s)) => cur = &s.object,
                    Some(MemberExpression::ComputedMemberExpression(c)) => cur = &c.object,
                    _ => return None,
                },
            },
            Expression::TSAsExpression(a) => cur = &a.expression,
            Expression::TSNonNullExpression(n) => cur = &n.expression,
            Expression::ParenthesizedExpression(p) => cur = &p.expression,
            Expression::Identifier(id) => return Some((id.name.as_str(), id.span.start)),
            _ => return None,
        }
    }
}

/// Property label for a member access: `min` in `Math.min` / `JSON["parse"]`.
/// Dynamic keys (`JSON[k]`) have no label — the root alone is reported.
fn member_prop_label(me: &MemberExpression) -> Option<String> {
    match me {
        MemberExpression::StaticMemberExpression(s) => Some(s.property.name.as_str().to_string()),
        MemberExpression::ComputedMemberExpression(c) => match &c.expression {
            Expression::StringLiteral(s) => Some(s.value.to_string()),
            _ => None,
        },
        MemberExpression::PrivateFieldExpression(_) => None,
    }
}

/// `mx-js-member`: JS builtins with no native counterpart. The translator
/// passes them through verbatim so they fail the C++ compile; flagging
/// them here (instead of the misleading `mx-undefined`) tells the user
/// what is actually wrong. Member accesses report the full `Root.prop`
/// span; bare references report the root. Locally shadowed names are the
/// user's own and stay quiet.
struct JsMemberVisitor {
    unresolved: HashSet<u32>,
    // Reported member spans, so bare references inside them stay quiet.
    members: Vec<(u32, u32)>,
    // Call-callee spans for lowered conversions (String(x), parseInt(...)):
    // the bare loop must not flag the callee itself. `new` forms are
    // never recorded, so they keep erroring.
    allowed_calls: HashSet<u32>,
    out: Vec<(u32, String, String)>,
}
impl JsMemberVisitor {
    fn check_root(&mut self, root: Option<(&str, u32)>, span: (u32, u32), prop: Option<String>) {
        let Some((name, start)) = root else { return };
        if !self.unresolved.contains(&start) {
            return;
        }
        if name == "console" {
            if let Some(p) = prop {
                if !CONSOLE_METHODS.contains(&p.as_str()) {
                    self.members.push(span);
                    self.out.push((span.0, "console".into(), format!("console.{p}")));
                }
            }
            return;
        }
        // `Promise<T>` is a type and `new Promise(...)` lowers, but the
        // static combinators/helpers have no runtime counterpart. Reject
        // them here instead of emitting `Promise.resolve` as raw C++.
        if name == "Promise" {
            if let Some(p) = prop {
                self.members.push(span);
                self.out.push((span.0, "Promise".into(), format!("Promise.{p}")));
            }
            return;
        }
        if let Some(p) = prop.as_ref() {
            if supported_builtin_member(name, p) {
                self.members.push(span);
                return;
            }
        }
        if UNSUPPORTED_JS_ROOTS.contains(&name) {
            self.members.push(span);
            let label = prop.map_or_else(|| name.to_string(), |p| format!("{name}.{p}"));
            self.out.push((span.0, name.to_string(), label));
        }
    }
}
impl<'a> Visit<'a> for JsMemberVisitor {
    fn visit_static_member_expression(&mut self, e: &StaticMemberExpression<'a>) {
        // The object subtree still needs visiting (deeper roots live
        // there); each node is visited once, so nothing double-reports.
        let root = member_root_name(&e.object);
        let prop = Some(e.property.name.as_str().to_string());
        self.check_root(root, (e.span.start, e.span.end), prop);
        self.visit_expression(&e.object);
    }
    fn visit_computed_member_expression(&mut self, e: &ComputedMemberExpression<'a>) {
        let root = member_root_name(&e.object);
        let prop = match &e.expression {
            Expression::StringLiteral(s) => Some(s.value.to_string()),
            _ => None,
        };
        self.check_root(root, (e.span.start, e.span.end), prop);
        self.visit_expression(&e.expression);
    }
    fn visit_call_expression(&mut self, e: &CallExpression<'a>) {
        // `JSON.parse(x)`: the callee carries the member span.
        if let Some(me) = e.callee.as_member_expression() {
            let (root, prop) = match me {
                MemberExpression::StaticMemberExpression(s) => {
                    (member_root_name(&s.object), Some(s.property.name.as_str().to_string()))
                }
                MemberExpression::ComputedMemberExpression(c) => (
                    member_root_name(&c.object),
                    match &c.expression {
                        Expression::StringLiteral(s) => Some(s.value.to_string()),
                        _ => None,
                    },
                ),
                MemberExpression::PrivateFieldExpression(_) => (None, None),
            };
            self.check_root(root, (me.span().start, me.span().end), prop);
            // Recurse into the callee object: f(Math.max(1, 2)) and
            // Math.sqrt(2).toFixed(3) must see the inner roots too.
            match me {
                MemberExpression::StaticMemberExpression(m) => {
                    self.visit_expression(&m.object);
                }
                MemberExpression::ComputedMemberExpression(m) => {
                    self.visit_expression(&m.object);
                    self.visit_expression(&m.expression);
                }
                MemberExpression::PrivateFieldExpression(m) => {
                    self.visit_expression(&m.object);
                }
            }
        }
        // Lowered conversions (String(x), parseInt(...)): the bare callee
        // is allowed here so the bare-builtin rule stays quiet for it.
        if let Expression::Identifier(id) = &e.callee {
            if CONVERSION_CALLS.contains(&id.name.as_str()) {
                self.allowed_calls.insert(e.callee.span().start);
            }
        }
        for arg in &e.arguments {
            self.visit_argument(arg);
        }
    }
    fn visit_new_expression(&mut self, e: &NewExpression<'a>) {
        if let Some(me) = e.callee.as_member_expression() {
            let root = match me {
                MemberExpression::StaticMemberExpression(s) => member_root_name(&s.object),
                MemberExpression::ComputedMemberExpression(c) => member_root_name(&c.object),
                MemberExpression::PrivateFieldExpression(_) => None,
            };
            self.check_root(root, (me.span().start, me.span().end), member_prop_label(me));
        }
        for arg in &e.arguments {
            self.visit_argument(arg);
        }
    }
    fn visit_chain_expression(&mut self, e: &ChainExpression<'a>) {
        match &e.expression {
            ChainElement::CallExpression(call) => {
                if let Some(me) = call.callee.as_member_expression() {
                    let root = match me {
                        MemberExpression::StaticMemberExpression(s) => member_root_name(&s.object),
                        MemberExpression::ComputedMemberExpression(c) => {
                            member_root_name(&c.object)
                        }
                        MemberExpression::PrivateFieldExpression(_) => None,
                    };
                    self.check_root(root, (me.span().start, me.span().end), member_prop_label(me));
                }
                for arg in &call.arguments {
                    self.visit_argument(arg);
                }
            }
            _ => {
                if let Some(me) = e.expression.as_member_expression() {
                    let root = match me {
                        MemberExpression::StaticMemberExpression(s) => member_root_name(&s.object),
                        MemberExpression::ComputedMemberExpression(c) => {
                            member_root_name(&c.object)
                        }
                        MemberExpression::PrivateFieldExpression(_) => None,
                    };
                    self.check_root(root, (e.span.start, e.span.end), member_prop_label(me));
                }
            }
        }
    }
}

fn check_js_members(program: &Program, file_path: &str, offsets: &[usize]) -> Vec<LintError> {
    let semantic = SemanticBuilder::new().with_build_nodes(true).build(program).semantic;
    let scoping = semantic.scoping();
    let mut unresolved = HashSet::new();
    let mut bare: Vec<(&str, Vec<Span>)> = Vec::new();
    for (atom, refs) in scoping.root_unresolved_references() {
        let mut spans = Vec::new();
        for ref_id in refs {
            let node_id = scoping.get_reference(*ref_id).node_id();
            let span = semantic.nodes().get_node(node_id).span();
            unresolved.insert(span.start);
            spans.push(span);
        }
        bare.push((atom, spans));
    }
    let mut visitor = JsMemberVisitor {
        unresolved,
        members: Vec::new(),
        allowed_calls: HashSet::new(),
        out: Vec::new(),
    };
    visitor.visit_program(program);
    let skip = collect_skip_spans(program);
    let mut out = Vec::new();
    for (start, root, label) in &visitor.out {
        let (line, col) = offset_to_line_col_fast(offsets, *start);
        let suggestion = if root == "console" {
            "Use console.log/warn/error/info instead".to_string()
        } else if root == "Promise" {
            "Await a runtime task (`fetch(...)`) or use `new Promise(...)`".to_string()
        } else {
            "Replace with runtime-type methods or plain operators (docs/javascript/types.md)"
                .to_string()
        };
        out.push(LintError {
            severity: "error".into(),
            code: "mx-js-member".into(),
            message: format!("Unsupported member `{label}` — no native counterpart"),
            suggestion: Some(suggestion),
            file_path: file_path.into(),
            line,
            col,
        });
    }
    // Bare references (`const f = JSON`): same rule, root-only label.
    // `console` alone is fine — only unknown methods are flagged above.
    for (name, spans) in bare {
        if !UNSUPPORTED_JS_ROOTS.contains(&name) {
            continue;
        }
        // Lowered conversion calls (String(x), Number(x), parseInt(...)):
        // the callee span is recorded by the visitor, not an error.
        for span in spans {
            if CONVERSION_CALLS.contains(&name) && visitor.allowed_calls.contains(&span.start) {
                continue;
            }
            if skip.intrinsics.contains(&span.start) {
                continue;
            }
            if skip.tags.contains(&span.start) {
                continue;
            }
            if skip.type_ranges.iter().any(|(s, e)| span.start >= *s && span.end <= *e) {
                continue;
            }
            if visitor.members.iter().any(|(s, e)| span.start >= *s && span.end <= *e) {
                continue;
            }
            let (line, col) = offset_to_line_col_fast(offsets, span.start);
            out.push(LintError {
                severity: "error".into(),
                code: "mx-js-member".into(),
                message: format!("Unsupported builtin `{name}` — no native counterpart"),
                suggestion: Some(
                    "Replace with runtime-type methods or plain operators (docs/javascript/types.md)"
                        .to_string(),
                ),
                file_path: file_path.into(),
                line,
                col,
            });
        }
    }
    out
}

/// `mx-js-op`: operators the C++ translator cannot emit. Each one below
/// was verified to produce uncompilable C++ (comment stubs like `/* pow */`
/// or verbatim `typeof`/`delete`). `in` lowers to `js_has_property` and
/// stays allowed; `instanceof` has no valid right-hand side today (builtin
/// ctors don't exist, user classes aren't `JsValue`s) so it is denied too.
fn check_js_operators(program: &Program, file_path: &str, offsets: &[usize]) -> Vec<LintError> {
    struct Visitor<'a> {
        // (span start, operator text, hint)
        out: Vec<(u32, &'a str, &'static str)>,
    }
    impl<'a> Visit<'a> for Visitor<'a> {
        fn visit_binary_expression(&mut self, e: &BinaryExpression<'a>) {
            let denied = match e.operator {
                BinaryOperator::Exponential => Some(("**", "Use repeated multiplication (`a * a`) or a `.cpp` helper")),
                BinaryOperator::ShiftRightZeroFill => {
                    Some((">>>", "Use `>>` for non-negative values, or a `.cpp` helper"))
                }
                BinaryOperator::Instanceof => Some((
                    "instanceof",
                    "Morph has no runtime type hierarchy — compare against a discriminant field instead",
                )),
                _ => None,
            };
            if let Some((op, hint)) = denied {
                self.out.push((e.span.start, op, hint));
            }
            oxc_ast_visit::walk::walk_binary_expression(self, e);
        }
        fn visit_assignment_expression(&mut self, e: &AssignmentExpression<'a>) {
            let denied = match e.operator {
                AssignmentOperator::Exponential => {
                    Some(("**=", "Use `x = x * x` or a `.cpp` helper"))
                }
                AssignmentOperator::ShiftRightZeroFill => Some((
                    ">>>=",
                    "Unsigned shift-assign has no counterpart — use `>>=` for non-negative values",
                )),
                AssignmentOperator::LogicalAnd => Some(("&&=", "Use `x = x && y`")),
                AssignmentOperator::LogicalOr => Some(("||=", "Use `x = x || y`")),
                AssignmentOperator::LogicalNullish => Some(("??=", "Use `x = x ?? y`")),
                _ => None,
            };
            if let Some((op, hint)) = denied {
                self.out.push((e.span.start, op, hint));
            }
            oxc_ast_visit::walk::walk_assignment_expression(self, e);
        }
        fn visit_unary_expression(&mut self, e: &UnaryExpression<'a>) {
            let denied = match e.operator {
                UnaryOperator::Typeof => Some((
                    "typeof",
                    "Morph is statically typed — branch on the declared type instead",
                )),
                UnaryOperator::Void => {
                    Some(("void", "Drop `void` — the bare expression evaluates the same"))
                }
                UnaryOperator::Delete => {
                    Some(("delete", "Assign `undefined` instead of deleting the property"))
                }
                _ => None,
            };
            if let Some((op, hint)) = denied {
                self.out.push((e.span.start, op, hint));
            }
            oxc_ast_visit::walk::walk_unary_expression(self, e);
        }
    }
    let mut visitor = Visitor { out: Vec::new() };
    visitor.visit_program(program);
    visitor
        .out
        .iter()
        .map(|(start, op, hint)| {
            let (line, col) = offset_to_line_col_fast(offsets, *start);
            LintError {
                severity: "error".into(),
                code: "mx-js-op".into(),
                message: format!("Operator `{op}` cannot be emitted to C++"),
                suggestion: Some((*hint).into()),
                file_path: file_path.into(),
                line,
                col,
            }
        })
        .collect()
}

/// C++ keywords that can appear as JS identifiers or as member/property
/// names. A binding emits an identifier the C++ compiler rejects
/// (`auto double`); a class field or method named `delete` fails the same
/// way, even though `delete` is a legal JS property key.
const CPP_KEYWORDS: &[&str] = &[
    "alignas",
    "alignof",
    "and",
    "and_eq",
    "asm",
    "auto",
    "bitand",
    "bitor",
    "bool",
    "break",
    "case",
    "catch",
    "char",
    "char8_t",
    "char16_t",
    "char32_t",
    "class",
    "co_await",
    "co_return",
    "co_yield",
    "compl",
    "concept",
    "const",
    "const_cast",
    "consteval",
    "constexpr",
    "constinit",
    "continue",
    "decltype",
    "default",
    "delete",
    "do",
    "double",
    "dynamic_cast",
    "else",
    "enum",
    "explicit",
    "export",
    "extern",
    "false",
    "float",
    "for",
    "friend",
    "goto",
    "if",
    "import",
    "inline",
    "int",
    "long",
    "module",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "not",
    "not_eq",
    "nullptr",
    "operator",
    "or",
    "or_eq",
    "private",
    "protected",
    "public",
    "register",
    "reinterpret_cast",
    "requires",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "static_assert",
    "static_cast",
    "struct",
    "switch",
    "template",
    "this",
    "thread_local",
    "throw",
    "true",
    "try",
    "typedef",
    "typeid",
    "typename",
    "union",
    "unsigned",
    "using",
    "virtual",
    "void",
    "volatile",
    "wchar_t",
    "while",
    "xor",
    "xor_eq",
];

/// `mx-js-syntax`: syntax constructs the translator cannot handle. Every
/// entry below was verified to produce uncompilable C++ or silently
/// dropped code (see the translator probes, not the stale coverage doc:
/// `?.`, destructuring, for-of/in, spreads in *arrays* and getters all
/// lower correctly and stay allowed).
struct JsSyntaxVisitor {
    // (span start, construct text, hint)
    out: Vec<(u32, &'static str, &'static str)>,
}
impl<'a> Visit<'a> for JsSyntaxVisitor {
    fn visit_expression(&mut self, e: &Expression<'a>) {
        match e {
            Expression::YieldExpression(y) => {
                self.out.push((
                    y.span.start,
                    "`yield`",
                    "Use an explicit state machine or an array you index through",
                ));
            }
            Expression::ImportMeta(m) => {
                self.out.push((
                    m.span.start,
                    "`import.meta`",
                    "Pass build-time values in through imports or constants",
                ));
            }
            Expression::RegExpLiteral(r) => {
                self.out.push((
                    r.span.start,
                    "regular expression literal",
                    "Use a `.cpp` helper or plain string checks instead",
                ));
            }
            _ => {}
        }
        oxc_ast_visit::walk::walk_expression(self, e);
    }
    fn visit_function(&mut self, f: &Function<'a>, flags: ScopeFlags) {
        if f.generator {
            self.out.push((
                f.span.start,
                "generator function",
                "Use an explicit state machine or an array you index through",
            ));
            return;
        }
        oxc_ast_visit::walk::walk_function(self, f, flags);
    }
    fn visit_with_statement(&mut self, e: &WithStatement<'a>) {
        self.out.push((
            e.span.start,
            "`with` statement",
            "Name the object explicitly instead of opening its scope",
        ));
    }
    fn visit_break_statement(&mut self, e: &BreakStatement<'a>) {
        if e.label.is_some() {
            self.out.push((
                e.span.start,
                "labeled `break`",
                "Restructure with a flag variable or an early return",
            ));
        }
    }
    fn visit_continue_statement(&mut self, e: &ContinueStatement<'a>) {
        if e.label.is_some() {
            self.out.push((
                e.span.start,
                "labeled `continue`",
                "Restructure with a flag variable or filter the loop input",
            ));
        }
    }
    fn visit_object_expression(&mut self, e: &ObjectExpression<'a>) {
        for prop in &e.properties {
            if let ObjectPropertyKind::SpreadProperty(sp) = prop {
                self.out.push((
                    sp.span.start,
                    "object spread",
                    "Copy the fields explicitly — spread data is dropped",
                ));
            }
        }
        oxc_ast_visit::walk::walk_object_expression(self, e);
    }
    fn visit_import_expression(&mut self, e: &ImportExpression<'a>) {
        self.out.push((
            e.span.start,
            "dynamic `import()`",
            "Import statically at the top of the file instead",
        ));
        oxc_ast_visit::walk::walk_import_expression(self, e);
    }
    fn visit_decorator(&mut self, e: &Decorator<'a>) {
        self.out.push((
            e.span.start,
            "decorator",
            "Inline the decorator's effect — decorators are dropped",
        ));
    }
    fn visit_declaration(&mut self, d: &Declaration<'a>) {
        if let Declaration::TSNamespaceDeclaration(n) = d {
            self.out.push((
                n.span.start,
                "`namespace` block",
                "Export plain functions and constants instead",
            ));
            return;
        }
        if let Declaration::TSEnumDeclaration(e) = d {
            self.out.push((
                e.span.start,
                "`enum` declaration",
                "Use a plain object const with string/number literals",
            ));
            return;
        }
        oxc_ast_visit::walk::walk_declaration(self, d);
    }
    fn visit_import_declaration(&mut self, d: &ImportDeclaration<'a>) {
        // Type-only imports are erased before codegen; their names never
        // become C++ bindings, so they cannot collide with keywords.
        if matches!(d.import_kind, ImportOrExportKind::Type) {
            return;
        }
        if d.specifiers.as_ref().is_some_and(|specs| {
            specs
                .iter()
                .any(|s| matches!(s, ImportDeclarationSpecifier::ImportNamespaceSpecifier(_)))
        }) {
            self.out.push((
                d.span.start,
                "`import * as` namespace import",
                "Import the names explicitly: `import { token, greet } from './lib.ts'`",
            ));
            return;
        }
        oxc_ast_visit::walk::walk_import_declaration(self, d);
    }
    fn visit_import_specifier(&mut self, s: &ImportSpecifier<'a>) {
        if matches!(s.import_kind, ImportOrExportKind::Type) {
            return;
        }
        oxc_ast_visit::walk::walk_import_specifier(self, s);
    }
    // Type-only syntax is erased before codegen. Its contents never
    // become runtime bindings, so rest params / keyword names inside a
    // type alias or annotation must not reject an otherwise valid file.
    fn visit_ts_type_alias_declaration(&mut self, _d: &TSTypeAliasDeclaration<'a>) {}
    fn visit_ts_interface_declaration(&mut self, _d: &TSInterfaceDeclaration<'a>) {}
    fn visit_ts_type_annotation(&mut self, _a: &TSTypeAnnotation<'a>) {}
    fn visit_formal_parameter_rest(&mut self, p: &FormalParameterRest<'a>) {
        self.out.push((
            p.span.start,
            "rest parameter (`...args`)",
            "Pass an array parameter explicitly instead",
        ));
        oxc_ast_visit::walk::walk_formal_parameter_rest(self, p);
    }
    fn visit_binding_identifier(&mut self, id: &BindingIdentifier<'a>) {
        if CPP_KEYWORDS.contains(&id.name.as_str()) {
            self.out.push((
                id.span.start,
                "C++ keyword as a binding name",
                "Rename the binding — C++ reserves this word",
            ));
        }
    }
    fn visit_property_definition(&mut self, p: &PropertyDefinition<'a>) {
        self.check_member_key(&p.key);
        oxc_ast_visit::walk::walk_property_definition(self, p);
    }
    fn visit_method_definition(&mut self, m: &MethodDefinition<'a>) {
        self.check_member_key(&m.key);
        oxc_ast_visit::walk::walk_method_definition(self, m);
    }
}

impl<'a> JsSyntaxVisitor {
    fn check_member_key(&mut self, key: &PropertyKey<'a>) {
        let (name, start) = match key {
            PropertyKey::StaticIdentifier(id) => (id.name.as_str(), id.span.start),
            PropertyKey::Identifier(id) => (id.name.as_str(), id.span.start),
            _ => return,
        };
        if CPP_KEYWORDS.contains(&name) {
            self.out.push((
                start,
                "C++ keyword as a class member name",
                "Rename the field or method — C++ reserves this word",
            ));
        }
    }
}

fn check_js_syntax(program: &Program, file_path: &str, offsets: &[usize]) -> Vec<LintError> {
    let mut st = JsSyntaxVisitor { out: Vec::new() };
    st.visit_program(program);
    st.out
        .iter()
        .map(|(start, what, hint)| {
            let (line, col) = offset_to_line_col_fast(offsets, *start);
            LintError {
                severity: "error".into(),
                code: "mx-js-syntax".into(),
                message: format!("{what} cannot be translated to C++"),
                suggestion: Some((*hint).into()),
                file_path: file_path.into(),
                line,
                col,
            }
        })
        .collect()
}
/// Receiver kinds the method rule can prove: literals, and state
/// bindings whose type argument or initializer pins them down.
/// Anything else (params, call results, reassigned bindings) stays
/// quiet — an unprovable receiver is never an error.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RecvKind {
    Str,
    Arr,
    Obj,
    Num,
    Bool,
}

/// Builtin namespace members with native lowering (mirrors the
/// translator's emit_js_builtin): anything else on these roots stays
/// an mx-js-member error.
fn supported_builtin_member(root: &str, member: &str) -> bool {
    matches!(
        (root, member),
        ("Math", "min" | "max" | "abs" | "floor" | "ceil" | "round" | "pow" | "sqrt")
            | ("JSON", "parse" | "stringify")
            | ("Number", "isFinite" | "isNaN" | "isInteger")
            | ("Object", "keys" | "values" | "entries")
            | ("Array", "isArray")
    )
}

/// Value conversions and numeric globals lowered by the translator
/// (call form only — `new String()` stays an error).
const CONVERSION_CALLS: &[&str] = &["String", "Number", "Boolean", "parseInt", "parseFloat"];

/// Supported methods per receiver kind, mirroring the translator:
/// string helpers (`StringMethod`), `JsArray::{push,pop}`,
/// `JsObject::{has,keys,get}`, numbers/booleans (`toString`).
const fn supported_methods(kind: RecvKind) -> &'static [&'static str] {
    match kind {
        RecvKind::Str => &[
            "toUpperCase",
            "toLowerCase",
            "charAt",
            "indexOf",
            "lastIndexOf",
            "substring",
            "substr",
            "slice",
            "trim",
            "trimStart",
            "trimEnd",
            "replace",
            "replaceAll",
            "split",
            "match",
            "matchAll",
            "search",
            "padStart",
            "padEnd",
            "repeat",
            "startsWith",
            "endsWith",
            "includes",
            "localeCompare",
            "normalize",
            "toLocaleUpperCase",
            "toLocaleLowerCase",
            "toString",
        ],
        RecvKind::Arr => &[
            "push",
            "pop",
            "join",
            "slice",
            "concat",
            "includes",
            "indexOf",
            "lastIndexOf",
            "at",
            "reverse",
            "toString",
        ],
        RecvKind::Obj => &["has", "keys", "get", "values", "entries"],
        RecvKind::Num => &["toString", "toFixed", "toPrecision", "toExponential"],
        RecvKind::Bool => &["toString"],
    }
}

const fn recv_kind_name(kind: RecvKind) -> &'static str {
    match kind {
        RecvKind::Str => "strings",
        RecvKind::Arr => "arrays",
        RecvKind::Obj => "objects",
        RecvKind::Num => "numbers",
        RecvKind::Bool => "booleans",
    }
}

fn sniff_state_kind(type_arg: Option<&str>, init: &str) -> Option<RecvKind> {
    if let Some(t) = type_arg.map(str::trim) {
        match t {
            "string" | "String" => return Some(RecvKind::Str),
            "number" | "Number" => return Some(RecvKind::Num),
            "boolean" | "Boolean" => return Some(RecvKind::Bool),
            _ => {}
        }
        if t.starts_with("Array") || t.ends_with("[]") {
            return Some(RecvKind::Arr);
        }
        if t.starts_with('{') || t == "object" || t == "Object" {
            return Some(RecvKind::Obj);
        }
        return None;
    }
    let init = init.trim();
    if init.starts_with('"') || init.starts_with('\'') || init.starts_with('`') {
        Some(RecvKind::Str)
    } else if init.starts_with('[') {
        Some(RecvKind::Arr)
    } else if init.starts_with('{') {
        Some(RecvKind::Obj)
    } else if init == "true" || init == "false" {
        Some(RecvKind::Bool)
    } else if init.parse::<f64>().is_ok() {
        Some(RecvKind::Num)
    } else {
        None
    }
}

/// `mx-js-method`: method calls on provably-typed receivers where the
/// runtime type has no such method. Anything unprovable stays quiet;
/// anything supported stays quiet; the rest names the exact method.
fn check_js_method_calls(
    program: &Program,
    content: &str,
    file_path: &str,
    offsets: &[usize],
) -> Vec<LintError> {
    fn recv_of<'a>(
        expr: &'a Expression<'a>,
        states: &HashMap<String, RecvKind>,
    ) -> Option<RecvKind> {
        match expr {
            Expression::StringLiteral(_) | Expression::TemplateLiteral(_) => Some(RecvKind::Str),
            Expression::ArrayExpression(_) => Some(RecvKind::Arr),
            Expression::ObjectExpression(_) => Some(RecvKind::Obj),
            Expression::NumericLiteral(_) | Expression::BigIntLiteral(_) => Some(RecvKind::Num),
            Expression::BooleanLiteral(_) => Some(RecvKind::Bool),
            Expression::Identifier(id) => states.get(id.name.as_str()).copied(),
            Expression::TSAsExpression(a) => recv_of(&a.expression, states),
            Expression::TSNonNullExpression(n) => recv_of(&n.expression, states),
            Expression::ParenthesizedExpression(p) => recv_of(&p.expression, states),
            _ => None,
        }
    }
    struct Visitor<'a> {
        states: &'a HashMap<String, RecvKind>,
        /// Spans of `.map` calls that are direct JSX children (list
        /// position): the only place list rendering lowers them.
        list_maps: HashSet<u32>,
        out: Vec<(u32, u32, String, String)>,
    }
    /// `arr.map(item => <jsx/>)` directly inside JSX children lowers
    /// through the list machinery (see the walker's `try_list_from_map`),
    /// so it is exempt — but only there. The same call anywhere else
    /// (attributes, logic, conditions) breaks and still errors.
    fn is_list_map_call(call: &CallExpression) -> bool {
        let Some(me) = call.callee.as_member_expression() else { return false };
        let MemberExpression::StaticMemberExpression(s) = me else { return false };
        if s.property.name.as_str() != "map" {
            return false;
        }
        let Some(arg) = call.arguments.first().and_then(|a| a.as_expression()) else {
            return false;
        };
        let Expression::ArrowFunctionExpression(arrow) = arg else { return false };
        let Some(body) = arrow.body.as_expression() else { return false };
        // Mirror try_jsx_from_expr: JSX, fragments, parenthesized.
        let mut cur = body;
        loop {
            match cur {
                Expression::JSXElement(_) | Expression::JSXFragment(_) => return true,
                Expression::ParenthesizedExpression(p) => cur = &p.expression,
                _ => return false,
            }
        }
    }
    impl<'a> Visitor<'a> {
        /// Record direct-child list calls before descending: the walker
        /// only list-lowers `.map` in exactly this position.
        fn note_list_children(&mut self, children: &[JSXChild<'a>]) {
            for child in children {
                if let JSXChild::ExpressionContainer(c) = child {
                    if let Some(Expression::CallExpression(call)) = c.expression.as_expression() {
                        if is_list_map_call(call) {
                            self.list_maps.insert(call.span.start);
                        }
                    }
                }
            }
        }
    }
    impl<'a> Visit<'a> for Visitor<'a> {
        fn visit_jsx_element(&mut self, e: &JSXElement<'a>) {
            self.note_list_children(&e.children);
            oxc_ast_visit::walk::walk_jsx_element(self, e);
        }
        fn visit_jsx_fragment(&mut self, e: &JSXFragment<'a>) {
            self.note_list_children(&e.children);
            oxc_ast_visit::walk::walk_jsx_fragment(self, e);
        }
        fn visit_call_expression(&mut self, e: &CallExpression<'a>) {
            if self.list_maps.contains(&e.span.start) {
                // List rendering: walk the template for nested violations
                // but never flag the `.map` itself.
                if let Some(me) = e.callee.as_member_expression() {
                    match me {
                        MemberExpression::StaticMemberExpression(s) => {
                            self.visit_expression(&s.object);
                        }
                        MemberExpression::ComputedMemberExpression(c) => {
                            self.visit_expression(&c.object);
                        }
                        MemberExpression::PrivateFieldExpression(_) => {}
                    }
                }
                for arg in &e.arguments {
                    self.visit_argument(arg);
                }
                return;
            }
            if let Some(me) = e.callee.as_member_expression() {
                let (root, prop) = match me {
                    MemberExpression::StaticMemberExpression(s) => (
                        recv_of(&s.object, self.states),
                        Some(s.property.name.as_str().to_string()),
                    ),
                    MemberExpression::ComputedMemberExpression(c) => (
                        recv_of(&c.object, self.states),
                        match &c.expression {
                            Expression::StringLiteral(s) => Some(s.value.to_string()),
                            _ => None,
                        },
                    ),
                    MemberExpression::PrivateFieldExpression(_) => (None, None),
                };
                if let (Some(kind), Some(method)) = (root, prop) {
                    if !supported_methods(kind).contains(&method.as_str()) {
                        self.out.push((
                            me.span().start,
                            me.span().end,
                            method,
                            recv_kind_name(kind).to_string(),
                        ));
                    }
                }
            }
            oxc_ast_visit::walk::walk_call_expression(self, e);
        }
        fn visit_chain_expression(&mut self, e: &ChainExpression<'a>) {
            if let ChainElement::CallExpression(call) = &e.expression {
                if let Some(me) = call.callee.as_member_expression() {
                    let root = match me {
                        MemberExpression::StaticMemberExpression(s) => {
                            recv_of(&s.object, self.states)
                        }
                        MemberExpression::ComputedMemberExpression(c) => {
                            recv_of(&c.object, self.states)
                        }
                        MemberExpression::PrivateFieldExpression(_) => None,
                    };
                    if let (Some(kind), Some(method)) = (root, member_prop_label(me)) {
                        if !supported_methods(kind).contains(&method.as_str()) {
                            self.out.push((
                                me.span().start,
                                me.span().end,
                                method,
                                recv_kind_name(kind).to_string(),
                            ));
                        }
                    }
                }
            }
            oxc_ast_visit::walk::walk_chain_expression(self, e);
        }
    }
    let states = collect_state_kinds(program, content);
    let mut visitor = Visitor { states: &states, list_maps: HashSet::new(), out: Vec::new() };
    visitor.visit_program(program);
    let mut out = Vec::new();
    for (start, _end, method, kind) in &visitor.out {
        let (line, col) = offset_to_line_col_fast(offsets, *start);
        let suggestion = if method == "length" {
            "Did you mean the `length` property (no call)?".to_string()
        } else {
            "Check the runtime type docs for supported methods, or use a `.cpp` helper".to_string()
        };
        out.push(LintError {
            severity: "error".into(),
            code: "mx-js-method".into(),
            message: format!("Method `.{method}()` is not implemented for {kind}"),
            suggestion: Some(suggestion),
            file_path: file_path.into(),
            line,
            col,
        });
    }
    out
}

/// State bindings pinned to receiver kinds, collected check-locally:
/// `const [v, setV] = morphState("ada")`, single-element patterns and
/// nested positions all count. The IR walker's `state_vars` only covers
/// top-level pairs (codegen relies on that shape), so the checker reads
/// call sites itself instead of widening walker output.
fn collect_state_kinds(program: &Program, content: &str) -> HashMap<String, RecvKind> {
    fn span_text<'s>(content: &'s str, start: u32, end: u32) -> &'s str {
        content.get(start as usize..end as usize).unwrap_or("")
    }
    fn first_binding_ident(pat: &BindingPattern) -> Option<String> {
        match pat {
            BindingPattern::BindingIdentifier(id) => Some(id.name.to_string()),
            BindingPattern::ArrayPattern(a) => {
                for e in a.elements.iter().flatten() {
                    if let Some(name) = first_binding_ident(e) {
                        return Some(name);
                    }
                }
                None
            }
            _ => None,
        }
    }
    struct Collector<'s> {
        content: &'s str,
        out: HashMap<String, RecvKind>,
    }
    impl<'a> Visit<'a> for Collector<'_> {
        fn visit_variable_declarator(&mut self, d: &VariableDeclarator<'a>) {
            if let Some(Expression::CallExpression(call)) = &d.init {
                if let Expression::Identifier(id) = &call.callee {
                    let name = id.name.as_str();
                    if name == "morphState" || name == "morphShared" {
                        let type_arg = call
                            .type_arguments
                            .as_ref()
                            .and_then(|t| t.params.first())
                            .map(|p| span_text(self.content, p.span().start, p.span().end));
                        let init = call
                            .arguments
                            .first()
                            .and_then(|a| a.as_expression())
                            .map(|e| span_text(self.content, e.span().start, e.span().end))
                            .unwrap_or("");
                        if let (Some(getter), Some(kind)) =
                            (first_binding_ident(&d.id), sniff_state_kind(type_arg, init))
                        {
                            self.out.insert(getter, kind);
                        }
                    }
                }
            }
            // Plain literals used as locals: `const xs = [1, 2, 3]`
            // makes `xs.sort()` provably an array method. Keep the
            // morphState-derived kind when both apply.
            if let (BindingPattern::BindingIdentifier(id), Some(init)) = (&d.id, &d.init) {
                let text = span_text(self.content, init.span().start, init.span().end);
                if let Some(kind) = sniff_state_kind(None, text) {
                    self.out.entry(id.name.to_string()).or_insert(kind);
                }
            }
            oxc_ast_visit::walk::walk_variable_declarator(self, d);
        }
    }
    let mut collector = Collector { content, out: HashMap::new() };
    collector.visit_program(program);
    collector.out
}

/// `mx-js-global`: browser-only globals with no native counterpart.
/// Resolved from real scope references (not substring matching), so string
/// contents, comments, JSX text and user properties (`x.document`) stay
/// quiet while every syntactic use (`(document).title`, newlines between
/// object and member) is caught at its exact span.
fn check_js_globals(program: &Program, file_path: &str, offsets: &[usize]) -> Vec<LintError> {
    let semantic = SemanticBuilder::new().with_build_nodes(true).build(program).semantic;
    let scoping = semantic.scoping();
    let skip = collect_skip_spans(program);
    let mut out = Vec::new();
    let mut names: Vec<(&str, Vec<Span>)> = Vec::new();
    for (atom, refs) in scoping.root_unresolved_references() {
        if !UNSUPPORTED_GLOBALS.contains(&atom.as_str()) {
            continue;
        }
        let mut spans = Vec::new();
        for ref_id in refs {
            let node_id = scoping.get_reference(*ref_id).node_id();
            spans.push(semantic.nodes().get_node(node_id).span());
        }
        names.push((atom, spans));
    }
    names.sort_by(|a, b| a.0.cmp(b.0));
    for (name, spans) in names {
        for span in spans {
            if skip.intrinsics.contains(&span.start) || skip.tags.contains(&span.start) {
                continue;
            }
            if skip.type_ranges.iter().any(|(s, e)| span.start >= *s && span.end <= *e) {
                continue;
            }
            let (line, col) = offset_to_line_col_fast(offsets, span.start);
            out.push(LintError {
                severity: "error".into(),
                code: "mx-js-global".into(),
                message: format!("`{name}` is not available in native runtime"),
                suggestion: Some(format!("Use Morph state / C++ instead of browser `{name}`")),
                file_path: file_path.into(),
                line,
                col,
            });
        }
    }
    out
}

fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn offset_to_line_col_fast(offsets: &[usize], offset: u32) -> (usize, usize) {
    let off = offset as usize;
    // binary search for line
    let line = match offsets.binary_search(&off) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    };
    let line_start = offsets[line.saturating_sub(1)];
    let col = off.saturating_sub(line_start) + 1;
    (line, col)
}

/// Check a .mx file for parse + semantic errors (with caching support via caller)
pub fn check(source: &str, file_path: &str) -> Vec<LintError> {
    let offsets = build_line_offsets(source);
    let allocator = Allocator::default();
    let source_type = SourceType::from_path("file.tsx").unwrap();
    let ret = Parser::new(&allocator, source, source_type).parse();
    let mut errors = Vec::new();

    for diag in &ret.diagnostics {
        let span = diag.labels.first().map_or(Span::new(0, 0), LabeledSpan::span);
        let (line, col) = offset_to_line_col_fast(&offsets, span.start);
        errors.push(LintError {
            severity: "error".into(),
            code: "parse-error".into(),
            message: diag.message.to_string(),
            suggestion: diag.help.as_ref().map(ToString::to_string),
            file_path: file_path.into(),
            line,
            col,
        });
    }

    if ret.panicked {
        errors.push(LintError {
            severity: "error".into(),
            code: "parse-panic".into(),
            message: "Parser panicked — unrecoverable syntax error".into(),
            suggestion: Some("Check for unmatched braces or truncated file".into()),
            file_path: file_path.into(),
            line: 1,
            col: 1,
        });
        return errors;
    }

    // Unresolved identifiers: real scope analysis (imports, params,
    // shadowing all count as declared). Morph APIs without their
    // 'morph' import get their own code; lowercase JSX intrinsics
    // (`<div>`) are excluded by span. Skipped when parsing failed —
    // the semantic builder assumes a well-formed tree.
    if ret.diagnostics.is_empty() {
        errors.extend(check_unresolved(&ret.program, file_path, &offsets));
        // Unsupported JS builtins (member spans + bare roots): their own
        // code, so check_unresolved stays quiet about them (see above).
        errors.extend(check_js_members(&ret.program, file_path, &offsets));
        // Browser-only globals, detected from scope references.
        errors.extend(check_js_globals(&ret.program, file_path, &offsets));
        // Unemittable operators (`**`, `typeof`, `delete`, ...).
        errors.extend(check_js_operators(&ret.program, file_path, &offsets));
        // Unhandleable syntax (generators, `with`, object spread, ...).
        errors.extend(check_js_syntax(&ret.program, file_path, &offsets));
        // Unimplemented methods on provably-typed receivers.
        errors.extend(check_js_method_calls(&ret.program, source, file_path, &offsets));
    }

    // Walk program to build MxSource for semantic lints (even with parse errors, program is partial)
    let mut walker = crate::js_walker::MxWalker::new(source);
    {
        walker.visit_program(&ret.program);
    }
    let mx_source = MxSource {
        filename: file_path.into(),
        imports: walker.imports,
        window_config: walker.window_config,
        components: walker.components,
        shared_bindings: walker.shared_bindings,
        event_bindings: walker.event_bindings,
        state_vars: walker.state_vars,
        effects: walker.effects,
        inner_functions: walker.inner_functions,
        function_declarations: walker.function_declarations,
        class_declarations: walker.class_declarations,
        exported_vars: walker.exported_vars,
        named_exports: walker.named_exports,
        default_export: walker.default_export,
        re_exports: walker.re_exports,
        global_vars: walker.global_vars,
        global_var_names: walker.global_var_names,
        console_logs: walker.console_logs,
        extra_headers: walker.extra_headers,
        cpp_imports: walker.cpp_imports,
    };

    let lints = lint(&mx_source, source, file_path);
    errors.extend(lints);
    errors
}

// ── Registry mirrors Python's checker/registry.py ──────────────────────────

static SUPPORTED_TAGS: &[&str] = &[
    "div",
    "span",
    "p",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "button",
    "input",
    "img",
    "a",
    "ul",
    "ol",
    "li",
    "table",
    "thead",
    "tbody",
    "tr",
    "td",
    "th",
    "form",
    "label",
    "section",
    "header",
    "footer",
    "nav",
    "main",
    "article",
    "aside",
    "body",
    "view",
    "text",
    "morph-window",
    "fragment",
];

static STUB_TAGS: &[&str] = &["select", "textarea"];

static GLOBAL_PROPS: &[&str] = &["className", "class", "id", "style", "key"];

static EVENT_PROPS: &[&str] = &[
    "onClick",
    "onDoubleClick",
    "onMouseDown",
    "onMouseUp",
    "onMouseEnter",
    "onMouseLeave",
    "onKeyUp",
    "onKeyDown",
    "onChange",
    "onInput",
    "onFocus",
    "onBlur",
];

static TAG_PROPS: &[(&str, &[&str])] = &[
    ("img", &["src", "alt", "width", "height"]),
    ("a", &["href", "target"]),
    ("input", &["value", "maxLength", "minLength", "placeholder", "disabled", "type"]),
    (
        "morph-window",
        &["title", "width", "height", "minWidth", "maxWidth", "minHeight", "maxHeight"],
    ),
];

#[inline]
fn is_supported_tag(tag: &str) -> bool {
    SUPPORTED_TAGS.contains(&tag)
}

#[inline]
fn is_component_tag(tag: &str) -> bool {
    tag.chars().next().is_some_and(char::is_uppercase)
}

#[inline]
fn is_allowed_prop(tag: &str, prop: &str) -> bool {
    if GLOBAL_PROPS.contains(&prop) {
        return true;
    }
    if EVENT_PROPS.contains(&prop) {
        return true;
    }
    if prop.starts_with("data-") || prop.starts_with("aria-") {
        return true;
    }
    for (t, props) in TAG_PROPS {
        if *t == tag && props.contains(&prop) {
            return true;
        }
    }
    false
}

static UNSUPPORTED_GLOBALS: &[&str] = &[
    "document",
    "window",
    "localStorage",
    "sessionStorage",
    "navigator",
    "location",
    "history",
    "screen",
    "alert",
    "prompt",
    "confirm",
    "requestAnimationFrame",
];

/// JS builtins with no native counterpart: the translator passes them
/// through verbatim, so they fail the C++ compile. Reported as
/// `mx-js-member` (not the misleading `mx-undefined`) with the member
/// when one is accessed (`JSON.parse`) or the bare root otherwise.
static UNSUPPORTED_JS_ROOTS: &[&str] = &[
    "Math",
    "JSON",
    "Date",
    "RegExp",
    "Object",
    "Array",
    "Number",
    "String",
    "Boolean",
    "Symbol",
    "BigInt",
    "Map",
    "Set",
    "WeakMap",
    "WeakSet",
    "WeakRef",
    "Proxy",
    "Reflect",
    "Intl",
    "Atomics",
    "DataView",
    "ArrayBuffer",
    "SharedArrayBuffer",
    "Int8Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "Int16Array",
    "Uint16Array",
    "Int32Array",
    "Uint32Array",
    "Float32Array",
    "Float64Array",
    "BigInt64Array",
    "BigUint64Array",
];

/// `console` itself is supported, but only these methods exist natively.
static CONSOLE_METHODS: &[&str] = &["log", "warn", "error", "info"];

pub fn lint(source: &MxSource, content: &str, file_path: &str) -> Vec<LintError> {
    let mut out = Vec::new();

    // ── mx-export: exactly one export default function ──
    // Skip for logic-only modules (.ts files, or modules that have shared/event
    // bindings or helper functions but no components — these are stores/helpers
    // imported by other modules).
    let has_bindings = !source.shared_bindings.is_empty()
        || !source.event_bindings.is_empty()
        || !source.function_declarations.is_empty()
        || !source.global_vars.is_empty();
    let is_logic_module = source.filename.ends_with(".ts") && !source.filename.ends_with(".tsx");
    let skip_component_check = is_logic_module || has_bindings;
    if !skip_component_check {
        if source.components.is_empty() {
            out.push(LintError {
                severity: "error".into(),
                code: "mx-export".into(),
                message: "No component found — expected `export default function App()`".into(),
                suggestion: Some(
                    "Add `export default function App() { return (<div>...</div>) }`".into(),
                ),
                file_path: file_path.into(),
                line: 1,
                col: 1,
            });
        } else if source.components.iter().filter(|c| c.is_default).count() > 1 {
            out.push(LintError {
                severity: "error".into(),
                code: "mx-export".into(),
                message: "Multiple default exports — only one allowed".into(),
                suggestion: Some("Keep a single `export default function App()`".into()),
                file_path: file_path.into(),
                line: 1,
                col: 1,
            });
        }
    }

    // ── window checks ──
    if !skip_component_check && source.window_config.is_none() && !source.components.is_empty() {
        let has_morph_window =
            source.components.iter().any(|c| jsx_has_tag(&c.jsx, "morph-window"));
        if !has_morph_window {
            out.push(LintError {
                severity: "warning".into(),
                code: "mx-window-missing".into(),
                message: "Missing `windowConfig` export and no <morph-window>".into(),
                suggestion: Some(
                    "Add `export const windowConfig = { title: \"App\", width: 800, height: 600 }`"
                        .into(),
                ),
                file_path: file_path.into(),
                line: 1,
                col: 1,
            });
        }
    }

    // ── Walk JSX for tag/prop/list/JS global checks ──
    for comp in &source.components {
        lint_jsx(&comp.jsx, content, file_path, &mut out);
    }

    // ── Component usage checks (single-file) ──
    lint_component_usages(source, file_path, &mut out);

    // ── Inline style objects: same registry as stylesheets, but an error
    // (a layout that silently differs is worse than a dropped declaration).
    lint_inline_styles(source, file_path, &mut out);

    // ── Scope + API checks (morphState/morphShared/morphEvent placement,
    //    morphOn/morphEmit removal) ──
    out.extend(lint_state_event_scope(source, content, file_path));

    // ── CSS.load deprecation ──
    for imp in &source.imports {
        if imp.style == "css_load" {
            let path = imp.kind.path().unwrap_or("stylesheet");
            out.push(LintError {
                severity: "warning".into(),
                code: "mx-css-load-deprecated".into(),
                message: format!("`CSS.load(\"{path}\")` is deprecated"),
                suggestion: Some(format!("Use `import \"{path}\"` instead")),
                file_path: file_path.into(),
                line: 1,
                col: 1,
            });
        }
    }

    // ── Deduplicate by code+line+col
    let mut seen = HashSet::new();
    out.retain(|e| seen.insert((e.code.clone(), e.line, e.col, e.message.clone())));

    out
}

fn jsx_has_tag(node: &JsxNode, target: &str) -> bool {
    match node {
        JsxNode::Element { tag, children, .. } => {
            if tag == target {
                return true;
            }
            children.iter().any(|c| jsx_has_tag(c, target))
        }
        JsxNode::Fragment { children, .. } => children.iter().any(|c| jsx_has_tag(c, target)),
        JsxNode::Conditional { then_branch, else_branch, .. } => {
            then_branch.iter().any(|c| jsx_has_tag(c, target))
                || else_branch.iter().any(|c| jsx_has_tag(c, target))
        }
        JsxNode::List { item_template, .. } => jsx_has_tag(item_template, target),
        _ => false,
    }
}

fn lint_jsx(node: &JsxNode, _content: &str, file_path: &str, out: &mut Vec<LintError>) {
    match node {
        JsxNode::Element { tag, props, children, line, col, .. } => {
            // ── mx-tag ──
            if !is_supported_tag(tag) && !is_component_tag(tag) && !tag.starts_with("__") {
                if STUB_TAGS.contains(&tag.as_str()) {
                    out.push(LintError {
                        severity: "warning".into(),
                        code: "mx-tag-stub".into(),
                        message: format!("Tag <{tag}> is registered but not fully implemented"),
                        suggestion: Some(format!(
                            "Use <div> with custom handling instead of <{tag}>"
                        )),
                        file_path: file_path.into(),
                        line: *line,
                        col: *col,
                    });
                } else {
                    let suggestion = suggest_tag(tag);
                    out.push(LintError {
                        severity: "error".into(),
                        code: "mx-tag".into(),
                        message: format!("Unknown tag <{tag}>"),
                        suggestion: suggestion.map(|s| format!("Did you mean <{s}>?")),
                        file_path: file_path.into(),
                        line: *line,
                        col: *col,
                    });
                }
            }

            // ── mx-prop ──
            for prop in props.keys() {
                if prop == "class" {
                    out.push(LintError {
                        severity: "warning".into(),
                        code: "mx-prop".into(),
                        message: format!("Use `className` instead of `class` on <{tag}>"),
                        suggestion: Some("Replace `class` with `className`".into()),
                        file_path: file_path.into(),
                        line: *line,
                        col: *col,
                    });
                    continue;
                }
                if !is_allowed_prop(tag, prop) {
                    // Component tags declare their own props (checked by
                    // `lint_component_usages` / `lint_graph`); the generic
                    // event-name hint only applies to native elements.
                    if prop.starts_with("on") && !is_component_tag(tag) {
                        out.push(LintError {
                            severity: "warning".into(),
                            code: "mx-prop".into(),
                            message: format!("Unknown prop `{prop}` on <{tag}>"),
                            suggestion: Some("Check event name (onClick, onInput, etc.)".into()),
                            file_path: file_path.into(),
                            line: *line,
                            col: *col,
                        });
                    } else if SUPPORTED_TAGS.contains(&tag.as_str())
                        && !is_component_tag(tag)
                        && prop.len() < 20
                    {
                        if let Some(s) = suggest_prop_for_tag(prop, tag) {
                            out.push(LintError {
                                severity: "warning".into(),
                                code: "mx-prop".into(),
                                message: format!("Unknown prop `{prop}` on <{tag}>"),
                                suggestion: Some(format!("Did you mean `{s}`?")),
                                file_path: file_path.into(),
                                line: *line,
                                col: *col,
                            });
                        }
                    }
                }
            }

            for child in children {
                lint_jsx(child, _content, file_path, out);
            }
        }
        JsxNode::Fragment { children, .. } => {
            for child in children {
                lint_jsx(child, _content, file_path, out);
            }
        }
        JsxNode::Conditional { then_branch, else_branch, .. } => {
            for c in then_branch {
                lint_jsx(c, _content, file_path, out);
            }
            for c in else_branch {
                lint_jsx(c, _content, file_path, out);
            }
        }
        JsxNode::List { key_expr, item_template, line, col, .. } => {
            if key_expr.is_empty() {
                out.push(LintError {
                    severity: "warning".into(),
                    code: "mx-list-key".into(),
                    message: "List rendering without `key` prop — may cause state mismatches"
                        .into(),
                    suggestion: Some(
                        "Add `key={item.id}` to the root element inside `.map()`".into(),
                    ),
                    file_path: file_path.into(),
                    line: *line,
                    col: *col,
                });
            }
            lint_jsx(item_template, _content, file_path, out);
        }
        _ => {}
    }
}

fn suggest_prop_for_tag(input: &str, tag: &str) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    let candidates = GLOBAL_PROPS.iter().copied().chain(EVENT_PROPS.iter().copied()).chain(
        TAG_PROPS.iter().filter(|(t, _)| *t == tag).flat_map(|(_, props)| props.iter().copied()),
    );
    for prop in candidates {
        // Jaro-Winkler like the unresolved-name hints: transposed typos
        // (`widht` → `width`) score high, unrelated names stay quiet.
        let sim = strsim::jaro_winkler(input, prop);
        if sim > 0.7 {
            if let Some((_, best_sim)) = &best {
                if sim > *best_sim {
                    best = Some((prop.to_string(), sim));
                }
            } else {
                best = Some((prop.to_string(), sim));
            }
        }
    }
    best.map(|(s, _)| s)
}

/// One component instantiation site found while walking JSX.
struct ComponentUse {
    tag: String,
    props: Vec<String>,
    line: usize,
    col: usize,
}

fn collect_component_uses(node: &JsxNode, out: &mut Vec<ComponentUse>) {
    match node {
        JsxNode::Element { tag, props, children, line, col, .. } => {
            if is_component_tag(tag) && !tag.starts_with("__") {
                out.push(ComponentUse {
                    tag: tag.clone(),
                    props: props.keys().cloned().collect(),
                    line: *line,
                    col: *col,
                });
            }
            for child in children {
                collect_component_uses(child, out);
            }
        }
        JsxNode::Fragment { children, .. } => {
            for child in children {
                collect_component_uses(child, out);
            }
        }
        JsxNode::Conditional { then_branch, else_branch, .. } => {
            for c in then_branch {
                collect_component_uses(c, out);
            }
            for c in else_branch {
                collect_component_uses(c, out);
            }
        }
        JsxNode::List { item_template, .. } => collect_component_uses(item_template, out),
        _ => {}
    }
}

/// Inline `style={{...}}` validation: keys are already kebab-case (the
/// walker normalizes them), so the stylesheet registry decides directly.
/// Unlike stylesheets this is an error — see `mx-style-prop`.
fn lint_inline_styles(source: &MxSource, file_path: &str, out: &mut Vec<LintError>) {
    fn walk(node: &JsxNode, file_path: &str, out: &mut Vec<LintError>) {
        match node {
            JsxNode::Element { tag, props, children, line, col, .. } => {
                if let Some(JsxPropValue::Style(map)) = props.get("style") {
                    let mut keys: Vec<&String> = map.keys().collect();
                    keys.sort();
                    for key in keys {
                        if !crate::css_registry::is_known_property(key) {
                            out.push(LintError {
                                severity: "error".into(),
                                code: "mx-style-prop".into(),
                                message: format!(
                                    "Unsupported style property `{key}` on <{tag}>"
                                ),
                                suggestion: Some(
                                    "See the supported CSS list — use Flexbox (`display: flex`) for layout"
                                        .into(),
                                ),
                                file_path: file_path.into(),
                                line: *line,
                                col: *col,
                            });
                        }
                    }
                }
                for child in children {
                    walk(child, file_path, out);
                }
            }
            JsxNode::Fragment { children, .. } => {
                for child in children {
                    walk(child, file_path, out);
                }
            }
            JsxNode::Conditional { then_branch, else_branch, .. } => {
                for c in then_branch.iter().chain(else_branch.iter()) {
                    walk(c, file_path, out);
                }
            }
            JsxNode::List { item_template, .. } => {
                walk(item_template, file_path, out);
            }
            _ => {}
        }
    }
    for comp in &source.components {
        walk(&comp.jsx, file_path, out);
    }
}

/// Single-file component validation: unknown tags, plus prop checks
/// against locally-declared components. Cross-file imports are resolved
/// by `lint_graph`; here an imported name is accepted without prop checks
/// (the IR builder enforces those at build time).
fn lint_component_usages(source: &MxSource, file_path: &str, out: &mut Vec<LintError>) {
    let local: HashMap<&str, &crate::ast_types::MxComponent> =
        source.components.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut imported: HashSet<&str> = HashSet::new();
    for imp in &source.imports {
        if let MxImportKind::Component { default, specifiers, .. } = &imp.kind {
            if let Some(d) = default {
                imported.insert(d.as_str());
            }
            for (local, _) in specifiers {
                imported.insert(local.as_str());
            }
        }
    }
    for comp in &source.components {
        let mut uses = Vec::new();
        collect_component_uses(&comp.jsx, &mut uses);
        for u in uses {
            let Some(decl) = local.get(u.tag.as_str()) else {
                if imported.contains(u.tag.as_str()) {
                    continue;
                }
                out.push(LintError {
                    severity: "error".into(),
                    code: "mx-component-unknown".into(),
                    message: format!("Unknown component `<{}>`", u.tag),
                    suggestion: Some(format!(
                        "`{}` is not defined here and not imported — add `import {} from './...mx'`",
                        u.tag, u.tag
                    )),
                    file_path: file_path.into(),
                    line: u.line,
                    col: u.col,
                });
                continue;
            };
            check_call_props(decl, &u, file_path, out);
        }
    }
}

/// Shared prop validation shared by single-file and graph lints.
fn check_call_props(
    decl: &crate::ast_types::MxComponent,
    u: &ComponentUse,
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    let call: Vec<&str> =
        u.props.iter().map(String::as_str).filter(|p| *p != "key" && *p != "mid").collect();
    if decl.props.is_empty() && decl.props_param.is_empty() {
        if let Some(name) = call.first() {
            out.push(LintError {
                severity: "error".into(),
                code: "mx-component-prop".into(),
                message: format!("`<{0}>` takes no props but got `{name}`", u.tag),
                suggestion: Some(format!("Declare props on `{}` to accept `{name}`", decl.name)),
                file_path: file_path.into(),
                line: u.line,
                col: u.col,
            });
        }
        return;
    }
    if decl.props.is_empty() {
        return;
    }
    for prop in &decl.props {
        if !prop.optional && !call.contains(&prop.name.as_str()) {
            let known = decl.props.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ");
            out.push(LintError {
                severity: "error".into(),
                code: "mx-component-required".into(),
                message: format!("`<{0}>` is missing required prop `{1}`", u.tag, prop.name),
                suggestion: Some(format!("`{}` declares: {known}", decl.name)),
                file_path: file_path.into(),
                line: u.line,
                col: u.col,
            });
        }
    }
    for name in call {
        if decl.prop(name).is_none() {
            let known = decl.props.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", ");
            out.push(LintError {
                severity: "error".into(),
                code: "mx-component-prop".into(),
                message: format!("Unknown prop `{name}` on `<{0}>`", u.tag),
                suggestion: Some(format!("`{}` declares: {known}", decl.name)),
                file_path: file_path.into(),
                line: u.line,
                col: u.col,
            });
        }
    }
}

/// Walk the Oxc AST to enforce scope rules for morph framework calls.
///
/// - `morphState` may only appear inside a component body.
/// - `morphShared` / `morphEvent` may only appear at module scope, exported.
/// - `morphOn` / `morphEmit` (old string-key API) are always errors, as is the
///   old `morphShared("key", init)` / `morphEvent("key")` string-key form.
fn lint_state_event_scope(source: &MxSource, content: &str, file_path: &str) -> Vec<LintError> {
    let mut components: HashSet<&str> = source.components.iter().map(|c| c.name.as_str()).collect();
    // A named default export (`export default function App`) is also a
    // component; scope checking keys anonymous default exports as "default".
    if source.components.iter().any(|c| c.is_default) {
        components.insert("default");
    }
    let offsets = build_line_offsets(content);
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, content, SourceType::from_path("file.tsx").unwrap()).parse();
    if ret.panicked {
        return Vec::new();
    }
    // `export { theme, setTheme }` exports the same bindings as an inline
    // `export const [theme, setTheme] = …`; pass that through so the scope
    // check accepts both spellings.
    let exported_locals: HashSet<&str> =
        source.named_exports.iter().map(|(local, _)| local.as_str()).collect();
    let mut out = Vec::new();
    for stmt in &ret.program.body {
        let listed = match stmt {
            Statement::VariableDeclaration(vd) => vd.declarations.iter().any(|d| {
                binding_ident_name(&d.id).is_some_and(|n| exported_locals.contains(n.as_str()))
                    || binding_array_elements(&d.id)
                        .into_iter()
                        .flatten()
                        .flatten()
                        .any(|n| exported_locals.contains(n.as_str()))
            }),
            _ => false,
        };
        scope_stmt(stmt, &components, false, listed, 0, false, &offsets, file_path, &mut out);
    }
    out
}

/// State initializer shapes the signal emitter implements: plain data
/// literals only. Anything else becomes `Signal<auto>` (or references
/// names invisible in component scope) and fails C++ compile.
fn is_plain_state_init(expr: &Expression) -> bool {
    match expr {
        Expression::StringLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BooleanLiteral(_) => true,
        Expression::ArrayExpression(arr) => arr.elements.iter().all(|e| match e.as_expression() {
            Some(inner) => is_plain_state_init(inner),
            // Holes and spreads have no literal initializer form.
            None => false,
        }),
        // Negative integer literals (`-5`); anything else unary breaks.
        Expression::UnaryExpression(u) => {
            matches!(u.operator, UnaryOperator::UnaryNegation)
                && matches!(&u.argument, Expression::NumericLiteral(_))
        }
        _ => false,
    }
}

/// Shared shape check for `morphState`/`morphShared` initializers.
/// Returns silently when there is nothing to check (arity errors are
/// reported by the caller's own rule).
fn check_state_init(
    call: &CallExpression,
    file_path: &str,
    offsets: &[usize],
    out: &mut Vec<LintError>,
) {
    let Some(arg) = call.arguments.first().and_then(|a| a.as_expression()) else {
        return;
    };
    if is_plain_state_init(arg) {
        return;
    }
    let (hint, what) = match arg {
        Expression::ObjectExpression(_) => {
            ("use one morphState per field instead of an object", "object state initializers")
        }
        Expression::Identifier(_) => {
            ("initialize with a literal value instead of a variable", "variable state initializers")
        }
        _ => (
            "initialize with a plain literal (string, number, boolean, or array of literals)",
            "non-literal state initializers",
        ),
    };
    scope_error(
        "mx-state-init",
        format!("{what} cannot be translated to C++"),
        hint.into(),
        file_path,
        offsets,
        arg.span().start,
        out,
    );
}

/// Initializer shapes the component-const pipeline still cannot lower.
/// Arrow/function/object consts now lower (functions as inner functions,
/// objects as `JsObject`); class expressions have no native equivalent.
const fn unlowerable_const_init(init: &Expression) -> Option<(&'static str, &'static str)> {
    match init {
        Expression::ClassExpression(_) => {
            Some(("class expressions assigned to consts", "use a class declaration instead"))
        }
        _ => None,
    }
}

/// `const [value, setValue] = morphState(...)`: array pattern, at least
/// two plain identifier elements. Anything else on a state call breaks
/// the syntactic wiring codegen depends on.
fn is_state_pair(pat: &BindingPattern) -> bool {
    if let BindingPattern::ArrayPattern(a) = pat {
        a.elements.len() >= 2
            && a.elements.iter().all(|e| matches!(e, Some(BindingPattern::BindingIdentifier(_))))
    } else {
        false
    }
}

/// Name of a simple identifier binding, if the pattern is a plain identifier.
fn binding_ident_name(bp: &BindingPattern) -> Option<String> {
    if let BindingPattern::BindingIdentifier(id) = bp {
        Some(id.name.to_string())
    } else {
        None
    }
}

/// Identifier names bound by an array pattern (`[a, b]`), holes included as
/// `None`.
fn binding_array_elements(bp: &BindingPattern) -> Option<Vec<Option<String>>> {
    if let BindingPattern::ArrayPattern(arr) = bp {
        Some(arr.elements.iter().map(|e| e.as_ref().and_then(binding_ident_name)).collect())
    } else {
        None
    }
}

fn scope_error(
    code: &str,
    message: String,
    suggestion: String,
    file_path: &str,
    offsets: &[usize],
    span_start: u32,
    out: &mut Vec<LintError>,
) {
    let (line, col) = offset_to_line_col_fast(offsets, span_start);
    out.push(LintError {
        severity: "error".into(),
        code: code.into(),
        message,
        suggestion: Some(suggestion),
        file_path: file_path.into(),
        line,
        col,
    });
}

/// `morphEffect(...)` calls nested anywhere inside one top-level statement.
/// Effects lift to one native closure per component at a fixed build-time
/// point; inside a branch/loop there is no lift point and the call reaches
/// codegen as an unknown function (mx-effect-nested).
#[derive(Default)]
struct NestedEffectCalls {
    spans: Vec<u32>,
}

impl<'a> Visit<'a> for NestedEffectCalls {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if let Expression::Identifier(id) = &call.callee {
            if id.name.as_str() == "morphEffect" {
                self.spans.push(call.span.start);
            }
        }
        oxc_ast_visit::walk::walk_call_expression(self, call);
    }
}

fn is_morph_effect_call(expr: &Expression) -> bool {
    matches!(
        expr,
        Expression::CallExpression(call)
            if matches!(&call.callee, Expression::Identifier(id) if id.name.as_str() == "morphEffect")
    )
}

/// Report every `morphEffect` call in a component-body statement that is
/// not itself the supported top-level form. A top-level `morphEffect(...)`
/// statement still gets scanned *inside* its arguments, so a nested effect
/// in its callback is caught too.
fn check_nested_effects(
    stmt: &Statement,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    let mut finder = NestedEffectCalls::default();
    match stmt {
        Statement::ExpressionStatement(es) if is_morph_effect_call(&es.expression) => {
            if let Expression::CallExpression(call) = &es.expression {
                for arg in &call.arguments {
                    finder.visit_argument(arg);
                }
            }
        }
        _ => finder.visit_statement(stmt),
    }
    report_nested_effects(finder.spans, offsets, file_path, out);
}

fn report_nested_effects(
    spans: Vec<u32>,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    for start in spans {
        scope_error(
            "mx-effect-nested",
            "`morphEffect` must be a top-level statement of the component body".into(),
            "Move the effect out of the branch/loop and guard its work with `if` on state".into(),
            file_path,
            offsets,
            start,
            out,
        );
    }
}

fn scope_stmt(
    stmt: &Statement,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    body_top: bool,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match stmt {
        Statement::ExportDefaultDeclaration(d) => {
            let decl_in_comp = components.contains("default") || in_component;
            match &d.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(f) => {
                    scope_function(
                        f,
                        components,
                        decl_in_comp,
                        func_depth,
                        decl_in_comp,
                        offsets,
                        file_path,
                        out,
                    );
                }
                ExportDefaultDeclarationKind::ArrowFunctionExpression(a) => {
                    scope_arrow(
                        a,
                        components,
                        decl_in_comp,
                        func_depth,
                        decl_in_comp,
                        offsets,
                        file_path,
                        out,
                    );
                }
                ExportDefaultDeclarationKind::FunctionExpression(f) => {
                    scope_function(
                        f,
                        components,
                        decl_in_comp,
                        func_depth,
                        decl_in_comp,
                        offsets,
                        file_path,
                        out,
                    );
                }
                ExportDefaultDeclarationKind::ParenthesizedExpression(p) => match &p.expression {
                    Expression::ArrowFunctionExpression(a) => {
                        scope_arrow(
                            a,
                            components,
                            decl_in_comp,
                            func_depth,
                            decl_in_comp,
                            offsets,
                            file_path,
                            out,
                        );
                    }
                    Expression::FunctionExpression(f) => {
                        scope_function(
                            f,
                            components,
                            decl_in_comp,
                            func_depth,
                            decl_in_comp,
                            offsets,
                            file_path,
                            out,
                        );
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        Statement::ExportDeclaration(d) => walk_declaration(
            &d.declaration,
            components,
            in_component,
            true,
            func_depth,
            false,
            offsets,
            file_path,
            out,
        ),
        Statement::FunctionDeclaration(f) => {
            let is_comp = f.id.as_ref().is_some_and(|id| components.contains(id.name.as_str()));
            scope_function(
                f,
                components,
                in_component || is_comp,
                func_depth,
                is_comp,
                offsets,
                file_path,
                out,
            );
        }
        Statement::VariableDeclaration(vd) => {
            for decl in &vd.declarations {
                scope_declarator(
                    decl,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    body_top,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Statement::ExpressionStatement(es) => scope_expr(
            &es.expression,
            components,
            in_component,
            in_export,
            func_depth,
            offsets,
            file_path,
            out,
        ),
        Statement::ReturnStatement(ret) => {
            if let Some(arg) = &ret.argument {
                scope_expr(
                    arg,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Statement::BlockStatement(blk) => {
            for s in &blk.body {
                scope_stmt(
                    s,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    false,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Statement::IfStatement(cond) => {
            scope_expr(
                &cond.test,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_stmt(
                &cond.consequent,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
            if let Some(alt) = &cond.alternate {
                scope_stmt(
                    alt,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    false,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Statement::ForStatement(f) => {
            if let Some(init) = &f.init {
                scope_for_init(
                    init,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            if let Some(test) = &f.test {
                scope_expr(
                    test,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            if let Some(upd) = &f.update {
                scope_expr(
                    upd,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            scope_stmt(
                &f.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        Statement::ForInStatement(f) => {
            scope_for_left(
                &f.left,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &f.right,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_stmt(
                &f.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        Statement::ForOfStatement(f) => {
            scope_for_left(
                &f.left,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &f.right,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_stmt(
                &f.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        Statement::WhileStatement(w) => {
            scope_expr(
                &w.test,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_stmt(
                &w.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        Statement::DoWhileStatement(dw) => {
            scope_stmt(
                &dw.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &dw.test,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Statement::SwitchStatement(s) => {
            scope_expr(
                &s.discriminant,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            for c in &s.cases {
                if let Some(t) = &c.test {
                    scope_expr(
                        t,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        offsets,
                        file_path,
                        out,
                    );
                }
                for st in &c.consequent {
                    scope_stmt(
                        st,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        false,
                        offsets,
                        file_path,
                        out,
                    );
                }
            }
        }
        Statement::LabeledStatement(l) => {
            scope_stmt(
                &l.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        Statement::ThrowStatement(t) => {
            scope_expr(
                &t.argument,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Statement::WithStatement(w) => {
            scope_expr(
                &w.object,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_stmt(
                &w.body,
                components,
                in_component,
                in_export,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        Statement::ClassDeclaration(c) => {
            scope_class(
                c,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                scope_stmt(
                    s,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    false,
                    offsets,
                    file_path,
                    out,
                );
            }
            if let Some(h) = &t.handler {
                for s in &h.body.body {
                    scope_stmt(
                        s,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        false,
                        offsets,
                        file_path,
                        out,
                    );
                }
            }
            if let Some(fin) = &t.finalizer {
                for s in &fin.body {
                    scope_stmt(
                        s,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        false,
                        offsets,
                        file_path,
                        out,
                    );
                }
            }
        }
        _ => {}
    }
}

fn walk_declaration(
    decl: &Declaration,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    body_top: bool,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    scope_declaration(
        decl,
        components,
        in_component,
        in_export,
        func_depth,
        body_top,
        offsets,
        file_path,
        out,
    );
}

fn scope_declaration(
    decl: &Declaration,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    body_top: bool,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match decl {
        Declaration::FunctionDeclaration(f) => {
            let is_comp = f.id.as_ref().is_some_and(|id| components.contains(id.name.as_str()));
            scope_function(
                f,
                components,
                in_component || is_comp,
                func_depth,
                is_comp,
                offsets,
                file_path,
                out,
            );
        }
        Declaration::VariableDeclaration(vd) => {
            for d in &vd.declarations {
                scope_declarator(
                    d,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    body_top,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        _ => {}
    }
}

fn scope_declarator(
    decl: &VariableDeclarator,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    body_top: bool,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    let Some(init) = &decl.init else { return };
    let owned_name = binding_ident_name(&decl.id).unwrap_or_default();
    // Destructuring at the top of a component body never reaches codegen:
    // only plain-identifier consts become reactive lambdas (and the
    // two-name morphState pair becomes a signal). Anything else drops its
    // bindings — flag it where it stands. Loop heads and nested blocks
    // translate wholesale and stay quiet.
    if body_top && in_component {
        // State calls have their own pair-shape rule; everything else
        // with a pattern never reaches codegen.
        let mut is_state_call = false;
        if let Expression::CallExpression(call) = init {
            if let Expression::Identifier(id) = &call.callee {
                let n = id.name.as_str();
                is_state_call = n == "morphState" || n == "morphShared" || n == "morphEvent";
            }
        }
        if is_state_call {
            // State calls must be `[value, setValue]` pairs: whole-tuple,
            // single, spread and object shapes all break the wiring.
            if !is_state_pair(&decl.id) {
                scope_error(
                    "mx-state-pattern",
                    "`morphState` must be destructured as `const [value, setValue] = morphState(...)`".into(),
                    "Use array destructuring with exactly two bindings".into(),
                    file_path,
                    offsets,
                    decl.span.start,
                    out,
                );
            }
        } else if binding_ident_name(&decl.id).is_none() {
            scope_error(
                "mx-js-syntax",
                "destructuring in component bodies cannot be translated to C++".into(),
                "Declare each binding with its own const (only `const [value, setValue] = morphState(...)` pairs lower)".into(),
                file_path,
                offsets,
                decl.span.start,
                out,
            );
        } else if let Some((what, hint)) = unlowerable_const_init(init) {
            scope_error(
                "mx-js-syntax",
                format!("{what} cannot be translated to C++"),
                hint.into(),
                file_path,
                offsets,
                decl.span.start,
                out,
            );
        }
    }
    // Destructuring defaults can hide calls (`const { a = morphState(0) } = x`)
    // so the pattern is walked for value positions as well as bindings.
    scope_binding_pattern(
        &decl.id,
        components,
        in_component,
        in_export,
        func_depth,
        offsets,
        file_path,
        out,
    );
    // A component-arrow declarator gets `in_component=true` for its body
    // (morphState is legal there). Other initializers are plain expressions.
    if let Expression::ArrowFunctionExpression(a) = init {
        let is_arrow_comp = !owned_name.is_empty() && components.contains(owned_name.as_str());
        scope_arrow(
            a,
            components,
            in_component || is_arrow_comp,
            func_depth,
            is_arrow_comp,
            offsets,
            file_path,
            out,
        );
    } else if let Expression::FunctionExpression(f) = init {
        // `const App = function () { … }` is the same component as the
        // arrow spelling, so its body allows framework state calls.
        let is_fn_comp = !owned_name.is_empty() && components.contains(owned_name.as_str());
        scope_function(
            f,
            components,
            in_component || is_fn_comp,
            func_depth,
            is_fn_comp,
            offsets,
            file_path,
            out,
        );
    } else {
        scope_expr(init, components, in_component, in_export, func_depth, offsets, file_path, out);
    }
}

fn scope_function(
    f: &Function,
    components: &HashSet<&str>,
    in_component: bool,
    func_depth: usize,
    body_top: bool,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    if let Some(body) = &f.body {
        for s in &body.statements {
            if in_component && body_top {
                check_nested_effects(s, offsets, file_path, out);
            }
            scope_stmt(
                s,
                components,
                in_component,
                false,
                func_depth + 1,
                body_top,
                offsets,
                file_path,
                out,
            );
        }
    }
}

fn scope_arrow(
    a: &ArrowFunctionExpression,
    components: &HashSet<&str>,
    in_component: bool,
    func_depth: usize,
    body_top: bool,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    if let Some(body) = a.body.as_function_body() {
        for s in &body.statements {
            if in_component && body_top {
                check_nested_effects(s, offsets, file_path, out);
            }
            scope_stmt(
                s,
                components,
                in_component,
                false,
                func_depth + 1,
                body_top,
                offsets,
                file_path,
                out,
            );
        }
    } else if let Some(expr) = a.body.as_expression() {
        if in_component && body_top {
            let mut finder = NestedEffectCalls::default();
            finder.visit_expression(expr);
            report_nested_effects(finder.spans, offsets, file_path, out);
        }
        scope_expr(expr, components, in_component, false, func_depth + 1, offsets, file_path, out);
    }
}

fn scope_for_init(
    init: &ForStatementInit,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match init {
        ForStatementInit::VariableDeclaration(vd) => {
            for d in &vd.declarations {
                scope_declarator(
                    d,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    false,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        _ => {
            if let Some(e) = init.as_expression() {
                scope_expr(
                    e,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
}

fn scope_for_left(
    left: &ForStatementLeft,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match left {
        ForStatementLeft::VariableDeclaration(vd) => {
            for d in &vd.declarations {
                scope_declarator(
                    d,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    false,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        _ => {
            if let Some(at) = left.as_assignment_target() {
                scope_assignment_target(
                    at,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
}

fn scope_assignment_target(
    at: &AssignmentTarget,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    // Simple targets: identifiers, member expressions, or TS casts.
    match at {
        AssignmentTarget::AssignmentTargetIdentifier(id) => {
            let _ = id;
        }
        AssignmentTarget::TSAsExpression(e) => {
            scope_expr(
                &e.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        AssignmentTarget::TSNonNullExpression(e) => {
            scope_expr(
                &e.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        _ => {
            if let Some(me) = at.as_member_expression() {
                scope_member(
                    me,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
}

fn scope_simple_assignment_target(
    at: &SimpleAssignmentTarget,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match at {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(id) => {
            let _ = id;
        }
        SimpleAssignmentTarget::TSAsExpression(e) => {
            scope_expr(
                &e.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        SimpleAssignmentTarget::TSNonNullExpression(e) => {
            scope_expr(
                &e.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        _ => {
            if let Some(me) = at.as_member_expression() {
                scope_member(
                    me,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
}

fn scope_member(
    me: &MemberExpression,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match me {
        MemberExpression::StaticMemberExpression(s) => {
            scope_expr(
                &s.object,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        MemberExpression::ComputedMemberExpression(c) => {
            scope_expr(
                &c.object,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &c.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        MemberExpression::PrivateFieldExpression(p) => {
            scope_expr(
                &p.object,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
    }
}

fn scope_chain_element(
    el: &ChainElement,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match el {
        ChainElement::CallExpression(call) => {
            check_scope_call(call, in_component, in_export, func_depth, offsets, file_path, out);
            for arg in &call.arguments {
                scope_call_arg(
                    arg,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            scope_expr(
                &call.callee,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        ChainElement::TSNonNullExpression(e) => {
            // Recurse generally: `(morphState(0))!` is a call, not a member.
            scope_expr(
                &e.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        _ => {
            if let Some(me) = el.as_member_expression() {
                scope_member(
                    me,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
}

/// Walk a binding pattern's *value* positions: destructuring defaults
/// (`const { a = morphState(0) } = x`) and computed keys (`const { [k]: v } = x`)
/// can hide framework calls. Plain identifiers bind, they are not visited.
fn scope_binding_pattern(
    pat: &BindingPattern,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match pat {
        BindingPattern::BindingIdentifier(_) => {}
        BindingPattern::ObjectPattern(o) => {
            for p in &o.properties {
                if let Some(e) = p.key.as_expression() {
                    scope_expr(
                        e,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        offsets,
                        file_path,
                        out,
                    );
                }
                scope_binding_pattern(
                    &p.value,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            if let Some(r) = &o.rest {
                scope_binding_pattern(
                    &r.argument,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        BindingPattern::ArrayPattern(a) => {
            for p in a.elements.iter().flatten() {
                scope_binding_pattern(
                    p,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            if let Some(r) = &a.rest {
                scope_binding_pattern(
                    &r.argument,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        BindingPattern::AssignmentPattern(a) => {
            scope_binding_pattern(
                &a.left,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &a.right,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
    }
}

/// Walk one call argument, including spread (`f(...morphShared(0))`).
/// `as_expression` skips spread elements, so they need their own arm.
fn scope_call_arg(
    arg: &Argument,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match arg {
        Argument::SpreadElement(s) => {
            scope_expr(
                &s.argument,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        _ => {
            if let Some(e) = arg.as_expression() {
                scope_expr(
                    e,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
}

/// Walk a class (declaration or expression): heritage, computed keys,
/// decorators, method bodies, property initializers and static blocks.
/// Methods count as function boundaries, like plain nested functions.
fn scope_decorator(
    d: &Decorator,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    scope_expr(
        &d.expression,
        components,
        in_component,
        in_export,
        func_depth,
        offsets,
        file_path,
        out,
    );
}

/// Walk a class (declaration or expression): heritage, computed keys,
/// decorators, method bodies, property initializers and static blocks.
/// Methods count as function boundaries, like plain nested functions.
fn scope_class(
    c: &Class,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    for d in &c.decorators {
        scope_decorator(
            d,
            components,
            in_component,
            in_export,
            func_depth,
            offsets,
            file_path,
            out,
        );
    }
    if let Some(h) = &c.heritage {
        scope_expr(
            &h.expression,
            components,
            in_component,
            in_export,
            func_depth,
            offsets,
            file_path,
            out,
        );
    }
    for el in &c.body.body {
        scope_class_element(
            el,
            components,
            in_component,
            in_export,
            func_depth,
            offsets,
            file_path,
            out,
        );
    }
}

/// One class-body element: method bodies count as function boundaries,
/// property initializers and static blocks are value positions.
fn scope_class_element(
    el: &ClassElement,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match el {
        ClassElement::MethodDefinition(m) => {
            for d in &m.decorators {
                scope_decorator(
                    d,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            if let Some(e) = m.key.as_expression() {
                scope_expr(
                    e,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            scope_function(
                &m.value,
                components,
                in_component,
                func_depth,
                false,
                offsets,
                file_path,
                out,
            );
        }
        ClassElement::PropertyDefinition(p) => {
            scope_class_field(
                &p.decorators,
                &p.key,
                p.value.as_ref(),
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        ClassElement::StaticBlock(b) => {
            for s in &b.body {
                scope_stmt(
                    s,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    false,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        // Index signatures are type-only (`[k: string]: T`) — no value positions.
        ClassElement::TSIndexSignature(_) => {}
        ClassElement::AccessorProperty(a) => {
            scope_class_field(
                &a.decorators,
                &a.key,
                a.value.as_ref(),
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
    }
}

/// Shared field shape behind property and accessor definitions:
/// decorators, a possibly-computed key and an optional initializer.
fn scope_class_field(
    decorators: &[Decorator],
    key: &PropertyKey,
    value: Option<&Expression>,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    for d in decorators {
        scope_decorator(
            d,
            components,
            in_component,
            in_export,
            func_depth,
            offsets,
            file_path,
            out,
        );
    }
    if let Some(e) = key.as_expression() {
        scope_expr(e, components, in_component, in_export, func_depth, offsets, file_path, out);
    }
    if let Some(v) = value {
        scope_expr(v, components, in_component, in_export, func_depth, offsets, file_path, out);
    }
}

/// Walk JSX for framework calls in attribute values and expression
/// containers (`onClick={() => morphState(0)}`). Tag/prop *validation*
/// lives in `lint_jsx`; this only tracks call placement scope.
fn scope_jsx_expression(
    expr: &JSXExpression,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    if let Some(e) = expr.as_expression() {
        scope_expr(e, components, in_component, in_export, func_depth, offsets, file_path, out);
    }
}

fn scope_jsx_child(
    child: &JSXChild,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match child {
        JSXChild::Element(el) => {
            scope_jsx_element(
                el,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        JSXChild::Fragment(f) => {
            for c in &f.children {
                scope_jsx_child(
                    c,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        JSXChild::ExpressionContainer(c) => {
            scope_jsx_expression(
                &c.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        JSXChild::Spread(s) => {
            scope_expr(
                &s.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        JSXChild::Text(_) => {}
    }
}

fn scope_jsx_element(
    el: &JSXElement,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    for attr in &el.opening_element.attributes {
        match attr {
            JSXAttributeItem::Attribute(a) => {
                if let Some(v) = &a.value {
                    match v {
                        JSXAttributeValue::ExpressionContainer(c) => {
                            scope_jsx_expression(
                                &c.expression,
                                components,
                                in_component,
                                in_export,
                                func_depth,
                                offsets,
                                file_path,
                                out,
                            );
                        }
                        JSXAttributeValue::Element(e) => {
                            scope_jsx_element(
                                e,
                                components,
                                in_component,
                                in_export,
                                func_depth,
                                offsets,
                                file_path,
                                out,
                            );
                        }
                        JSXAttributeValue::Fragment(f) => {
                            for c in &f.children {
                                scope_jsx_child(
                                    c,
                                    components,
                                    in_component,
                                    in_export,
                                    func_depth,
                                    offsets,
                                    file_path,
                                    out,
                                );
                            }
                        }
                        JSXAttributeValue::StringLiteral(_) => {}
                    }
                }
            }
            JSXAttributeItem::SpreadAttribute(s) => {
                scope_expr(
                    &s.argument,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
    }
    for child in &el.children {
        scope_jsx_child(
            child,
            components,
            in_component,
            in_export,
            func_depth,
            offsets,
            file_path,
            out,
        );
    }
}

fn scope_expr(
    expr: &Expression,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    match expr {
        Expression::CallExpression(call) => {
            check_scope_call(call, in_component, in_export, func_depth, offsets, file_path, out);
            for arg in &call.arguments {
                scope_call_arg(
                    arg,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
            scope_expr(
                &call.callee,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::ArrowFunctionExpression(a) => {
            scope_arrow(a, components, in_component, func_depth, false, offsets, file_path, out);
        }
        Expression::FunctionExpression(f) => {
            scope_function(f, components, in_component, func_depth, false, offsets, file_path, out);
        }
        Expression::ParenthesizedExpression(pe) => {
            scope_expr(
                &pe.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::ConditionalExpression(c) => {
            scope_expr(
                &c.test,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &c.consequent,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &c.alternate,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::LogicalExpression(l) => {
            scope_expr(
                &l.left,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &l.right,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::BinaryExpression(b) => {
            scope_expr(
                &b.left,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &b.right,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::AssignmentExpression(ae) => {
            scope_assignment_target(
                &ae.left,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            scope_expr(
                &ae.right,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::UnaryExpression(u) => {
            scope_expr(
                &u.argument,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::UpdateExpression(up) => {
            scope_simple_assignment_target(
                &up.argument,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::TemplateLiteral(t) => {
            for e in &t.expressions {
                scope_expr(
                    e,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::TaggedTemplateExpression(tt) => {
            scope_expr(
                &tt.tag,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            for e in &tt.quasi.expressions {
                scope_expr(
                    e,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::ArrayExpression(arr) => {
            for e in &arr.elements {
                if let Some(e) = e.as_expression() {
                    scope_expr(
                        e,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        offsets,
                        file_path,
                        out,
                    );
                }
            }
        }
        Expression::ObjectExpression(obj) => {
            for prop in &obj.properties {
                if let ObjectPropertyKind::ObjectProperty(p) = prop {
                    scope_expr(
                        &p.value,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        offsets,
                        file_path,
                        out,
                    );
                } else if let ObjectPropertyKind::SpreadProperty(sp) = prop {
                    scope_expr(
                        &sp.argument,
                        components,
                        in_component,
                        in_export,
                        func_depth,
                        offsets,
                        file_path,
                        out,
                    );
                }
            }
        }
        Expression::NewExpression(ne) => {
            // The callee is a call position too (`new (morphState(0))()`).
            scope_expr(
                &ne.callee,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            for arg in &ne.arguments {
                scope_call_arg(
                    arg,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::SequenceExpression(seq) => {
            for e in &seq.expressions {
                scope_expr(
                    e,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::ChainExpression(ch) => {
            scope_chain_element(
                &ch.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::AwaitExpression(a) => {
            scope_expr(
                &a.argument,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::YieldExpression(y) => {
            if let Some(a) = &y.argument {
                scope_expr(
                    a,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::ImportExpression(ie) => {
            scope_expr(
                &ie.source,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
            if let Some(o) = &ie.options {
                scope_expr(
                    o,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::JSXElement(el) => {
            scope_jsx_element(
                el,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::JSXFragment(f) => {
            for c in &f.children {
                scope_jsx_child(
                    c,
                    components,
                    in_component,
                    in_export,
                    func_depth,
                    offsets,
                    file_path,
                    out,
                );
            }
        }
        Expression::ClassExpression(c) => {
            scope_class(
                c,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::TSAsExpression(a) => {
            scope_expr(
                &a.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        Expression::TSNonNullExpression(nn) => {
            scope_expr(
                &nn.expression,
                components,
                in_component,
                in_export,
                func_depth,
                offsets,
                file_path,
                out,
            );
        }
        _ => {
            if let Some(me) = expr.as_member_expression() {
                match me {
                    MemberExpression::StaticMemberExpression(s) => {
                        scope_expr(
                            &s.object,
                            components,
                            in_component,
                            in_export,
                            func_depth,
                            offsets,
                            file_path,
                            out,
                        );
                    }
                    MemberExpression::ComputedMemberExpression(c) => {
                        scope_expr(
                            &c.object,
                            components,
                            in_component,
                            in_export,
                            func_depth,
                            offsets,
                            file_path,
                            out,
                        );
                        scope_expr(
                            &c.expression,
                            components,
                            in_component,
                            in_export,
                            func_depth,
                            offsets,
                            file_path,
                            out,
                        );
                    }
                    MemberExpression::PrivateFieldExpression(p) => {
                        scope_expr(
                            &p.object,
                            components,
                            in_component,
                            in_export,
                            func_depth,
                            offsets,
                            file_path,
                            out,
                        );
                    }
                }
            }
        }
    }
}

fn check_scope_call(
    call: &CallExpression,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    let Expression::Identifier(id) = &call.callee else { return };
    let name = id.name.as_str();
    let start = call.span.start;
    match name {
        "morphState" => {
            if !in_component {
                scope_error(
                    "mx-state-scope",
                    "`morphState` may only be called inside a component body"
                        .into(),
                    "Move the `const [value, setValue] = morphState(...)` declaration into a component".into(),
                    file_path, offsets, start, out,
                );
            }
            check_state_init(call, file_path, offsets, out);
        }
        "morphShared" => {
            if call.arguments.len() != 1 {
                scope_error(
                    "mx-api-removed",
                    "`morphShared` now takes a single initial value (string keys removed)".into(),
                    "Use `export const [value, setValue] = morphShared<T>(initialValue)` at module scope".into(),
                    file_path, offsets, start, out,
                );
            } else if in_component || func_depth > 0 {
                scope_error(
                    "mx-shared-scope",
                    "`morphShared` may only appear at module scope".into(),
                    "Move `export const [value, setValue] = morphShared(...)` to the top of the file".into(),
                    file_path, offsets, start, out,
                );
            } else if !in_export {
                scope_error(
                    "mx-shared-scope",
                    "`morphShared` must be exported to be shared across modules".into(),
                    "Add `export` to the `const [value, setValue] = morphShared(...)` binding"
                        .into(),
                    file_path,
                    offsets,
                    start,
                    out,
                );
            } else {
                check_state_init(call, file_path, offsets, out);
            }
        }
        "morphEvent" => {
            if !call.arguments.is_empty() {
                scope_error(
                    "mx-api-removed",
                    "`morphEvent` no longer takes arguments (string keys removed)".into(),
                    "Use `export const ev = morphEvent<T>()` at module scope".into(),
                    file_path,
                    offsets,
                    start,
                    out,
                );
            } else if in_component || func_depth > 0 {
                scope_error(
                    "mx-event-scope",
                    "`morphEvent` may only appear at module scope".into(),
                    "Move `export const ev = morphEvent(...)` to the top of the file".into(),
                    file_path,
                    offsets,
                    start,
                    out,
                );
            } else if !in_export {
                scope_error(
                    "mx-event-scope",
                    "`morphEvent` must be exported to be shared across modules".into(),
                    "Add `export` to the `const ev = morphEvent<T>()` binding".into(),
                    file_path,
                    offsets,
                    start,
                    out,
                );
            }
        }
        "morphOn" => {
            scope_error(
                "mx-api-removed",
                "`morphOn` was removed — use `eventName.on(handler)` on an exported event".into(),
                "Substitute `morphOn(key, fn)` with `ev.on(fn)` on a `morphEvent` binding".into(),
                file_path,
                offsets,
                start,
                out,
            );
        }
        "morphEmit" => {
            scope_error(
                "mx-api-removed",
                "`morphEmit` was removed — use `eventName.emit(payload)` on an exported event".into(),
                "Substitute `morphEmit(key, payload)` with `ev.emit(payload)` on a `morphEvent` binding".into(),
                file_path, offsets, start, out,
            );
        }
        _ => {}
    }
}

/// Cross-file component validation over a resolved module graph.
///
/// Mirrors the IR builder's `resolve_component` + `bind_props` so `morph
/// check` surfaces unknown components and prop mismatches with file/line
/// before an expensive native compile.
pub fn lint_graph(graph: &crate::resolve::ModuleGraph) -> Vec<LintError> {
    let mut out = Vec::new();
    // mx-naming: lowercase `[a-z0-9_]` segments + no normalized collisions.
    // Namespaces resolve against the graph's source root so route graphs
    // (rebased to `src/`) accept shared `src/components/…` imports.
    {
        let mut seen: HashMap<String, String> = HashMap::new();
        for mod_path in graph.all_paths() {
            let file = mod_path.display().to_string();
            match crate::resolve::module_ns_path_from_base(graph.ns_base(), mod_path) {
                Err(msg) => out.push(LintError {
                    severity: "error".into(),
                    code: "mx-naming".into(),
                    message: msg,
                    suggestion: None,
                    file_path: file,
                    line: 1,
                    col: 1,
                }),
                Ok(ns) => {
                    if let Some(first) = seen.get(&ns) {
                        out.push(LintError {
                            severity: "error".into(),
                            code: "mx-naming".into(),
                            message: format!(
                                "mx-naming: `{file}` and `{first}` both normalize to `{ns}` — rename one. Learn more: https://morph.levizr.com/docs/errors/mx-naming"
                            ),
                            suggestion: None,
                            file_path: file,
                            line: 1,
                            col: 1,
                        });
                    } else {
                        seen.insert(ns, file);
                    }
                }
            }
        }
    }
    for mod_path in graph.all_paths() {
        let Some(resolved) = graph.get(mod_path) else {
            continue;
        };
        let file = mod_path.display().to_string();
        for comp in &resolved.source.components {
            let mut uses = Vec::new();
            collect_component_uses(&comp.jsx, &mut uses);
            for u in uses {
                match resolve_in_graph(graph, mod_path, &u.tag) {
                    Some(decl) => check_call_props(&decl, &u, &file, &mut out),
                    None => out.push(LintError {
                        severity: "error".into(),
                        code: "mx-component-unknown".into(),
                        message: format!("Unknown component `<{}>`", u.tag),
                        suggestion: Some(
                            "Check the import path and that the target exports this component"
                                .into(),
                        ),
                        file_path: file.clone(),
                        line: u.line,
                        col: u.col,
                    }),
                }
            }
        }
    }
    // Deduplicate by code+file+line+col+message.
    let mut seen = HashSet::new();
    out.retain(|e| {
        seen.insert((e.code.clone(), e.file_path.clone(), e.line, e.col, e.message.clone()))
    });
    out
}

/// Resolve a component name the way the IR builder does: local first,
/// then default/named `.mx` imports. Returns the declared component.
fn resolve_in_graph(
    graph: &crate::resolve::ModuleGraph,
    module: &std::path::Path,
    local: &str,
) -> Option<crate::ast_types::MxComponent> {
    let src = graph.modules.get(module)?;
    if let Some(c) = src.source.components.iter().find(|c| c.name == local) {
        return Some(c.clone());
    }
    for imp in &src.source.imports {
        if !imp.kind.is_module_source() {
            continue;
        }
        let MxImportKind::Component { path: raw, default, specifiers, .. } = &imp.kind else {
            continue;
        };
        let is_default = default.as_deref() == Some(local);
        let is_named = specifiers.iter().any(|(l, _)| l == local);
        if !is_default && !is_named {
            continue;
        }
        let target = src.module_imports.iter().find(|(p, _)| p == raw).map(|(_, t)| t)?;
        if is_default {
            if let Some((_, c)) = crate::resolve_component_export(graph, target, "default") {
                return Some(c);
            }
            return None;
        }
        let imported = specifiers.iter().find(|(l, _)| l == local).map(|(_, i)| i.as_str());
        if let Some(c) = imported.and_then(|i| crate::resolve_component_export(graph, target, i)) {
            return Some(c.1);
        }
        return None;
    }
    None
}

// Deep link to the per-code error doc.
// Every `mx-*` diagnostic printed by `morph check` / `morph build` points
// here so users get the full explanation and fix, not just one line.
pub fn docs_url(code: &str) -> String {
    format!("https://morph.levizr.com/docs/errors/{code}")
}

// Helper for tag suggestion (exposed to crate)
pub fn suggest_tag(input: &str) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    for tag in SUPPORTED_TAGS {
        // Jaro-Winkler like every other hint: transpositions score high.
        let sim = strsim::jaro_winkler(input, tag);
        if sim > 0.7 {
            if let Some((_, bs)) = &best {
                if sim > *bs {
                    best = Some((tag.to_string(), sim));
                }
            } else {
                best = Some((tag.to_string(), sim));
            }
        }
    }
    best.map(|(s, _)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(errors: &[LintError]) -> Vec<&str> {
        errors.iter().map(|e| e.code.as_str()).collect()
    }

    #[test]
    fn bare_morph_builtin_needs_its_import() {
        let content = "export default function App() {\n  const [count, setCount] = morphState(0)\n  return (<div>{count}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-no-morph-import"), "{errs:?}");
        let err = errs.iter().find(|e| e.code == "mx-no-morph-import").unwrap();
        assert!(
            err.suggestion.as_deref().unwrap().contains("import { morphState } from 'morph'"),
            "{err:?}"
        );
    }

    #[test]
    fn imported_morph_builtin_is_clean() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [count, setCount] = morphState(0)\n  return (<div>{count}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-no-morph-import"), "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
    }

    #[test]
    fn undefined_name_errors_with_suggestion() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [count, setCount] = morphState(0)\n  return (<div>{cont}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-undefined"), "{errs:?}");
        let err = errs.iter().find(|e| e.code == "mx-undefined").unwrap();
        assert!(err.suggestion.as_deref().unwrap().contains("count"), "{err:?}");
    }

    #[test]
    fn natives_and_intrinsics_are_clean() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [count, setCount] = morphState(0)\n  morphEffect(() => { console.log(count) })\n  return (<div><span>{count}</span></div>)\n}\n";
        // morphEffect bare → exactly one mx-no-morph-import; console/<div>/<span> clean.
        let errs = check(content, "App.mx");
        assert_eq!(errs.iter().filter(|e| e.code == "mx-no-morph-import").count(), 1, "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
    }

    #[test]
    fn top_level_effect_is_clean() {
        let content = "import { morphState, morphEffect } from 'morph'\nexport default function App() {\n  const [count, setCount] = morphState(0)\n  morphEffect(() => { console.log(count) })\n  return (<div><span>{count}</span></div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-effect-nested"), "{errs:?}");
    }

    #[test]
    fn effect_inside_branch_is_flagged() {
        let content = "import { morphState, morphEffect } from 'morph'\nexport default function App() {\n  const [count, setCount] = morphState(0)\n  if (count > 0) {\n    morphEffect(() => { console.log(count) })\n  }\n  return (<div><span>{count}</span></div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-effect-nested"), "{errs:?}");
        assert_eq!(errs.iter().filter(|e| e.code == "mx-effect-nested").count(), 1, "{errs:?}");
    }

    #[test]
    fn effect_inside_loop_and_nested_callback_are_flagged() {
        let content = "import { morphState, morphEffect } from 'morph'\nexport default function App() {\n  const [n, setN] = morphState(3)\n  for (const it of [1, 2]) {\n    morphEffect(() => { console.log(it) })\n  }\n  morphEffect(() => {\n    morphEffect(() => { console.log(n) })\n  })\n  return (<div><span>{n}</span></div>)\n}\n";
        let errs = check(content, "App.mx");
        assert_eq!(errs.iter().filter(|e| e.code == "mx-effect-nested").count(), 2, "{errs:?}");
    }

    #[test]
    fn effect_inside_inner_function_is_flagged() {
        let content = "import { morphState, morphEffect } from 'morph'\nexport default function App() {\n  const [n, setN] = morphState(3)\n  function attach() {\n    morphEffect(() => { console.log(n) })\n  }\n  return (<div><span>{n}</span></div>)\n}\n";
        let errs = check(content, "App.mx");
        assert_eq!(errs.iter().filter(|e| e.code == "mx-effect-nested").count(), 1, "{errs:?}");
    }

    #[test]
    fn named_default_component_state_is_accepted() {
        let content = "export default function App() {\n  const [count, setCount] = morphState(0)\n  return (<div>{count}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-state-scope"), "{errs:?}");
    }

    #[test]
    fn top_level_state_is_rejected() {
        let content = "const [count, setCount] = morphState(0)\nexport default function App() { return (<div/>) }\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-state-scope"), "{errs:?}");
    }

    #[test]
    fn function_expression_component_allows_state() {
        let content = "const App = function() {\n  const [count, setCount] = morphState(0)\n  return (<div>{count}</div>)\n}\nexport default App\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-state-scope"), "{errs:?}");

        let content = "export default (function() {\n  const [count, setCount] = morphState(0)\n  return (<div>{count}</div>)\n})\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-state-scope"), "{errs:?}");
    }

    #[test]
    fn export_list_counts_as_exported_shared_and_event() {
        let content = "import { morphShared, morphEvent } from 'morph'\nconst [theme, setTheme] = morphShared(\"light\")\nconst click = morphEvent()\nexport { theme, setTheme, click }\nexport default function App() { return (<div/>) }\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-shared-scope"), "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-event-scope"), "{errs:?}");

        // Unexported bindings still need their inline export.
        let content = "import { morphShared } from 'morph'\nconst [theme, setTheme] = morphShared(\"light\")\nexport default function App() { return (<div/>) }\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-shared-scope"), "{errs:?}");
    }

    #[test]
    fn unknown_component_is_an_error() {
        let src = crate::parse_mx_str(
            "export default function App() { return (<body><Nope /></body>) }",
            "App.mx",
        )
        .expect("parse");
        let errs = lint(&src, "", "App.mx");
        assert!(codes(&errs).contains(&"mx-component-unknown"));
    }

    #[test]
    fn imported_component_is_accepted() {
        let src = crate::parse_mx_str(
            "import Hero from './Hero.mx'\nexport default function App() { return (<body><Hero /></body>) }",
            "App.mx",
        )
        .expect("parse");
        let errs = lint(&src, "", "App.mx");
        assert!(!codes(&errs).contains(&"mx-component-unknown"));
    }

    #[test]
    fn local_prop_mismatch_is_an_error() {
        let src = crate::parse_mx_str(
            "export function Badge(props: { label: string }) { return (<span>{props.label}</span>) }\nexport default function App() { return (<body><Badge other=\"x\" /></body>) }",
            "App.mx",
        )
        .expect("parse");
        let errs = lint(&src, "", "App.mx");
        assert!(codes(&errs).contains(&"mx-component-prop"));
        assert!(codes(&errs).contains(&"mx-component-required"));
    }

    #[test]
    fn css_load_is_deprecated_warning() {
        let src = crate::parse_mx_str(
            "import { CSS } from 'morph'\nCSS.load('./a.css')\nexport default function App() { return (<div/>) }",
            "App.mx",
        )
        .expect("parse");
        let errs = lint(&src, "", "App.mx");
        assert!(codes(&errs).contains(&"mx-css-load-deprecated"));
    }

    #[test]
    fn graph_resolves_cross_file_props() {
        let root = std::env::temp_dir().join(format!("morph_lint_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("mkdir");
        std::fs::write(
            root.join("src/App.mx"),
            "import Hero from './Hero.mx'\nexport default function App() { return (<body><Hero wrong=\"1\" /></body>) }",
        )
        .expect("write");
        std::fs::write(
            root.join("src/Hero.mx"),
            "export default function Hero(props: { title: string }) { return (<div>{props.title}</div>) }",
        )
        .expect("write");
        let graph = crate::resolve_graph(&root.join("src/App.mx"), &root).expect("graph");
        let errs = lint_graph(&graph);
        assert!(codes(&errs).contains(&"mx-component-prop"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn graph_resolves_aliased_component_imports() {
        let root = std::env::temp_dir().join(format!("morph_lint_alias_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("mkdir");
        // `import { Card as C }` … and `export { Card2 as C2 }` on the other
        // side — both spellings resolve without mx-component-unknown.
        std::fs::write(
            root.join("src/App.mx"),
            "import { Card as C } from './ui.mx'\nimport { C2 } from './ui2.mx'\nexport default function App() { return (<body><C /><C2 /></body>) }",
        )
        .expect("write");
        std::fs::write(
            root.join("src/ui.mx"),
            "export const Card = () => { return (<text>card</text>) }",
        )
        .expect("write");
        std::fs::write(
            root.join("src/ui2.mx"),
            "const Card2 = () => { return (<text>card2</text>) }\nexport { Card2 as C2 }",
        )
        .expect("write");
        let graph = crate::resolve_graph(&root.join("src/App.mx"), &root).expect("graph");
        let errs = lint_graph(&graph);
        assert!(!codes(&errs).contains(&"mx-component-unknown"), "{errs:?}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn graph_resolves_typescript_component_props() {
        let root = std::env::temp_dir().join(format!("morph_lint_ts_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("mkdir");
        std::fs::write(
            root.join("src/App.mx"),
            "import Hero from './Hero.tsx'\nexport default function App() { return (<body><Hero wrong=\"1\" /></body>) }",
        )
        .expect("write");
        std::fs::write(
            root.join("src/Hero.tsx"),
            "export default function Hero(props: { title: string }) { return (<div>{props.title}</div>) }",
        )
        .expect("write");
        let graph = crate::resolve_graph(&root.join("src/App.mx"), &root).expect("graph");
        let errs = lint_graph(&graph);
        assert!(codes(&errs).contains(&"mx-component-prop"));
        let _ = std::fs::remove_dir_all(&root);
    }

    const SCOPE_HDR: &str = "import { morphState } from 'morph'\nexport const windowConfig = { title: \"P\", width: 800, height: 600 }\n";

    fn scope_codes(body: &str) -> Vec<String> {
        let content = format!(
            "{SCOPE_HDR}{body}\nexport default function App() {{ return (<div>x</div>) }}\n"
        );
        check(&content, "App.mx").iter().map(|e| e.code.clone()).collect()
    }

    #[test]
    fn state_scope_reaches_switch_class_throw_await() {
        for body in [
            "switch (1) { case 1: { const [v] = morphState(0); break; } }",
            "class H { m() { return morphState(0); } }",
            "if (true) throw morphState(0);",
            "async function boot() { const v = await morphState(0); return v; }",
            "const { a = morphState(0) } = {};",
            "const v = new (morphState(0))();",
            "const v = [morphState(0)].length;",
            "class H { static x = morphState(0); }",
            "class H extends morphState(0) {}",
            "top: { const [v] = morphState(0); break top; }",
        ] {
            let found = scope_codes(body);
            assert!(found.contains(&"mx-state-scope".to_string()), "{body:?} got {found:?}");
        }
    }

    #[test]
    fn unsupported_builtin_members_report_js_member() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const v = Math.random();\n  const j = JSON.parse2({});\n  return (<div>{v}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert_eq!(codes(&errs).iter().filter(|c| **c == "mx-js-member").count(), 2, "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
        let math = errs.iter().find(|e| e.message.contains("Math.random")).unwrap();
        assert!(math.message.contains("no native counterpart"), "{math:?}");
    }

    #[test]
    fn promise_static_helpers_report_js_member() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const p = Promise.resolve(1);\n  return (<div>x</div>)\n}\n";
        let errs = check(content, "App.mx");
        let err = errs
            .iter()
            .find(|e| e.code == "mx-js-member")
            .unwrap_or_else(|| panic!("Promise.resolve fires mx-js-member, got {errs:?}"));
        assert!(err.message.contains("Promise.resolve"), "{err:?}");
        assert!(!codes(&errs).contains(&"mx-js-syntax"), "{errs:?}");
    }

    #[test]
    fn supported_builtins_stay_quiet() {
        // Lowered builtins (Math.min, JSON.parse, Object.keys, conversions)
        // must not trip mx-js-member, including nested in member chains.
        let content = "import { morphState } from 'morph'\nexport const windowConfig = { title: \"D\", width: 800, height: 600 }\nexport default function App() {\n  const o = JSON.parse('{\"a\":1}')\n  const t = Object.keys(o).join()\n  const m = Math.max(1, 9, 3)\n  const n = Number(\"42\")\n  const s = String(m)\n  const p = parseInt(\"42px\")\n  const f = Number.isFinite(m)\n  const b = Array.isArray(o)\n  return (<div>{t}{m}{n}{s}{p}{f}{b}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-member"), "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
    }

    #[test]
    fn shadowed_builtin_stays_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const Math = { min: (a, b) => a };\n  const v = Math.min(1, 2);\n  return (<div>{v}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-member"), "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
    }

    #[test]
    fn console_table_flags_but_log_is_clean() {
        let bad = "import { morphState } from 'morph'\nexport default function App() {\n  console.table([1]);\n  return (<div>x</div>)\n}\n";
        let errs = check(bad, "App.mx");
        assert!(codes(&errs).contains(&"mx-js-member"), "{errs:?}");
        let good = "import { morphState } from 'morph'\nexport default function App() {\n  console.log(1);\n  return (<div>x</div>)\n}\n";
        let errs = check(good, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-member"), "{errs:?}");
    }

    #[test]
    fn browser_global_in_string_and_comment_stays_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const s = \"see document.body for details\"; // window.title hack\n  return (<div>{s}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-global"), "{errs:?}");
    }

    #[test]
    fn real_browser_global_still_errors_once() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const t = document.title;\n  return (<div>{t}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-js-global"), "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
    }

    #[test]
    fn unemittable_operators_error_with_fixes() {
        let cases = [
            ("a ** 2", "**", "repeated multiplication"),
            ("a >>> 1", ">>>", "non-negative"),
            ("typeof a", "typeof", "statically typed"),
            ("void a", "void", "Drop `void`"),
            ("delete a.x", "delete", "undefined"),
            ("a instanceof B", "instanceof", "discriminant"),
            ("n **= 2", "**=", "x * x"),
            ("n >>>= 1", ">>>=", "non-negative"),
            ("n &&= 2", "&&=", "x && y"),
            ("n ||= 2", "||=", "x || y"),
            ("n ??= 2", "??=", "x ?? y"),
        ];
        for (expr, op, hint) in cases {
            let content = format!(
                "import {{ morphState }} from 'morph'\nconst a = 1; let n = 1; const B = 1; const v = {expr};\nexport default function App() {{ return (<div>{{v}}</div>) }}\n"
            );
            let errs = check(&content, "App.mx");
            let err = errs
                .iter()
                .find(|e| e.code == "mx-js-op")
                .unwrap_or_else(|| panic!("{expr} fires mx-js-op, got {errs:?}"));
            assert_eq!(err.severity, "error");
            assert!(err.message.contains(&format!("Operator `{op}`")), "{err:?}");
            assert!(err.suggestion.as_deref().unwrap().contains(hint), "{err:?}");
        }
    }

    #[test]
    fn emittable_operators_stay_quiet() {
        let content = "import { morphState } from 'morph'\nconst a = {}; let n = 1; n += 1; n >>= 1; n |= 1;\nconst k = \"x\" in a;\nexport default function App() { return (<div>{k}</div>) }\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-op"), "{errs:?}");
    }

    #[test]
    fn untranslatable_syntax_errors_with_fixes() {
        let cases = [
            ("function* gen() { yield 1; }", "generator function", "state machine"),
            ("with (x) { y = 1; }", "`with` statement", "explicitly"),
            ("top: { break top; }", "labeled `break`", "early return"),
            ("const m = import.meta.url;", "`import.meta`", "imports or constants"),
            ("const m = import(\"./x.ts\");", "dynamic `import()`", "statically"),
            ("const o = { ...x, z: 1 };", "object spread", "explicitly"),
            ("namespace N { export const x = 1; }", "`namespace` block", "plain functions"),
        ];
        for (stmt, what, hint) in cases {
            let content = format!(
                "import {{ morphState }} from 'morph'\nconst x = {{}};\n{stmt}\nexport default function App() {{ return (<div>x</div>) }}\n"
            );
            let errs = check(&content, "App.mx");
            let err = errs
                .iter()
                .find(|e| e.code == "mx-js-syntax")
                .unwrap_or_else(|| panic!("{stmt} fires mx-js-syntax, got {errs:?}"));
            assert_eq!(err.severity, "error");
            assert!(err.message.contains(what), "{err:?}");
            assert!(err.suggestion.as_deref().unwrap().contains(hint), "{err:?}");
        }
    }

    #[test]
    fn translatable_syntax_stays_quiet() {
        let content = "import { morphState } from 'morph'\nclass A { greet(): any { return 1; } }\nclass B extends A { m(): any { return super.greet(); } }\nexport default function App() {\n  const arr = [...[1]];\n  for (let i = 0; i < 2; i++) { if (i) { break; } }\n  return (<div>{arr.length}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-syntax"), "{errs:?}");
    }

    #[test]
    fn runtime_only_syntax_rejections_error() {
        let cases = [
            ("const r = /ab+c/;", "regular expression literal"),
            ("enum Color { Red }", "`enum` declaration"),
            ("function f(...xs: number[]) { return xs }", "rest parameter"),
        ];
        for (stmt, what) in cases {
            let content = format!(
                "import {{ morphState }} from 'morph'\nconst x = {{}};\n{stmt}\nexport default function App() {{ return (<div>x</div>) }}\n"
            );
            let errs = check(&content, "App.mx");
            let err = errs
                .iter()
                .find(|e| e.code == "mx-js-syntax")
                .unwrap_or_else(|| panic!("{stmt} fires mx-js-syntax, got {errs:?}"));
            assert!(err.message.contains(what), "{err:?}");
        }
    }

    #[test]
    fn await_inside_async_stays_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  async function load() { await fetch(\"http://127.0.0.1\"); }\n  return (<div onClick={load}>x</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-syntax"), "{errs:?}");
    }

    #[test]
    fn namespace_imports_error() {
        let content = "import { morphState } from 'morph'\nimport * as lib from './lib.ts';\nexport default function App() { return (<div>{lib.token}</div>) }\n";
        let errs = check(content, "App.mx");
        let err = errs
            .iter()
            .find(|e| e.code == "mx-js-syntax")
            .unwrap_or_else(|| panic!("namespace import fires mx-js-syntax, got {errs:?}"));
        assert!(err.message.contains("`import * as` namespace import"), "{err:?}");
    }

    #[test]
    fn cpp_keyword_bindings_error() {
        for name in ["int", "goto", "double", "template", "co_await"] {
            let content = format!(
                "import {{ morphState }} from 'morph'\nexport default function App() {{\n  const {name} = 1;\n  return (<div>{name}</div>)\n}}\n"
            );
            let errs = check(&content, "App.mx");
            let err = errs
                .iter()
                .find(|e| e.code == "mx-js-syntax")
                .unwrap_or_else(|| panic!("{name} fires mx-js-syntax, got {errs:?}"));
            assert!(err.message.contains("C++ keyword as a binding name"), "{err:?}");
        }
    }

    #[test]
    fn cpp_keyword_class_members_error() {
        let content = "import { morphState } from 'morph'\nclass C {\n  int = 1;\n  delete() { return 1 }\n}\nexport default function App() { return (<div>x</div>) }\n";
        let errs = check(content, "App.mx");
        let members: Vec<_> = errs
            .iter()
            .filter(|e| e.code == "mx-js-syntax" && e.message.contains("class member"))
            .collect();
        assert_eq!(members.len(), 2, "{errs:?}");
    }

    #[test]
    fn type_only_positions_stay_quiet() {
        // Type aliases and annotations are erased; rest params and keyword
        // names inside them must not resolve to runtime C++ bindings.
        let content = "import { morphState } from 'morph'\nimport type { double } from './types.ts';\nimport { type float } from './types.ts';\ntype F = (...args: string[]) => void;\ntype long = number;\nconst unused: F | undefined = undefined;\nexport default function App() {\n  const x: long = 1;\n  return (<div>{x}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-syntax"), "{errs:?}");
    }

    #[test]
    fn unknown_component_has_no_duplicate_undefined() {
        let content = "import { morphState } from 'morph'\nexport default function App() { return (<Frob>x</Frob>) }\n";
        let errs = check(content, "App.mx");
        assert!(codes(&errs).contains(&"mx-component-unknown"), "{errs:?}");
        assert!(!codes(&errs).contains(&"mx-undefined"), "{errs:?}");
    }

    #[test]
    fn nested_builtin_calls_stay_quiet() {
        // Nested lowered calls (Object.keys(o).join()) must see the inner
        // root: previously the inner call was never visited and the bare
        // root fell through to mx-js-member.
        let content = "import { morphState } from 'morph'\nexport const windowConfig = { title: \"D\", width: 800, height: 600 }\nexport default function App() {\n  const [a, setA] = morphState([1, 2, 3])\n  const o = JSON.parse('{\"a\":1}')\n  const u = Object.keys(o)\n  const t = Object.keys(o).join()\n  const m = Math.sqrt(2).toFixed(3)\n  console.log(\"arr\", a.join(\"-\"), a.slice(1).join(), a.concat([4]).join(), a.includes(2), a.indexOf(3), a.at(-1))\n  console.log(\"obj\", JSON.stringify(o), Object.keys(o).join())\n  console.log(t, u, m)\n  return (<div>{t}{u}{m}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-member"), "{errs:?}");
    }

    #[test]
    fn nested_unsupported_member_still_errors_once() {
        // The deeper traversal must not double-report: one member error.
        let content = "import { morphState } from 'morph'\nexport const windowConfig = { title: \"D\", width: 800, height: 600 }\nexport default function App() {\n  const t = Math.random().toFixed(2)\n  return (<div>{t}</div>)\n}\n";
        let errs = check(content, "App.mx");
        let member_errs = errs.iter().filter(|e| e.code == "mx-js-member").count();
        assert_eq!(member_errs, 1, "{errs:?}");
    }
    #[test]
    fn transposed_prop_typo_suggests() {
        let content = "import { morphState } from 'morph'\nexport default function App() { return (<div sytle=\"x\">x</div>) }\n";
        let errs = check(content, "App.mx");
        let err = errs.iter().find(|e| e.code == "mx-prop").expect("mx-prop fires");
        assert!(err.suggestion.as_deref().unwrap().contains("style"), "{err:?}");
    }

    #[test]
    fn inline_style_unknown_prop_is_an_error() {
        let content = "import { morphState } from 'morph'\nexport default function App() { return (<div style={{ float: \"left\", color: \"red\" }}>x</div>) }\n";
        let errs = check(content, "App.mx");
        let err = errs.iter().find(|e| e.code == "mx-style-prop").expect("mx-style-prop fires");
        assert_eq!(err.severity, "error");
        assert!(err.message.contains("float"), "{err:?}");
        assert!(err.message.contains("<div>"), "{err:?}");
    }

    #[test]
    fn inline_style_known_props_are_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() { return (<div style={{ backgroundColor: \"red\", width: \"10px\" }}>x</div>) }\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-style-prop"), "{errs:?}");
    }

    #[test]
    fn unimplemented_methods_error_with_receiver() {
        let cases = [
            (
                "const [name] = morphState(\"ada\");\n  const v = name.toFancyCase();",
                "toFancyCase",
                "strings",
            ),
            (
                "const [items] = morphState([1, 2]);\n  const v = items.map(x => x);",
                "map",
                "arrays",
            ),
            ("const v = \"abc\".length();", "length", "strings"),
        ];
        for (body, method, kind) in cases {
            let content = format!(
                "import {{ morphState }} from 'morph'\nexport default function App() {{\n  {body}\n  return (<div>{{v}}</div>)\n}}\n"
            );
            let errs = check(&content, "App.mx");
            let err = errs
                .iter()
                .find(|e| e.code == "mx-js-method")
                .unwrap_or_else(|| panic!(".{method} fires mx-js-method, got {errs:?}"));
            assert_eq!(err.severity, "error");
            assert!(err.message.contains(&format!(".{method}()")), "{err:?}");
            assert!(err.message.contains(kind), "{err:?}");
        }
    }

    #[test]
    fn implemented_methods_and_unknown_receivers_stay_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [name] = morphState(\"ada\");\n  const [items] = morphState([1, 2]);\n  items.push(3);\n  const v = name.toUpperCase();\n  return (<div>{v}{items.length}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-method"), "{errs:?}");
        let unknown = "import { morphState } from 'morph'\nexport default function App({ data }: any) {\n  const v = data.anything.deeper();\n  return (<div>{v}</div>)\n}\n";
        let errs = check(unknown, "App.mx");
        assert!(!codes(&errs).contains(&"mx-js-method"), "{errs:?}");
    }

    #[test]
    fn component_destructuring_errors_but_nested_and_loops_stay_quiet() {
        let bad = "import { morphState } from 'morph'\nexport default function App() {\n  const [a, b] = [1, 2];\n  return (<div>{a}</div>)\n}\n";
        let errs = check(bad, "App.mx");
        let err = errs
            .iter()
            .find(|e| e.code == "mx-js-syntax" && e.message.contains("destructuring"))
            .expect("destructuring fires");
        assert_eq!(err.severity, "error");
        let obj = "import { morphState } from 'morph'\nexport default function App() {\n  const { x } = { x: 1 };\n  return (<div>{x}</div>)\n}\n";
        let errs = check(obj, "App.mx");
        assert!(
            errs.iter().any(|e| e.code == "mx-js-syntax" && e.message.contains("destructuring")),
            "{errs:?}"
        );
        // Nested blocks and loop heads translate wholesale.
        let ok = "import { morphState } from 'morph'\nexport default function App() {\n  const [c, setC] = morphState(0);\n  let out = 0;\n  if (c > 0) {\n    const [a] = [1];\n    out = a;\n  }\n  for (const [k, v] of [[1, 2]]) {\n    out = k;\n  }\n  return (<div>{out}</div>)\n}\n";
        let errs = check(ok, "App.mx");
        assert!(
            !errs.iter().any(|e| e.code == "mx-js-syntax" && e.message.contains("destructuring")),
            "{errs:?}"
        );
    }

    #[test]
    fn state_pair_pattern_stays_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [v, setV] = morphState(0);\n  return (<div>{v}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(
            !errs.iter().any(|e| e.code == "mx-js-syntax" && e.message.contains("destructuring")),
            "{errs:?}"
        );
        assert!(!codes(&errs).contains(&"mx-state-init"), "{errs:?}");
    }

    #[test]
    fn non_literal_state_init_errors() {
        for (init, hint) in [
            ("{ name: \"Ada\" }", "per field"),
            ("base", "literal value"),
            ("base + 1", "plain literal"),
            ("`tpl${1}`", "plain literal"),
            ("null", "plain literal"),
        ] {
            let content = format!(
                "import {{ morphState }} from 'morph'\nconst base = 5;\nexport default function App() {{\n  const [v, setV] = morphState({init});\n  return (<div>{{v}}</div>)\n}}\n"
            );
            let errs = check(&content, "App.mx");
            let err = errs
                .iter()
                .find(|e| e.code == "mx-state-init")
                .unwrap_or_else(|| panic!("{init} fires mx-state-init, got {errs:?}"));
            assert_eq!(err.severity, "error");
            assert!(err.suggestion.as_deref().unwrap().contains(hint), "{err:?}");
        }
    }

    #[test]
    fn state_non_pair_patterns_error() {
        for decl in [
            "const count = morphState(0);",
            "const [v] = morphState(0);",
            "const { v } = morphState(0);",
        ] {
            let content = format!(
                "import {{ morphState }} from 'morph'\nexport default function App() {{\n  {decl}\n  return (<div>x</div>)\n}}\n"
            );
            let errs = check(&content, "App.mx");
            assert!(codes(&errs).contains(&"mx-state-pattern"), "{decl}: {errs:?}");
        }
    }

    #[test]
    fn function_and_object_const_inits_stay_quiet() {
        // Arrow/function consts lower like inner functions; object
        // literals lower to JsObject. Neither is a syntax error.
        for init in ["() => 1", "function() { return 1; }", "{ tag: \"x\" }"] {
            let content = format!(
                "import {{ morphState }} from 'morph'\nexport default function App() {{\n  const h = {init};\n  return (<div>{{h}}</div>)\n}}\n"
            );
            let errs = check(&content, "App.mx");
            assert!(!errs.iter().any(|e| e.code == "mx-js-syntax"), "{init}: {errs:?}");
        }
    }

    #[test]
    fn class_expression_consts_error() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const C = class { m() { return 1; } };\n  return (<div>{C.name}</div>)\n}\n";
        let errs = check(content, "App.mx");
        let err = errs
            .iter()
            .find(|e| e.code == "mx-js-syntax")
            .unwrap_or_else(|| panic!("class expression fires mx-js-syntax, got {errs:?}"));
        assert!(err.message.contains("class expressions assigned to consts"), "{err:?}");
    }

    #[test]
    fn plain_consts_and_declarations_stay_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [c, setC] = morphState(0);\n  const t = 1;\n  function h() { return t; }\n  class K { m() { return 1; } }\n  return (<div>{h()}{t}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(
            !errs.iter().any(|e| e.code == "mx-js-syntax"
                && (e.message.contains("component bodies")
                    || e.message.contains("component consts"))),
            "{errs:?}"
        );
    }

    #[test]
    fn literal_state_inits_stay_quiet() {
        let content = "import { morphState } from 'morph'\nexport default function App() {\n  const [a, sa] = morphState(\"x\");\n  const [b, sb] = morphState(5);\n  const [c, sc] = morphState(true);\n  const [d, sd] = morphState([1, \"a\"]);\n  const [e, se] = morphState(-5);\n  return (<div>{a}</div>)\n}\n";
        let errs = check(content, "App.mx");
        assert!(!codes(&errs).contains(&"mx-state-init"), "{errs:?}");
    }
}
