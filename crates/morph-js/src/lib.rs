pub mod codegen;
pub mod error;
pub mod linter;
pub mod parser;

pub use error::MorphJsError;
pub use parser::{translate_str, translate_to_cpp, translate_to_rust};

/// High-level translate API used by morphc and other crates
/// Returns generated C++ code string
pub fn translate(source: &str, filename: &str) -> Result<String, MorphJsError> {
    parser::translate_to_cpp(source, filename, 0, false)
}

/// Translate with custom indent (for embedded usage)
pub fn translate_with_indent(source: &str, filename: &str, indent: usize) -> Result<String, MorphJsError> {
    parser::translate_to_cpp(source, filename, indent, false)
}

pub fn translate_rust(source: &str, filename: &str) -> Result<String, MorphJsError> {
    parser::translate_to_rust(source, filename, 0)
}

// Re-export for external crates
pub fn translate_file_to_cpp(path: &std::path::Path) -> Result<String, MorphJsError> {
    let source = std::fs::read_to_string(path).map_err(|e| MorphJsError::Io(e.to_string()))?;
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("file.ts");
    translate(&source, filename)
}
