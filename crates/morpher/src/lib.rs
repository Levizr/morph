pub mod codegen;
pub mod error;
pub mod linter;
pub mod parser;

pub use codegen::context::TypeMode;
pub use error::MorphJsError;
pub use parser::{translate_str, translate_to_cpp, translate_to_rust};

/// Options controlling translation.
/// Intent-based codegen (escape analysis, type widening, native types) is
/// always on; there is no optimize flag.
#[derive(Debug, Clone, Default)]
pub struct TranslateOptions {
    /// Type resolution mode (default: Infer)
    pub type_mode: TypeMode,
    /// Absolute path to the runtime cpp directory (used to emit global includes).
    /// If None, relative includes like "../../runtime/cpp/..." are used.
    pub runtime_path: Option<String>,
    /// Indent level for generated code
    pub indent: usize,
}

impl TranslateOptions {
    pub fn new() -> Self {
        Self::default()
    }
}

/// High-level translate API used by morphc and other crates.
/// Returns generated C++ code string.
pub fn translate(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<String, MorphJsError> {
    parser::translate_to_cpp(source, filename, options)
}

/// Translate with default options (type_mode=Infer, no runtime path)
pub fn translate_default(source: &str, filename: &str) -> Result<String, MorphJsError> {
    translate(source, filename, TranslateOptions::default())
}

/// Translate a file as a linkable fragment for app builds (GUI mode).
/// Same translation as [`translate`], then wrapped so the result links
/// into a larger binary: `#include` lines stay at top level, everything
/// else goes inside a per-file namespace (no `main`, no globals leak).
/// CLI file morphing is untouched and keeps emitting standalone code.
pub fn translate_fragment(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<String, MorphJsError> {
    let code = translate_to_cpp(source, filename, options)?;
    Ok(wrap_fragment_namespace(&code, filename))
}

/// Split leading `#include` lines from the body and wrap the body in a
/// namespace derived from the file stem (`app.ts` → `app_logic`). Nested
/// helper blocks (e.g. `morph::js_cmp`) nest along harmlessly: every use
/// site sits inside the same wrapper, so names keep resolving.
fn wrap_fragment_namespace(code: &str, filename: &str) -> String {
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("fragment");
    let mut namespace: String = stem
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect();
    if namespace.is_empty() || namespace.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        namespace.insert(0, '_');
    }
    namespace.push_str("_logic");

    let mut header_end = 0;
    for line in code.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("#include") {
            header_end += line.len() + 1;
        } else {
            break;
        }
    }
    let header_end = header_end.min(code.len());
    let (header, body) = code.split_at(header_end);
    format!("{}namespace {} {{\n\n{}}} // namespace {}\n", header, namespace, body, namespace)
}

/// Translate with custom indent and default options
pub fn translate_with_indent(
    source: &str,
    filename: &str,
    indent: usize,
) -> Result<String, MorphJsError> {
    translate(source, filename, TranslateOptions { indent, ..Default::default() })
}

pub fn translate_rust(source: &str, filename: &str) -> Result<String, MorphJsError> {
    parser::translate_to_rust(source, filename, 0)
}

// Re-export for external crates
pub fn translate_file_to_cpp(path: &std::path::Path) -> Result<String, MorphJsError> {
    let source = std::fs::read_to_string(path).map_err(|e| MorphJsError::Io(e.to_string()))?;
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file.ts");
    translate_default(&source, filename)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragment_wraps_body_in_file_namespace() {
        let code =
            translate_fragment("let x = 1;\nconsole.log(x);\n", "app.ts", TranslateOptions::default())
                .unwrap();
        assert!(code.contains("namespace app_logic {"));
        assert!(code.ends_with("} // namespace app_logic\n"));
    }

    #[test]
    fn fragment_keeps_includes_at_top_level() {
        let code =
            translate_fragment("let x = 1;\nconsole.log(x);\n", "app.ts", TranslateOptions::default())
                .unwrap();
        let namespace_at = code.find("namespace app_logic").unwrap();
        for line in code[..namespace_at].lines() {
            let trimmed = line.trim();
            assert!(trimmed.is_empty() || trimmed.starts_with("#include"), "line: {line}");
        }
    }

    #[test]
    fn fragment_namespace_sanitizes_stem() {
        let code = translate_fragment("let x = 1;\n", "my-app.v2.ts", TranslateOptions::default())
            .unwrap();
        assert!(code.contains("namespace my_app_v2_logic {"));
    }

    #[test]
    fn fragment_keeps_exe_output_untouched() {
        let source = "let x = 1;\nconsole.log(x);\n";
        let exe = translate(source, "app.ts", TranslateOptions::default()).unwrap();
        assert!(!exe.contains("namespace app_logic"));
    }
}
