use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::codegen::cpp::CppTranslator;
use crate::codegen::rust::RustTranslator;
use crate::error::MorphJsError;

pub struct TranslateOutput {
    pub code: String,
    pub headers: Vec<String>,
}

pub fn translate_to_cpp(source: &str, filename: &str, indent_level: usize) -> Result<String, MorphJsError> {
    // Fast path: parse with oxc bump allocator
    let allocator = Allocator::default();
    // Determine source type: treat .ts/.tsx as typescript, .js as js but still allow types
    let source_type = if filename.ends_with(".tsx") || filename.ends_with(".ts") {
        SourceType::from_path(filename).unwrap_or_default().with_typescript(true)
    } else if filename.ends_with(".jsx") {
        SourceType::from_path(filename).unwrap_or_default().with_jsx(true)
    } else {
        // default to ts for .js/.ts to be permissive
        SourceType::from_path("file.ts").unwrap().with_typescript(true)
    };

    let ret = Parser::new(&allocator, source, source_type).parse();

    // Handle panicked (Rust-specific strictness for `std::` qualified types)
    // Python tree-sitter is lenient and recovers via ERROR nodes, but oxc panics on `::`
    // Fallback: normalize C++ types to valid TS and re-parse
    if ret.panicked {
        // Try to normalize `std::vector<int>` -> `Array<number>`, `std::string` -> `string`, etc.
        let normalized = normalize_cpp_types(source);
        if normalized != source {
            let alloc2 = Allocator::default();
            let ret2 = Parser::new(&alloc2, &normalized, source_type).parse();
            if !ret2.panicked && ret2.diagnostics.is_empty() {
                let mut translator = CppTranslator::new(&normalized, indent_level);
                let code = translator.translate_program(&ret2.program);
                return Ok(code);
            }
        }
        return Err(MorphJsError::Parse(format!("parser panicked on {}", filename)));
    }
    if !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|e| e.message.to_string()).collect();
        // Also try normalized fallback for diagnostics
        let normalized = normalize_cpp_types(source);
        if normalized != source {
            let alloc2 = Allocator::default();
            let ret2 = Parser::new(&alloc2, &normalized, source_type).parse();
            if !ret2.panicked && ret2.diagnostics.is_empty() {
                let mut translator = CppTranslator::new(&normalized, indent_level);
                let code = translator.translate_program(&ret2.program);
                return Ok(code);
            }
        }
        return Err(MorphJsError::Parse(format!(
            "parse errors in {}: {}",
            filename,
            msgs.join("; ")
        )));
    }

    let mut translator = CppTranslator::new(source, indent_level);
    let code = translator.translate_program(&ret.program);
    Ok(code)
}

fn normalize_cpp_types(source: &str) -> String {
    // Map C++ qualified types to valid TS equivalents for oxc parser
    // User proposal: std::string -> std_string (valid TS identifier) then remap "_" -> "::" after oxc
    // This makes `std::vector<int>` etc. valid TS for oxc strict parser
    let mut s = source.to_string();
    // Generic `::` -> `_` placeholder for all std:: types (e.g., std::string -> std_string, std::vector -> std_vector)
    // This is more generic than hardcoding each type and handles any std:: qualified type
    if s.contains("::") {
        s = s.replace("::", "_");
    }
    // For std::vector, std::optional etc. with `<`, the `<` remains valid TS generics
    // e.g., std_vector<int> is valid TS as generic type `std_vector<int>` (identifier with `<int>`)
    // No further change needed for `<>`
    // Handle std::string that was not caught by `::` replacement due to already being `std_string`
    // (already handled above via `::` -> `_`)
    s
}

pub fn translate_to_rust(source: &str, filename: &str, indent_level: usize) -> Result<String, MorphJsError> {
    let allocator = Allocator::default();
    let source_type = if filename.ends_with(".tsx") || filename.ends_with(".ts") {
        SourceType::from_path(filename).unwrap_or_default().with_typescript(true)
    } else if filename.ends_with(".jsx") {
        SourceType::from_path(filename).unwrap_or_default().with_jsx(true)
    } else {
        SourceType::from_path("file.ts").unwrap().with_typescript(true)
    };
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked {
        let normalized = normalize_cpp_types(source);
        if normalized != source {
            let alloc2 = Allocator::default();
            let ret2 = Parser::new(&alloc2, &normalized, source_type).parse();
            if !ret2.panicked && ret2.diagnostics.is_empty() {
                let mut translator = RustTranslator::new(&normalized, indent_level);
                return Ok(translator.translate_program(&ret2.program));
            }
        }
        return Err(MorphJsError::Parse(format!("parser panicked on {}", filename)));
    }
    if !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|e| e.message.to_string()).collect();
        let normalized = normalize_cpp_types(source);
        if normalized != source {
            let alloc2 = Allocator::default();
            let ret2 = Parser::new(&alloc2, &normalized, source_type).parse();
            if !ret2.panicked && ret2.diagnostics.is_empty() {
                let mut translator = RustTranslator::new(&normalized, indent_level);
                return Ok(translator.translate_program(&ret2.program));
            }
        }
        return Err(MorphJsError::Parse(format!("parse errors in {}: {}", filename, msgs.join("; "))));
    }
    let mut translator = RustTranslator::new(source, indent_level);
    Ok(translator.translate_program(&ret.program))
}

/// For reuse inside other crates (e.g. morph-codegen) without file IO
pub fn translate_str(source: &str) -> Result<String, MorphJsError> {
    translate_to_cpp(source, "file.ts", 0)
}

pub fn translate_str_rust(source: &str) -> Result<String, MorphJsError> {
    translate_to_rust(source, "file.ts", 0)
}
