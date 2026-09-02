use anyhow::{bail, Result};
use std::path::Path;

pub(crate) mod ast_types;
pub(crate) mod js_walker;
mod css_parser;
pub mod linter;

pub use ast_types::*;
pub use linter::{check as lint_check, lint};

/// Parse an .mx file (which is TSX) and return a structured representation.
pub fn parse_mx_file(path: &Path) -> Result<MxSource> {
    let source = std::fs::read_to_string(path)?;
    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    parse_mx_str(&source, &filename)
}

/// Parse .mx source text into structured representation.
pub fn parse_mx_str(source: &str, filename: &str) -> Result<MxSource> {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = oxc_span::SourceType::from_path("file.tsx").unwrap();

    let ret = oxc_parser::Parser::new(&allocator, source, source_type).parse();

    if ret.panicked {
        bail!("Parser panicked on {filename}");
    }
    if !ret.diagnostics.is_empty() {
        let msgs: Vec<String> = ret.diagnostics.iter().map(|d| d.to_string()).collect();
        bail!("Parse errors in {filename}: {}", msgs.join("; "));
    }

    let mut walker = js_walker::MxWalker::new(source);
    {
        use oxc_ast_visit::Visit;
        walker.visit_program(&ret.program);
    }

    Ok(MxSource {
        filename: filename.to_string(),
        imports: walker.imports,
        window_config: walker.window_config,
        components: walker.components,
        state_vars: walker.state_vars,
        effects: walker.effects,
        inner_functions: walker.inner_functions,
        function_declarations: walker.function_declarations,
        global_vars: walker.global_vars,
        console_logs: walker.console_logs,
        extra_headers: walker.extra_headers,
        cpp_imports: walker.cpp_imports,
    })
}

/// Parse external CSS text into style rules + `@keyframes`.
pub fn parse_css(source: &str) -> Result<ast_types::CssData> {
    css_parser::parse_css(source)
}
