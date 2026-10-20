use anyhow::{bail, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub(crate) mod ast_types;
pub mod css_lint;
mod css_parser;
pub mod css_registry;
pub(crate) mod js_walker;
pub mod linter;
pub mod resolve;
pub mod routes;

pub use ast_types::*;
pub use linter::{check as lint_check, docs_url, lint};
pub use resolve::{
    module_ns_path, module_ns_path_from_base, module_ns_segments, module_ns_segments_from_base,
    resolve_graph, resolve_import_path, ModuleGraph, ResolvedModule,
};

/// Resolve a module's component export by name.
///
/// `"default"` is the default-export component, a plain name matches a
/// component directly, and `export { Card as Button }` aliases map through
/// `named_exports`. Single source of truth for import/use-site resolution
/// in the IR builder and the linter.
pub fn exported_component<'a>(source: &'a MxSource, exported: &str) -> Option<&'a MxComponent> {
    if exported == "default" {
        return source.components.iter().find(|c| c.is_default);
    }
    if let Some(c) = source.components.iter().find(|c| c.name == exported && c.exported) {
        return Some(c);
    }
    let original = source.named_exports.iter().find(|(_, e)| e == exported).map(|(l, _)| l)?;
    source.components.iter().find(|c| c.name == *original && c.exported)
}

/// Resolve a component export through re-export chains (barrel modules).
///
/// Starts at `module` looking for `exported` (`"default"` for the default
/// export), follows `export { x } from './m'`, `export { default as X }
/// from './m'`, and `export * from './m'`, and returns the module that
/// actually declares the component plus the component itself. Cycles and
/// dead ends resolve to `None`.
pub fn resolve_component_export(
    graph: &ModuleGraph,
    module: &Path,
    exported: &str,
) -> Option<(PathBuf, MxComponent)> {
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let mut current_path = module.to_path_buf();
    let mut current_name = exported.to_string();
    loop {
        if !visited.insert(current_path.clone()) {
            return None;
        }
        let resolved = graph.modules.get(&current_path)?;
        if let Some(c) = exported_component(&resolved.source, &current_name) {
            return Some((current_path.clone(), c.clone()));
        }
        let mut next: Option<(String, String)> = None;
        for re in &resolved.source.re_exports {
            if re.star {
                // `export *` covers named exports only, never `default`.
                if current_name != "default" {
                    next = Some((re.path.clone(), current_name.clone()));
                    break;
                }
                continue;
            }
            if let Some((original, _)) = re.names.iter().find(|(_, e)| e == &current_name) {
                next = Some((re.path.clone(), original.clone()));
                break;
            }
        }
        let (raw_path, next_name) = next?;
        let target =
            resolved.module_imports.iter().find(|(p, _)| p == &raw_path).map(|(_, t)| t.clone())?;
        current_path = target;
        current_name = next_name;
    }
}

/// Parse an .mx file (which is TSX) and return a structured representation.
pub fn parse_mx_file(path: &Path) -> Result<MxSource> {
    let source = std::fs::read_to_string(path)?;
    let filename = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
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
        let msgs: Vec<String> = ret.diagnostics.iter().map(ToString::to_string).collect();
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
        global_var_names: walker.global_var_names,
        console_logs: walker.console_logs,
        extra_headers: walker.extra_headers,
        cpp_imports: walker.cpp_imports,
    })
}

/// Parse external CSS text into style rules + `@keyframes`.
pub fn parse_css(source: &str) -> Result<CssData> {
    css_parser::parse_css(source)
}
