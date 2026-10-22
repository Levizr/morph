use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::TranslateOptions;
use crate::codegen::cpp::{CppTranslator, cpp_string_literal};
use crate::codegen::rust::RustTranslator;
use crate::error::MorphJsError;

/// Seed ambient app state into a translator: mapped read expressions plus
/// recorded types for operand classes and member-access style.
fn apply_ambient_state(translator: &mut CppTranslator, options: &TranslateOptions) {
    translator.set_omit_param_defaults(options.omit_param_defaults);
    for (name, cpp_type) in &options.state_types {
        translator.ctx.var_types.insert(name.clone(), cpp_type.clone());
    }
    for (name, expression) in &options.state_vars {
        translator.ctx.state_vars.insert(name.clone(), expression.clone());
    }
}

pub fn translate_to_cpp(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<String, MorphJsError> {
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
                let mut translator = CppTranslator::new(
                    &normalized,
                    options.indent,
                    options.type_mode,
                    options.runtime_path.clone(),
                );
                apply_ambient_state(&mut translator, &options);
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
                let mut translator = CppTranslator::new(
                    &normalized,
                    options.indent,
                    options.type_mode,
                    options.runtime_path.clone(),
                );
                apply_ambient_state(&mut translator, &options);
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

    let mut translator =
        CppTranslator::new(source, options.indent, options.type_mode, options.runtime_path.clone());
    apply_ambient_state(&mut translator, &options);
    let code = translator.translate_program(&ret.program);
    Ok(code)
}

/// Translate one standalone expression against ambient app state.
/// `source` is parsed as an initializer, so object/array literals are
/// expressions rather than block statements.
pub fn translate_expression_to_cpp(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<crate::ExpressionOutput, MorphJsError> {
    let wrapped = format!("const __morph_expr = ({source});");
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filename).unwrap_or_default().with_typescript(true);
    let ret = Parser::new(&allocator, &wrapped, source_type).parse();
    if ret.panicked || !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|e| e.message.to_string()).collect();
        return Err(MorphJsError::Parse(format!(
            "parse errors in expression {}: {}",
            filename,
            msgs.join("; ")
        )));
    }
    let init = ret
        .program
        .body
        .iter()
        .find_map(|stmt| match stmt {
            oxc_ast::ast::Statement::VariableDeclaration(vd) => {
                vd.declarations.first().and_then(|d| d.init.as_ref())
            }
            _ => None,
        })
        .ok_or_else(|| MorphJsError::Parse(format!("empty expression in {filename}")))?;
    let mut translator = CppTranslator::new(
        &wrapped,
        options.indent,
        options.type_mode,
        options.runtime_path.clone(),
    );
    apply_ambient_state(&mut translator, &options);
    translator.analyze_program(&ret.program);
    let ty = translator.expression_type(init);
    let body = translator.emit_expression_host(init);
    let includes = translator.ctx.generate_includes();
    Ok(crate::ExpressionOutput { includes, body, ty })
}

/// Translate one `.mx`-embedded logic snippet (handler body, effect,
/// inner function) against ambient app state. Single parse attempt, no
/// C++-type fallback: snippets must be plain TypeScript; anything else
/// is a hard error for the caller to route elsewhere.
pub fn translate_snippet_to_cpp(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<String, MorphJsError> {
    Ok(translate_snippet_to_cpp_with_types(source, filename, &options)?.0)
}

/// Like [`translate_snippet_to_cpp`], but also returns the inferred C++
/// return types of functions declared in the snippet. The app builder
/// seeds these into ambient state types so later expressions (for example
/// `make().name`) choose bracket access for object factories.
pub(crate) fn translate_snippet_to_cpp_with_types(
    source: &str,
    filename: &str,
    options: &TranslateOptions,
) -> Result<(String, std::collections::HashMap<String, String>), MorphJsError> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(filename).unwrap_or_default().with_typescript(true);
    let ret = Parser::new(&allocator, source, source_type).parse();
    if ret.panicked || !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|e| e.message.to_string()).collect();
        return Err(MorphJsError::Parse(format!(
            "parse errors in snippet {}: {}",
            filename,
            msgs.join("; ")
        )));
    }
    let mut translator =
        CppTranslator::new(source, options.indent, options.type_mode, options.runtime_path.clone());
    apply_ambient_state(&mut translator, options);
    let mut out = translator.translate_program(&ret.program);
    // A snippet that is nothing but a string literal — a const RHS like
    // `const s = "hi"`, or JSX text — parses as a directive prologue:
    // oxc lifts it out of `body`, and the translation came back empty
    // (the const then vanished downstream). Re-emit those literals as
    // statements. Non-empty bodies keep dropping directives: there they
    // are real `"use strict"`-style prologues with no C++ meaning.
    if ret.program.body.is_empty() {
        for directive in &ret.program.directives {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&cpp_string_literal(&directive.expression.value));
            out.push_str(";\n");
        }
    }
    let fn_return_types = translator.ctx.fn_return_types.clone();
    Ok((out, fn_return_types))
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

pub fn translate_to_rust(
    source: &str,
    filename: &str,
    indent_level: usize,
) -> Result<String, MorphJsError> {
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
        return Err(MorphJsError::Parse(format!(
            "parse errors in {}: {}",
            filename,
            msgs.join("; ")
        )));
    }
    let mut translator = RustTranslator::new(source, indent_level);
    Ok(translator.translate_program(&ret.program))
}

/// For reuse inside other crates (e.g. morph-codegen) without file IO
pub fn translate_str(source: &str) -> Result<String, MorphJsError> {
    translate_to_cpp(source, "file.ts", TranslateOptions::default())
}

pub fn translate_str_rust(source: &str) -> Result<String, MorphJsError> {
    translate_to_rust(source, "file.ts", 0)
}
