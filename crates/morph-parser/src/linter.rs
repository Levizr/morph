use std::collections::HashSet;

use crate::ast_types::{JsxNode, LintError, MxSource};

fn offset_to_line_col(source: &str, offset: u32) -> (usize, usize) {
    let offset = offset as usize;
    let mut line = 1usize;
    let mut last = 0usize;
    for (i, ch) in source.char_indices() {
        if i >= offset { break; }
        if ch == '\n' { line += 1; last = i + 1; }
    }
    (line, offset.saturating_sub(last) + 1)
}

fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' { offsets.push(i + 1); }
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
    let source_type = oxc_span::SourceType::from_path("file.tsx").unwrap();
    let ret = oxc_parser::Parser::new(&allocator, source, source_type).parse();
    let mut errors = Vec::new();

    for diag in &ret.diagnostics {
        let span = diag.labels.first().map(|l| l.span()).unwrap_or(oxc_span::Span::new(0, 0));
        let (line, col) = offset_to_line_col_fast(&offsets, span.start);
        errors.push(LintError {
            severity: "error".into(),
            code: "parse-error".into(),
            message: diag.message.to_string(),
            suggestion: diag.help.as_ref().map(|h| h.to_string()),
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

    // Walk program to build MxSource for semantic lints (even with parse errors, program is partial)
    let mut walker = crate::js_walker::MxWalker::new(source);
    {
        use oxc_ast_visit::Visit;
        walker.visit_program(&ret.program);
    }
    let mx_source = MxSource {
        filename: file_path.into(),
        imports: walker.imports,
        window_config: walker.window_config,
        components: walker.components,
        state_vars: walker.state_vars,
        effects: walker.effects,
        inner_functions: walker.inner_functions,
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
    "div","span","p","h1","h2","h3","h4","h5","h6","button","input","img","a",
    "ul","ol","li","table","thead","tbody","tr","td","th","form","label",
    "section","header","footer","nav","main","article","aside","body","view",
    "text","morph-window","fragment",
];

static STUB_TAGS: &[&str] = &["select","textarea"];

static GLOBAL_PROPS: &[&str] = &["className","class","id","style","key"];

static EVENT_PROPS: &[&str] = &[
    "onClick","onDoubleClick","onMouseDown","onMouseUp","onMouseEnter","onMouseLeave",
    "onKeyUp","onKeyDown","onChange","onInput","onFocus","onBlur",
];

static TAG_PROPS: &[(&str, &[&str])] = &[
    ("img", &["src","alt","width","height"]),
    ("a", &["href","target"]),
    ("input", &["value","maxLength","minLength","placeholder","disabled","type"]),
    ("morph-window", &["title","width","height","minWidth","maxWidth","minHeight","maxHeight"]),
];

#[inline]
fn is_supported_tag(tag: &str) -> bool {
    SUPPORTED_TAGS.contains(&tag)
}

#[inline]
fn is_component_tag(tag: &str) -> bool {
    tag.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
}

#[inline]
fn is_allowed_prop(tag: &str, prop: &str) -> bool {
    if GLOBAL_PROPS.contains(&prop) { return true; }
    if EVENT_PROPS.contains(&prop) { return true; }
    if prop.starts_with("data-") || prop.starts_with("aria-") { return true; }
    for (t, props) in TAG_PROPS {
        if *t == tag && props.contains(&prop) { return true; }
    }
    false
}

static UNSUPPORTED_GLOBALS: &[&str] = &[
    "document","window","localStorage","sessionStorage","navigator","location",
    "history","screen","alert","prompt","confirm","requestAnimationFrame",
];

pub fn lint(source: &MxSource, content: &str, file_path: &str) -> Vec<LintError> {
    let mut out = Vec::new();

    // ── mx-export: exactly one export default function ──
    if source.components.is_empty() {
        out.push(LintError {
            severity: "error".into(),
            code: "mx-export".into(),
            message: "No component found — expected `export default function App()`".into(),
            suggestion: Some("Add `export default function App() { return (<div>...</div>) }`".into()),
            file_path: file_path.into(),
            line: 1, col: 1,
        });
    } else if source.components.iter().filter(|c| c.exported).count() > 1 {
        out.push(LintError {
            severity: "error".into(),
            code: "mx-export".into(),
            message: "Multiple default exports — only one allowed".into(),
            suggestion: Some("Keep a single `export default function App()`".into()),
            file_path: file_path.into(),
            line: 1, col: 1,
        });
    }

    // ── window checks ──
    if source.window_config.is_none() {
        if !source.components.is_empty() {
            let has_morph_window = source.components.iter().any(|c| jsx_has_tag(&c.jsx, "morph-window"));
            if !has_morph_window {
                out.push(LintError {
                    severity: "warning".into(),
                    code: "mx-window-missing".into(),
                    message: "Missing `windowConfig` export and no <morph-window>".into(),
                    suggestion: Some("Add `export const windowConfig = { title: \"App\", width: 800, height: 600 }`".into()),
                    file_path: file_path.into(),
                    line: 1, col: 1,
                });
            }
        }
    }

    // ── Walk JSX for tag/prop/list/JS global checks ──
    for comp in &source.components {
        lint_jsx(&comp.jsx, content, file_path, &mut out);
    }

    // ── JS globals scan (simple substring search with line info) ──
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") { continue; }
        for g in UNSUPPORTED_GLOBALS {
            if line.contains(&format!("{g}.")) || line.contains(&format!("{g}[")) || line.contains(&format!(" {g} ")) {
                let col = line.find(g).unwrap_or(0) + 1;
                out.push(LintError {
                    severity: "error".into(),
                    code: "mx-js-global".into(),
                    message: format!("`{g}` is not available in native runtime"),
                    suggestion: Some(format!("Use Morph state / C++ instead of browser `{g}`")),
                    file_path: file_path.into(),
                    line: idx + 1, col,
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
            if tag == target { return true; }
            children.iter().any(|c| jsx_has_tag(c, target))
        }
        JsxNode::Fragment { children, .. } => children.iter().any(|c| jsx_has_tag(c, target)),
        JsxNode::Conditional { then_branch, else_branch, .. } => {
            then_branch.iter().any(|c| jsx_has_tag(c, target)) || else_branch.iter().any(|c| jsx_has_tag(c, target))
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
                        suggestion: Some(format!("Use <div> with custom handling instead of <{tag}>")),
                        file_path: file_path.into(),
                        line: *line, col: *col,
                    });
                } else {
                    let suggestion = suggest_tag(tag);
                    out.push(LintError {
                        severity: "error".into(),
                        code: "mx-tag".into(),
                        message: format!("Unknown tag <{tag}>"),
                        suggestion: suggestion.map(|s| format!("Did you mean <{s}>?")),
                        file_path: file_path.into(),
                        line: *line, col: *col,
                    });
                }
            }

            // ── mx-prop ──
            for (prop, _) in props {
                if prop == "class" {
                    out.push(LintError {
                        severity: "warning".into(),
                        code: "mx-prop".into(),
                        message: format!("Use `className` instead of `class` on <{tag}>"),
                        suggestion: Some("Replace `class` with `className`".into()),
                        file_path: file_path.into(),
                        line: *line, col: *col,
                    });
                    continue;
                }
                if !is_allowed_prop(tag, prop) {
                    if prop.starts_with("on") {
                        out.push(LintError {
                            severity: "warning".into(),
                            code: "mx-prop".into(),
                            message: format!("Unknown prop `{prop}` on <{tag}>"),
                            suggestion: Some("Check event name (onClick, onInput, etc.)".into()),
                            file_path: file_path.into(),
                            line: *line, col: *col,
                        });
                    } else if SUPPORTED_TAGS.contains(&tag.as_str()) && !is_component_tag(tag) && prop.len() < 20 {
                        if let Some(s) = suggest_prop_for_tag(prop, tag) {
                            out.push(LintError {
                                severity: "warning".into(),
                                code: "mx-prop".into(),
                                message: format!("Unknown prop `{prop}` on <{tag}>"),
                                suggestion: Some(format!("Did you mean `{s}`?")),
                                file_path: file_path.into(),
                                line: *line, col: *col,
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
            for child in children { lint_jsx(child, _content, file_path, out); }
        }
        JsxNode::Conditional { then_branch, else_branch, .. } => {
            for c in then_branch { lint_jsx(c, _content, file_path, out); }
            for c in else_branch { lint_jsx(c, _content, file_path, out); }
        }
        JsxNode::List { key_expr, item_template, line, col, .. } => {
            if key_expr.is_empty() {
                out.push(LintError {
                    severity: "warning".into(),
                    code: "mx-list-key".into(),
                    message: "List rendering without `key` prop — may cause state mismatches".into(),
                    suggestion: Some("Add `key={item.id}` to the root element inside `.map()`".into()),
                    file_path: file_path.into(),
                    line: *line, col: *col,
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
        TAG_PROPS.iter().filter(|(t,_)| *t==tag).flat_map(|(_, props)| props.iter().copied())
    );
    for prop in candidates {
        let dist = strsim::levenshtein(input, prop) as f64;
        let max_len = input.len().max(prop.len()) as f64;
        let sim = 1.0 - (dist / max_len);
        if sim > 0.6 {
            if let Some((_, best_sim)) = &best {
                if sim > *best_sim { best = Some((prop.to_string(), sim)); }
            } else { best = Some((prop.to_string(), sim)); }
        }
    }
    best.map(|(s,_)| s)
}

// Helper for tag suggestion (exposed to crate)
pub fn suggest_tag(input: &str) -> Option<String> {
    let mut best: Option<(String, f64)> = None;
    for tag in SUPPORTED_TAGS {
        let dist = strsim::levenshtein(input, tag) as f64;
        let max_len = input.len().max(tag.len()) as f64;
        let sim = 1.0 - (dist / max_len);
        if sim > 0.5 {
            if let Some((_, bs)) = &best { if sim > *bs { best = Some((tag.to_string(), sim)); } } else { best = Some((tag.to_string(), sim)); }
        }
    }
    best.map(|(s,_)| s)
}
