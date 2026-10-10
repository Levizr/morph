//! Raw-CSS lint: unknown-property warnings with exact source spans.
//!
//! lightningcss drops unknown declarations before the builder ever sees
//! them, so this brace-aware scanner runs on the source text instead and
//! checks every declaration name against the same registry the builder
//! uses (`css_registry::is_known_property`) — check and build agree by
//! construction. Unknown properties warn (browsers ignore them; so does
//! Morph) with the exact line, while genuinely unsupported *behavior*
//! stays a hard error elsewhere.

use super::css_registry;
use crate::ast_types::LintError;

fn build_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0];
    for (i, ch) in source.char_indices() {
        if ch == '\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

fn line_col(offsets: &[usize], offset: usize) -> (usize, usize) {
    let line = match offsets.binary_search(&offset) {
        Ok(idx) => idx + 1,
        Err(idx) => idx,
    };
    let line_start = offsets[line.saturating_sub(1)];
    (line, offset.saturating_sub(line_start) + 1)
}

fn suggest_prop(input: &str) -> Option<String> {
    let mut best: Option<(&str, f64)> = None;
    for prop in css_registry::KNOWN_PROPERTIES {
        let sim = strsim::jaro_winkler(input, prop);
        if sim > 0.8 {
            match best {
                Some((_, s)) if sim <= s => {}
                _ => best = Some((prop, sim)),
            }
        }
    }
    best.map(|(s, _)| s.to_string())
}

fn push_warning(
    out: &mut Vec<LintError>,
    offsets: &[usize],
    file_path: &str,
    start: usize,
    name: &str,
) {
    let (line, col) = line_col(offsets, start);
    let (message, suggestion) = if name.starts_with("--") {
        (
            format!("Custom property `{name}` is not supported and will be dropped"),
            "Replace the variable with its value where it is used".to_string(),
        )
    } else {
        (
            format!("Unsupported CSS property `{name}` — Morph will ignore it"),
            suggest_prop(name).map_or_else(
                || "Check docs/css/properties.md for supported properties".to_string(),
                |s| format!("Did you mean `{s}`?"),
            ),
        )
    };
    out.push(LintError {
        severity: "warning".into(),
        code: "mx-css-prop".into(),
        message,
        suggestion: Some(suggestion),
        file_path: file_path.into(),
        line,
        col,
    });
}

struct Cursor<'a> {
    src: &'a [u8],
}

impl Cursor<'_> {
    /// Skip whitespace and `/* */` comments. Returns false at EOF.
    fn skip_trivia(&self, i: &mut usize) {
        while *i < self.src.len() {
            let c = self.src[*i];
            if c == b'/' && *i + 1 < self.src.len() && self.src[*i + 1] == b'*' {
                *i += 2;
                while *i + 1 < self.src.len() && !(self.src[*i] == b'*' && self.src[*i + 1] == b'/')
                {
                    *i += 1;
                }
                *i = (*i + 2).min(self.src.len());
            } else if c.is_ascii_whitespace() {
                *i += 1;
            } else {
                break;
            }
        }
    }

    /// Skip a quoted string starting at `i` (which points at the quote).
    const fn skip_string(&self, i: &mut usize) {
        let quote = self.src[*i];
        *i += 1;
        while *i < self.src.len() {
            let c = self.src[*i];
            if c == b'\\' {
                *i += 2;
            } else if c == quote {
                *i += 1;
                break;
            } else {
                *i += 1;
            }
        }
    }

    /// Read until `{`, `;` or `}` at this level (strings/comments aware).
    /// Returns the token start and the delimiter (or `None` at EOF).
    fn read_token(&self, i: &mut usize) -> (usize, Option<u8>) {
        let start = *i;
        while *i < self.src.len() {
            let c = self.src[*i];
            match c {
                b'"' | b'\'' => self.skip_string(i),
                b'/' if *i + 1 < self.src.len() && self.src[*i + 1] == b'*' => {
                    let mut j = *i;
                    self.skip_trivia(&mut j);
                    *i = j;
                }
                b'{' | b';' | b'}' => {
                    let d = c;
                    *i += 1;
                    return (start, Some(d));
                }
                _ => *i += 1,
            }
        }
        (start, None)
    }

    /// Find the matching `}` for the `{` at `open` (points past it).
    fn matching_close(&self, i: &mut usize) {
        let mut depth = 1usize;
        while *i < self.src.len() && depth > 0 {
            let c = self.src[*i];
            match c {
                b'"' | b'\'' => self.skip_string(i),
                b'/' if *i + 1 < self.src.len() && self.src[*i + 1] == b'*' => {
                    let mut j = *i;
                    self.skip_trivia(&mut j);
                    *i = j;
                }
                b'{' => {
                    depth += 1;
                    *i += 1;
                }
                b'}' => {
                    depth -= 1;
                    *i += 1;
                }
                _ => *i += 1,
            }
        }
    }
}

/// Check one declaration fragment (`name: value`). The name starts at
/// `start` (leading trivia already skipped).
fn check_declaration(
    src: &str,
    start: usize,
    end: usize,
    out: &mut Vec<LintError>,
    offsets: &[usize],
    file_path: &str,
) {
    let frag = &src[start..end];
    let Some(colon) = frag.find(':') else { return };
    let name = frag[..colon].trim();
    if name.is_empty() || name.starts_with('@') {
        return;
    }
    // Only identifier-shaped names are properties (`a:hover` preludes
    // never reach here — they end in `{`, not `;`).
    if !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_') {
        return;
    }
    let lower = name.to_ascii_lowercase();
    if css_registry::is_known_property(&lower) {
        return;
    }
    let name_off = start + frag.find(name).unwrap_or(0);
    push_warning(out, offsets, file_path, name_off, &lower);
}

/// Scan one brace level: declarations end in `;`, nested rules in
/// `{...}` (selectors, `@media`, `@keyframes` steps all recurse).
fn scan_block(
    cur: &Cursor,
    src: &str,
    mut i: usize,
    end: usize,
    out: &mut Vec<LintError>,
    offsets: &[usize],
    file_path: &str,
) {
    loop {
        cur.skip_trivia(&mut i);
        if i >= end {
            break;
        }
        if cur.src[i] == b'}' {
            break;
        }
        let (start, delim) = cur.read_token(&mut i);
        match delim {
            Some(b'{') => {
                let inner = i;
                cur.matching_close(&mut i);
                scan_block(cur, src, inner, i.saturating_sub(1).min(end), out, offsets, file_path);
            }
            Some(b';') => {
                check_declaration(src, start, i - 1, out, offsets, file_path);
            }
            Some(b'}') => {
                check_declaration(src, start, i - 1, out, offsets, file_path);
                break;
            }
            _ => {
                check_declaration(src, start, i.min(end), out, offsets, file_path);
                break;
            }
        }
    }
}

/// Lint one CSS file: unknown (dropped) properties warn with exact
/// spans; unparseable stylesheets warn once instead of failing silent.
pub fn check_css_file(content: &str, file_path: &str) -> Vec<LintError> {
    let mut out = Vec::new();
    let offsets = build_offsets(content);
    let cur = Cursor { src: content.as_bytes() };
    scan_block(&cur, content, 0, content.len(), &mut out, &offsets, file_path);
    // A stylesheet lightningcss itself rejects loses ALL its rules at
    // build time (silently skipped) — say so once, with its location.
    if let Err(e) = crate::parse_css(content) {
        let msg = e.to_string();
        let (line, col) = css_error_location(&msg);
        out.push(LintError {
            severity: "warning".into(),
            code: "mx-css-syntax".into(),
            message: format!("Stylesheet has a syntax error and may lose rules: {msg}"),
            suggestion: Some("Fix the syntax error so every rule applies".into()),
            file_path: file_path.into(),
            line,
            col,
        });
    }
    out
}

/// Best-effort `line:col` out of a lightningcss error string
/// (`"... at :2:47"` / `"... at 2:47"`); falls back to the file start.
fn css_error_location(msg: &str) -> (usize, usize) {
    let Some(at) = msg.rfind("at ") else { return (1, 1) };
    let tail = msg[at + 3..].trim().trim_start_matches(':');
    let mut parts = tail.split(':');
    match (parts.next(), parts.next()) {
        (Some(l), Some(c)) => {
            let col: String = c.chars().take_while(char::is_ascii_digit).collect();
            match (l.parse::<usize>(), col.parse::<usize>()) {
                (Ok(line), Ok(col)) if line > 0 && col > 0 => (line, col),
                _ => (1, 1),
            }
        }
        _ => (1, 1),
    }
}

/// Verify the stylesheets one module imports.
///
/// Every `CssLocal` import must resolve on disk (same candidates the
/// build uses), or the app ships unstyled with no indication why.
/// Shared by `morph check` and the `morph build` lint gate so both
/// enforce the same rule.
pub fn check_css_imports(
    module_path: &std::path::Path,
    content: &str,
    cwd: &std::path::Path,
) -> Vec<LintError> {
    let filename =
        module_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let Ok(src) = crate::parse_mx_str(content, &filename) else { return Vec::new() };
    let dir = module_path.parent().unwrap_or(cwd);
    let mut out = Vec::new();
    for imp in &src.imports {
        let crate::MxImportKind::CssLocal { path } = &imp.kind else { continue };
        let found = [dir.join(path), cwd.join(path)].iter().any(|c| c.is_file());
        if found {
            continue;
        }
        let (line, col) = locate_import(content, path);
        out.push(LintError {
            severity: "error".into(),
            code: "mx-css-file-missing".into(),
            message: format!("CSS file not found: `{path}`"),
            suggestion: Some(
                "Check the relative path — imports resolve from the importing file".into(),
            ),
            file_path: module_path.display().to_string(),
            line,
            col,
        });
    }
    out
}

/// Locate an import path literal in source text for error spans.
fn locate_import(content: &str, path: &str) -> (usize, usize) {
    let mut line = 1usize;
    let mut line_start = 0usize;
    let needle = format!("\"{path}\"");
    let off = content.find(needle.as_str()).or_else(|| content.find(path)).unwrap_or(0);
    for (i, ch) in content.char_indices() {
        if i >= off {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    (line, off.saturating_sub(line_start) + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn warns(src: &str) -> Vec<LintError> {
        check_css_file(src, "style.css")
    }

    #[test]
    fn known_props_are_quiet() {
        let errs = warns(".a { color: red; margin: 0; display: flex; }\n");
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn unknown_prop_warns_with_span() {
        let errs = warns(".a {\n  widht: 10px;\n  color: red;\n}\n");
        assert_eq!(errs.len(), 1, "{errs:?}");
        let e = &errs[0];
        assert_eq!(e.code, "mx-css-prop");
        assert_eq!(e.severity, "warning");
        assert_eq!((e.line, e.col), (2, 3));
        assert!(e.message.contains("widht"), "{e:?}");
        assert!(e.suggestion.as_deref().unwrap().contains("width"), "{e:?}");
    }

    #[test]
    fn pseudo_selectors_and_media_are_not_declarations() {
        let errs = warns("@media (min-width: 100px) {\n  a:hover { color: red; }\n}\n");
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn strings_comments_urls_do_not_confuse() {
        let errs = warns("/* color: red; */\n.a { color: \"a;b:c\"; background: url(data:image/png;base64,AA==); }\n");
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn custom_properties_warn() {
        let errs = warns(".a { --brand: red; }\n");
        assert_eq!(errs.len(), 1, "{errs:?}");
        assert!(errs[0].message.contains("--brand"), "{errs:?}");
    }

    #[test]
    fn keyframes_and_last_decl_without_semicolon() {
        let errs = warns(
            "@keyframes x {\n  from { opacity: 0 }\n  to { opacity: 1; }\n}\n.a { color: red }\n",
        );
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn missing_css_import_errors_with_span() {
        let dir = std::env::temp_dir().join("morph_css_import_test");
        let _ = std::fs::create_dir_all(&dir);
        let app = dir.join("App.mx");
        let content =
            "import \"./nope.css\"\nexport default function App() { return (<div>x</div>) }\n";
        std::fs::write(&app, content).unwrap();
        let errs = check_css_imports(&app, content, &dir);
        assert_eq!(errs.len(), 1, "{errs:?}");
        let e = &errs[0];
        assert_eq!(e.code, "mx-css-file-missing");
        assert_eq!(e.severity, "error");
        assert_eq!((e.line, e.col), (1, 8));
        assert!(e.message.contains("./nope.css"), "{e:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn present_css_import_is_quiet() {
        let dir = std::env::temp_dir().join("morph_css_import_ok_test");
        let _ = std::fs::create_dir_all(&dir);
        let app = dir.join("App.mx");
        std::fs::write(dir.join("style.css"), ".a { color: red; }\n").unwrap();
        let content =
            "import \"./style.css\"\nexport default function App() { return (<div>x</div>) }\n";
        std::fs::write(&app, content).unwrap();
        let errs = check_css_imports(&app, content, &dir);
        assert!(errs.is_empty(), "{errs:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
