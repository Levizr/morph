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

    if ret.panicked {
        return Err(MorphJsError::Parse(format!("parser panicked on {}", filename)));
    }
    if !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|e| e.message.to_string()).collect();
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
        return Err(MorphJsError::Parse(format!("parser panicked on {}", filename)));
    }
    if !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|e| e.message.to_string()).collect();
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
