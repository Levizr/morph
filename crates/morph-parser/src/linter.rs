use std::collections::{HashMap, HashSet};

use crate::ast_types::{JsxNode, LintError, MxImportKind, MxSource};
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;

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
];

/// Morph APIs that must be imported from 'morph' (bare use is an error).
const MORPH_BUILTINS: &[&str] =
    &["morphState", "morphEffect", "morphShared", "morphEvent", "useWindow", "Window", "CSS"];

/// AST positions that are never value references: lowercase JSX tag
/// starts (`<div>`) and type-name ranges (`x: int`, `as Foo`,
/// generics). A real variable sharing an intrinsic's name is still
/// checked — only its own tag-position span is skipped.
struct SkipSpans {
    intrinsics: std::collections::HashSet<u32>,
    type_ranges: Vec<(u32, u32)>,
}

fn collect_skip_spans(program: &Program) -> SkipSpans {
    struct Collector {
        out: SkipSpans,
    }
    impl<'a> Visit<'a> for Collector {
        fn visit_jsx_element_name(&mut self, name: &JSXElementName<'a>) {
            if let JSXElementName::Identifier(id) = name {
                let tag = id.name.as_str();
                if tag.starts_with(|c: char| c.is_ascii_lowercase()) {
                    self.out.intrinsics.insert(id.span.start);
                }
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
        out: SkipSpans { intrinsics: std::collections::HashSet::new(), type_ranges: Vec::new() },
    };
    collector.visit_program(program);
    collector.out
}

/// Lowercase JSX tag positions (`<div>`, `<span>`): intrinsic elements,
/// never identifier references. Collected from the AST so a real
/// variable that merely shares a name is still checked at its own spans.
fn jsx_intrinsic_spans(program: &Program) -> std::collections::HashSet<u32> {
    collect_skip_spans(program).intrinsics
}

/// `mx-undefined` / `mx-no-morph-import`: every referenced name must be
/// declared (locals, params, imports, components) or imported from
/// 'morph' (for Morph APIs) or a supported native global. Uses real
/// scope resolution — shadowing, params, and imports all count.
fn check_unresolved(program: &Program, file_path: &str, offsets: &[usize]) -> Vec<LintError> {
    let mut out = Vec::new();
    let semantic =
        oxc_semantic::SemanticBuilder::new().with_build_nodes(true).build(program).semantic;
    let scoping = semantic.scoping();
    let skip = collect_skip_spans(program);
    let declared: Vec<&str> = scoping.symbol_names().collect();
    let mut seen = std::collections::HashSet::new();
    let mut names: Vec<(&str, Vec<oxc_span::Span>)> = Vec::new();
    for (atom, refs) in scoping.root_unresolved_references() {
        let mut spans = Vec::new();
        for ref_id in refs.iter() {
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
    let allocator = oxc_allocator::Allocator::default();
    let source_type = SourceType::from_path("file.tsx").unwrap();
    let ret = oxc_parser::Parser::new(&allocator, source, source_type).parse();
    let mut errors = Vec::new();

    for diag in &ret.diagnostics {
        let span = diag.labels.first().map_or(Span::new(0, 0), oxc_span::LabeledSpan::span);
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

    // ── JS globals scan (simple substring search with line info) ──
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            continue;
        }
        for g in UNSUPPORTED_GLOBALS {
            if line.contains(&format!("{g}."))
                || line.contains(&format!("{g}["))
                || line.contains(&format!(" {g} "))
            {
                let col = line.find(g).unwrap_or(0) + 1;
                out.push(LintError {
                    severity: "error".into(),
                    code: "mx-js-global".into(),
                    message: format!("`{g}` is not available in native runtime"),
                    suggestion: Some(format!("Use Morph state / C++ instead of browser `{g}`")),
                    file_path: file_path.into(),
                    line: idx + 1,
                    col,
                });
            }
        }
    }

    // Deduplicate by code+line+col
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
        let dist = strsim::levenshtein(input, prop) as f64;
        let max_len = input.len().max(prop.len()) as f64;
        let sim = 1.0 - (dist / max_len);
        if sim > 0.6 {
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
    let allocator = oxc_allocator::Allocator::default();
    let ret =
        oxc_parser::Parser::new(&allocator, content, SourceType::from_path("file.tsx").unwrap())
            .parse();
    if ret.panicked {
        return Vec::new();
    }
    let mut out = Vec::new();
    for stmt in &ret.program.body {
        scope_stmt(stmt, &components, false, false, 0, &offsets, file_path, &mut out);
    }
    out
}

/// Name of a simple identifier binding, if the pattern is a plain identifier.
fn binding_ident_name(bp: &BindingPattern) -> Option<String> {
    if let BindingPattern::BindingIdentifier(id) = bp {
        Some(id.name.to_string())
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

fn scope_stmt(
    stmt: &Statement,
    components: &HashSet<&str>,
    in_component: bool,
    in_export: bool,
    func_depth: usize,
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
                        offsets,
                        file_path,
                        out,
                    );
                }
                ExportDefaultDeclarationKind::ArrowFunctionExpression(a) => {
                    scope_arrow(a, components, decl_in_comp, func_depth, offsets, file_path, out);
                }
                _ => {}
            }
        }
        Statement::ExportDeclaration(d) => walk_declaration(
            &d.declaration,
            components,
            in_component,
            true,
            func_depth,
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
        Statement::TryStatement(t) => {
            for s in &t.block.body {
                scope_stmt(
                    s,
                    components,
                    in_component,
                    in_export,
                    func_depth,
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
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    let Some(init) = &decl.init else { return };
    let owned_name = binding_ident_name(&decl.id).unwrap_or_default();
    // A component-arrow declarator gets `in_component=true` for its body
    // (morphState is legal there). Other initializers are plain expressions.
    if let Expression::ArrowFunctionExpression(a) = init {
        let is_arrow_comp = !owned_name.is_empty() && components.contains(owned_name.as_str());
        scope_arrow(
            a,
            components,
            in_component || is_arrow_comp,
            func_depth,
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
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    if let Some(body) = &f.body {
        for s in &body.statements {
            scope_stmt(s, components, in_component, false, func_depth + 1, offsets, file_path, out);
        }
    }
}

fn scope_arrow(
    a: &ArrowFunctionExpression,
    components: &HashSet<&str>,
    in_component: bool,
    func_depth: usize,
    offsets: &[usize],
    file_path: &str,
    out: &mut Vec<LintError>,
) {
    if let Some(body) = a.body.as_function_body() {
        for s in &body.statements {
            scope_stmt(s, components, in_component, false, func_depth + 1, offsets, file_path, out);
        }
    } else if let Some(expr) = a.body.as_expression() {
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
            if let Some(me) = e.expression.as_member_expression() {
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
            scope_arrow(a, components, in_component, func_depth, offsets, file_path, out);
        }
        Expression::FunctionExpression(f) => {
            scope_function(f, components, in_component, func_depth, offsets, file_path, out);
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
            for arg in &ne.arguments {
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
    {
        let mut seen: HashMap<String, String> = HashMap::new();
        for mod_path in graph.all_paths() {
            let file = mod_path.display().to_string();
            match crate::resolve::module_ns_path(&graph.entry, mod_path) {
                Err(msg) => out.push(LintError {
                    severity: "error".into(),
                    code: "mx-naming".into(),
                    message: msg,
                    suggestion: Some(
                        "Use lowercase letters, digits and underscores in file and directory names"
                            .into(),
                    ),
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
                                "module {file} normalizes to namespace `{ns}`, already claimed by {first}: rename one (mx-naming)"
                            ),
                            suggestion: Some(
                                "Lowercased path segments must be unique across the project".into(),
                            ),
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
        let MxImportKind::Component { path: raw, default, specifiers } = &imp.kind else {
            continue;
        };
        let is_default = default.as_deref() == Some(local);
        let is_named = specifiers.iter().any(|(l, _)| l == local);
        if !is_default && !is_named {
            continue;
        }
        let target = src.module_imports.iter().find(|(p, _)| p == raw).map(|(_, t)| t)?;
        let target_mod = graph.modules.get(target)?;
        if is_default {
            if let Some(c) = target_mod.source.components.iter().find(|c| c.is_default) {
                return Some(c.clone());
            }
            return None;
        }
        if let Some(c) = target_mod.source.components.iter().find(|c| c.name == local && c.exported)
        {
            return Some(c.clone());
        }
        return None;
    }
    None
}

// Helper for tag suggestion (exposed to crate)
pub fn suggest_tag(input: &str) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    for tag in SUPPORTED_TAGS {
        let dist = strsim::levenshtein(input, tag) as f64;
        let max_len = input.len().max(tag.len()) as f64;
        let sim = 1.0 - (dist / max_len);
        if sim > 0.5 {
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
}
