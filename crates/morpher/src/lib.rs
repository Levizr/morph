pub mod codegen;
pub mod error;
pub mod linter;
pub mod parser;

pub use codegen::context::TypeMode;
pub use error::MorphJsError;
pub use parser::{translate_str, translate_to_cpp, translate_to_rust};

/// Options controlling translation
#[derive(Debug, Clone, Default)]
pub struct TranslateOptions {
    /// Enable optimized intent-based codegen (escape analysis, native types)
    pub optimize: bool,
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

/// Translate with default options (optimize=false, type_mode=Infer, no runtime path)
pub fn translate_default(source: &str, filename: &str) -> Result<String, MorphJsError> {
    translate(source, filename, TranslateOptions::default())
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
