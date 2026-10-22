pub mod codegen;
pub mod error;
pub mod linter;
pub mod parser;

pub use codegen::context::TypeMode;
pub use error::MorphJsError;
pub use parser::{translate_snippet_to_cpp, translate_str, translate_to_cpp, translate_to_rust};
pub use std::collections::HashMap;

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
    /// Ambient reads: JS name to C++ expression (`count` to `__st_count.get()`).
    /// Used when translating snippets spliced into a larger unit that owns
    /// these names. Empty by default (file translation declares its own).
    pub state_vars: HashMap<String, String>,
    /// Ambient types: JS name to recorded C++ type. Feeds operand classes
    /// (comparisons) and member-access style for ambient reads.
    pub state_types: HashMap<String, String>,
    /// Omit parameter defaults from top-level forward declarations. Project
    /// builds provide them once in `morph_api.h`; standalone translations
    /// keep them on the prototype and omit them from the definition.
    pub omit_param_defaults: bool,
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

/// Split leading `#include` lines (plus blanks) from the body.
/// Shared by fragment wrapping and snippet output: includes must merge at
/// the top of the hosting unit, never inline where spliced.
fn split_output_includes(code: &str) -> (&str, &str) {
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
    code.split_at(header_end)
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
    let mut namespace: String =
        stem.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect();
    if namespace.is_empty() || namespace.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        namespace.insert(0, '_');
    }
    namespace.push_str("_logic");

    let (header, body) = split_output_includes(code);
    format!("{}namespace {} {{\n\n{}}} // namespace {}\n", header, namespace, body, namespace)
}

/// A translated snippet: includes merge at the top of the hosting unit,
/// the body splices where the snippet sat.
#[derive(Debug, Clone, Default)]
pub struct SnippetOutput {
    /// Leading `#include` lines (plus blanks) of the translation.
    pub includes: String,
    /// Everything else: statements ready to splice.
    pub body: String,
    /// Inferred C++ return types for functions declared by the snippet.
    pub fn_return_types: HashMap<String, String>,
}

/// A translated standalone expression plus its inferred C++ type.
#[derive(Debug, Clone, Default)]
pub struct ExpressionOutput {
    /// Leading `#include` lines the expression needs.
    pub includes: String,
    /// The C++ expression, usable as an initializer.
    pub body: String,
    /// Inferred C++ type (`JsObject`, `std::vector<…>`, `bool`, …), when known.
    pub ty: Option<String>,
}

/// Translate `.mx`-embedded logic (handlers, effects, inner functions)
/// against ambient app state. Reads of `options.state_vars` names emit
/// the mapped C++ expressions; `options.state_types` feeds operand
/// classes and member-access style. Names the snippet declares itself
/// translate normally. Writes to ambient state need the app-side setter
/// convention (a Phase-2 build concern), so plain assignments to mapped
/// names are left as-is for the hosting unit to own.
pub fn translate_snippet(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<SnippetOutput, MorphJsError> {
    let (code, fn_return_types) =
        parser::translate_snippet_to_cpp_with_types(source, filename, &options)?;
    let (header, body) = split_output_includes(&code);
    Ok(SnippetOutput { includes: header.to_string(), body: body.to_string(), fn_return_types })
}

/// Translate one standalone expression against ambient app state.
/// Used for component consts: unlike [`translate_snippet`], an object or
/// array literal is an expression here, not a block statement.
pub fn translate_expression(
    source: &str,
    filename: &str,
    options: TranslateOptions,
) -> Result<ExpressionOutput, MorphJsError> {
    parser::translate_expression_to_cpp(source, filename, options)
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
        let code = translate_fragment(
            "let x = 1;\nconsole.log(x);\n",
            "app.ts",
            TranslateOptions::default(),
        )
        .unwrap();
        assert!(code.contains("namespace app_logic {"));
        assert!(code.ends_with("} // namespace app_logic\n"));
    }

    #[test]
    fn fragment_keeps_includes_at_top_level() {
        let code = translate_fragment(
            "let x = 1;\nconsole.log(x);\n",
            "app.ts",
            TranslateOptions::default(),
        )
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

    fn snippet_options() -> TranslateOptions {
        TranslateOptions {
            state_vars: [("count".to_string(), "__st_count.get()".to_string())]
                .into_iter()
                .collect(),
            state_types: [("count".to_string(), "JsNumber".to_string())].into_iter().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn snippet_maps_ambient_reads() {
        let out = translate_snippet("console.log(count + 1);\n", "handler.ts", snippet_options())
            .unwrap();
        assert!(out.body.contains("__st_count.get()"), "body: {}", out.body);
        assert!(!out.body.contains("count + 1"));
    }

    #[test]
    fn snippet_splits_includes_from_body() {
        let out =
            translate_snippet("console.log(count);\n", "handler.ts", snippet_options()).unwrap();
        assert!(out.includes.contains("#include"));
        assert!(!out.body.contains("#include"));
    }

    #[test]
    fn snippet_rejects_unparseable_input() {
        let err = translate_snippet("let = ;;;\n", "handler.ts", TranslateOptions::default());
        assert!(err.is_err());
    }

    #[test]
    fn snippet_keeps_bare_string_literal() {
        // `const s = "hi"` sends just the literal through the snippet
        // parser, where it parses as a directive prologue (oxc lifts it
        // out of `body`). The translation must still yield the literal,
        // or the const disappears and JSX references fail to compile.
        let out = translate_snippet("\"hi\"", "snippet.ts", TranslateOptions::default()).unwrap();
        assert_eq!(out.body.trim(), "\"hi\";");
        // Escapes survive through the C++ literal path.
        let out =
            translate_snippet("\"a\\\"b\\n\"", "snippet.ts", TranslateOptions::default()).unwrap();
        assert_eq!(out.body.trim(), "\"a\\\"b\\n\";");
    }

    #[test]
    fn snippet_drops_real_directive_prologues() {
        // Inside a non-empty program the leading literal is a real
        // directive and must not surface as a statement.
        let out = translate_snippet(
            "\"use strict\";\nlet x = 1;\nconsole.log(x);\n",
            "snippet.ts",
            TranslateOptions::default(),
        )
        .unwrap();
        assert!(!out.body.contains("use strict"), "body: {}", out.body);
        assert!(out.body.contains("x = 1"), "body: {}", out.body);
    }

    fn mapped_count(read: &str) -> TranslateOptions {
        let mut options = TranslateOptions::default();
        options.state_vars.insert("count".to_string(), read.to_string());
        options.state_types.insert("count".to_string(), "int".to_string());
        options
    }

    #[test]
    fn snippet_lowers_assignments_to_mapped_signal_setter() {
        // Body-level flow and inner closures write mutable locals through
        // the ambient mapping: `x = v` → `set(v)`, `x += v` reads back
        // through the mapping, `x++` becomes a one-step setter.
        let out = translate_snippet(
            "count = 1;\ncount += 2;\ncount++;\n",
            "body.ts",
            mapped_count("__st_count.get()"),
        )
        .unwrap();
        assert!(out.body.contains("__st_count.set(1);"), "body: {}", out.body);
        assert!(
            out.body.contains("__st_count.set((__st_count.get()) + (2));"),
            "body: {}",
            out.body
        );
        assert!(out.body.contains("__st_count.set((__st_count.get()) + 1);"), "body: {}", out.body);
    }

    #[test]
    fn snippet_lowers_peek_mapped_and_ctx_mapped_assignments() {
        // Body effects read their own locals through `peek()`; route
        // components map onto `ctx->` context members. The setter head is
        // derived by stripping either suffix.
        let out = translate_snippet("count += 2;\n", "body.ts", mapped_count("__st_total.peek()"))
            .unwrap();
        assert!(
            out.body.contains("__st_total.set((__st_total.peek()) + (2));"),
            "body: {}",
            out.body
        );
        let out = translate_snippet("count += 2;\n", "body.ts", mapped_count("ctx->count.get()"))
            .unwrap();
        assert!(
            out.body.contains("ctx->count.set((ctx->count.get()) + (2));"),
            "body: {}",
            out.body
        );
    }

    #[test]
    fn snippet_keeps_plain_assignment_without_mapping() {
        let out = translate_snippet("other += 2;\n", "body.ts", mapped_count("__st_count.get()"))
            .unwrap();
        assert!(!out.body.contains(".set("), "body: {}", out.body);
        assert!(out.body.contains("other += 2"), "body: {}", out.body);
    }

    #[test]
    fn for_of_over_js_sequence_converts_loop_var_in_numeric_context() {
        // `for (auto x : JsArray{...})` binds a JsValue element; `acc += x`
        // must ToNumber it — a raw operator+= cannot bind `JsNumber&` to the
        // element (`acc` is a native int).
        let out = translate_snippet(
            "let acc = 0;\nfor (const x of [1, 2]) {\n  acc += x;\n}\n",
            "body.ts",
            TranslateOptions::default(),
        )
        .unwrap();
        assert!(out.body.contains("std::get<JsNumber>(x.inner).as_int()"), "body: {}", out.body);
        assert!(out.body.contains("for (auto x : JsArray{1, 2})"), "body: {}", out.body);
    }

    #[test]
    fn for_of_over_native_sequence_keeps_plain_element() {
        // A `std::vector<int>` iterable keeps `x` as a plain int: no
        // ToNumber step is added.
        let options = TranslateOptions {
            state_vars: HashMap::from([("nums".to_string(), "nums".to_string())]),
            state_types: HashMap::from([("nums".to_string(), "std::vector<int>".to_string())]),
            ..Default::default()
        };
        let out = translate_snippet(
            "let acc = 0;\nfor (const x of nums) {\n  acc += x;\n}\n",
            "body.ts",
            options,
        )
        .unwrap();
        assert!(!out.body.contains("std::get<JsNumber>(x.inner)"), "body: {}", out.body);
        assert!(out.body.contains("acc += x"), "body: {}", out.body);
    }

    #[test]
    fn effect_cleanup_closure_captures_timer_handle() {
        // Regression for docs/dev/bugs#4: the cleanup arrow must capture
        // the timer handle with shared ownership instead of `[]`, and the
        // void `clearInterval` call must not be `return`ed from `-> auto`.
        let out = translate_snippet(
            "() => {\n  const id = setInterval(() => {}, 1000);\n  return () => clearInterval(id);\n}\n",
            "snippet.ts",
            TranslateOptions::default(),
        )
        .unwrap();
        assert!(out.body.starts_with("[]() -> auto"), "outer keeps `[]`: {}", out.body);
        assert!(
            out.body.contains("std::shared_ptr<int64_t> id = "),
            "handle is shared: {}",
            out.body
        );
        assert!(
            out.body.contains("[&, id]() -> void { morph::clear_timer((*id)); }"),
            "cleanup captures by value: {}",
            out.body
        );
    }

    #[test]
    fn plain_effect_body_keeps_empty_capture() {
        let out = translate_snippet(
            "() => {\n  console.log(count);\n}\n",
            "handler.ts",
            snippet_options(),
        )
        .unwrap();
        assert!(out.body.starts_with("[]() -> auto"), "body: {}", out.body);
        assert!(!out.body.contains("shared_ptr"), "body: {}", out.body);
    }
}
