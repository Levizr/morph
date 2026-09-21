use std::collections::HashMap;

/// A parsed .mx source file.
#[derive(Debug, Clone)]
pub struct MxSource {
    pub filename: String,
    pub imports: Vec<MxImport>,
    pub window_config: Option<WindowConfig>,
    pub components: Vec<MxComponent>,
    /// Exported top-level `const [getter, setter] = morphShared<T>(init)`.
    pub shared_bindings: Vec<SharedBinding>,
    /// Exported top-level `const name = morphEvent<T>()`.
    pub event_bindings: Vec<EventBinding>,
    pub state_vars: Vec<StateVar>,
    pub effects: Vec<MxEffect>,
    pub inner_functions: Vec<InnerFunction>,
    pub function_declarations: Vec<InnerFunction>,
    /// Top-level `class C { ... }` declarations (any module kind).
    pub class_declarations: Vec<ClassDecl>,
    /// Top-level exported `const`/`let` bindings (non-component,
    /// non-destructured): `{name, declarator source}`.
    pub exported_vars: Vec<ExportedVar>,
    /// `export { local as exported }` without a `from` clause.
    pub named_exports: Vec<(String, String)>,
    /// Declared name behind `export default` (`None` when anonymous).
    pub default_export: Option<String>,
    /// `export ... from './path'` re-exports (resolved into the graph).
    pub re_exports: Vec<ReExport>,
    pub global_vars: Vec<String>,
    pub console_logs: Vec<String>,
    pub extra_headers: Vec<String>,
    pub cpp_imports: Vec<CppImport>,
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    pub visible: bool,
    pub modal: bool,
    /// Owner id/route, `""` = none, `"auto"` = most-recently-focused.
    /// String literals only; anything else is ignored (consistent with
    /// the other keys — dynamic values cannot lower to WIDs).
    pub parent: String,
    /// Presentation role literal (`""` = default). Validated at lowering.
    pub role: String,
}

#[derive(Debug, Clone)]
pub enum MxImportKind {
    CssLocal {
        path: String,
    },
    CssUrl {
        url: String,
    },
    CppLocal {
        path: String,
        specifiers: Vec<String>,
    },
    /// Module import: `default` is the local name of a default import;
    /// `specifiers` are `(local, imported)` pairs (`import { a as b }`
    /// records `("b", "a")`; non-aliased pairs repeat the name).
    Component {
        path: String,
        default: Option<String>,
        specifiers: Vec<(String, String)>,
    },
}

impl MxImportKind {
    /// Raw module path, if this import references a file/module.
    pub fn path(&self) -> Option<&str> {
        match self {
            Self::CssLocal { path }
            | Self::CppLocal { path, .. }
            | Self::Component { path, .. } => Some(path),
            Self::CssUrl { .. } => None,
        }
    }

    /// True for `.mx` component-module imports.
    pub fn is_mx(&self) -> bool {
        matches!(self, Self::Component { path, .. } if std::path::Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mx")))
    }

    /// True for the `morph` runtime module.
    pub fn is_morph_module(&self) -> bool {
        matches!(self, Self::Component { path, .. } if path == "morph")
    }

    /// True for TypeScript helper imports (`.ts`, but not `.tsx`: the
    /// extension check alone preserves that distinction).
    pub fn is_ts(&self) -> bool {
        matches!(self, Self::Component { path, .. } if std::path::Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("ts")))
    }

    /// True for `.tsx` helper imports.
    pub fn is_tsx(&self) -> bool {
        matches!(self, Self::Component { path, .. } if std::path::Path::new(path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tsx")))
    }

    /// True for morph module imports that carry user bindings: `.mx`, `.ts`,
    /// or `.tsx` component modules (everything except the `morph` runtime).
    pub fn is_module_source(&self) -> bool {
        self.is_mx() || self.is_ts() || self.is_tsx()
    }
}

#[derive(Debug, Clone)]
pub struct MxImport {
    pub kind: MxImportKind,
    pub style: String, // "import" | "css_load"
}

#[derive(Debug, Clone)]
pub struct CppImport {
    pub path: String,
    pub specifiers: Vec<String>,
}

/// One declared prop of a reusable component. `prop_type` is the raw TS
/// annotation text, interpreted by IR/codegen.
#[derive(Debug, Clone)]
pub struct ComponentProp {
    pub name: String,
    pub prop_type: String,
    pub optional: bool,
}

impl ComponentProp {
    /// Best-effort check for callable prop types.
    pub fn is_function(&self) -> bool {
        let t = self.prop_type.trim();
        if t.is_empty() {
            return false;
        }
        // Top-level `=>` outside any brackets means a function type.
        let mut depth = 0i32;
        let mut chars = t.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '<' | '(' | '[' | '{' => depth += 1,
                '>' | ')' | ']' | '}' => depth -= 1,
                '=' if depth == 0 && chars.peek() == Some(&'>') => return true,
                '"' | '\'' => {
                    chars.by_ref().find(|nc| *nc == c);
                }
                _ => {}
            }
        }
        t == "Function" || t.starts_with("Function<") || t.starts_with("(...")
    }
}

#[derive(Debug, Clone)]
pub struct MxComponent {
    pub name: String,
    pub exported: bool,
    /// Statement-level `<event>.on(handler)` subscriptions.
    pub event_subs: Vec<EventSub>,
    /// True for the module's default export.
    pub is_default: bool,
    /// Props parameter name when declared as an identifier; empty when
    /// destructured or absent.
    pub props_param: String,
    /// Declared props; empty when the component takes no props or is untyped.
    pub props: Vec<ComponentProp>,
    /// Declared prop names, derived from `props`.
    pub params: Vec<String>,
    pub jsx: JsxNode,
    pub state_vars: Vec<StateVar>,
    pub effects: Vec<MxEffect>,
    pub inner_functions: Vec<InnerFunction>,
    pub consts: Vec<ComponentConst>,
    pub console_logs: Vec<String>,
}

impl MxComponent {
    /// Declared prop by name.
    pub fn prop(&self, name: &str) -> Option<&ComponentProp> {
        self.props.iter().find(|p| p.name == name)
    }
}

#[derive(Debug, Clone)]
pub struct StateVar {
    pub getter: String,
    pub setter: String,
    pub init: String,
    /// Raw text of the explicit type argument, e.g. `number` from
    /// `morphState<number>(0)`. Empty when omitted.
    pub type_arg: Option<String>,
}

/// An exported top-level `const [getter, setter] = morphShared<T>(init)`.
///
/// `morphShared` may only appear at module scope; the binding is shared by
/// importing its getter/setter names from this file.
#[derive(Debug, Clone)]
pub struct SharedBinding {
    pub getter: String,
    pub setter: String,
    pub type_arg: Option<String>,
    pub init: String,
    pub line: usize,
    pub col: usize,
}

/// An exported top-level `const name = morphEvent<T>()` channel.
#[derive(Debug, Clone)]
pub struct EventBinding {
    pub name: String,
    pub type_arg: Option<String>,
    pub line: usize,
    pub col: usize,
}

/// A statement-level `<event>.on(handler)` subscription inside a component
/// body, with the raw handler source.
#[derive(Debug, Clone)]
pub struct EventSub {
    pub event: String,
    pub handler: String,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct MxEffect {
    pub callback: String,
    pub deps: String,
}

#[derive(Debug, Clone)]
pub struct InnerFunction {
    pub name: String,
    pub source: String,
    /// True when declared with an `export` keyword (importable cross-file).
    /// Component-inner helpers are never exported.
    pub exported: bool,
}

/// A top-level `class C { ... }` declaration with its source span.
#[derive(Debug, Clone)]
pub struct ClassDecl {
    pub name: String,
    pub source: String,
    pub exported: bool,
}

/// A top-level exported `const`/`let` binding with its declarator source.
#[derive(Debug, Clone)]
pub struct ExportedVar {
    pub name: String,
    pub source: String,
}

/// An `export ... from './path'` re-export: `(original, exported)` name
/// pairs (`star` for `export *`, with optional `export * as ns` target).
#[derive(Debug, Clone)]
pub struct ReExport {
    pub path: String,
    pub names: Vec<(String, String)>,
    pub star: bool,
    pub star_as: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ComponentConst {
    pub name: String,
    pub rhs: String,
}

/// Intermediate JSX representation (mirrors Python's dict-based IR).
#[derive(Debug, Clone)]
pub enum JsxNode {
    Element {
        tag: String,
        props: HashMap<String, JsxPropValue>,
        children: Vec<Self>,
        self_closing: bool,
        line: usize,
        col: usize,
    },
    Fragment {
        props: HashMap<String, JsxPropValue>,
        children: Vec<Self>,
        line: usize,
        col: usize,
    },
    Text(String),
    Expression(String),
    Conditional {
        condition: String,
        then_branch: Vec<Self>,
        else_branch: Vec<Self>,
        line: usize,
        col: usize,
    },
    List {
        array_expr: String,
        item_param: String,
        index_param: String,
        key_expr: String,
        item_template: Box<Self>,
        line: usize,
        col: usize,
    },
}

/// Lint diagnostic — mirrors Python's LintError
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LintError {
    pub severity: String, // "error" | "warning"
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub file_path: String,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub enum JsxPropValue {
    String(String),
    Expr(String),
    Ref(String),
    Fn(String),
    Template(String),
    Style(HashMap<String, StyleValue>),
    Bool,
}

#[derive(Debug, Clone)]
pub enum StyleValue {
    Static(String),
    Expr(String),
}

/// A single CSS style rule: its selector plus its declarations.
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selector: String,
    pub properties: HashMap<String, String>,
}

/// A single `@keyframes` entry: a selector offset (0..1) plus its declarations.
#[derive(Debug, Clone)]
pub struct CssKeyframe {
    pub offset: f32,
    pub properties: HashMap<String, String>,
}

/// The result of parsing a CSS file: style rules in source order (so equal
/// specificity resolves by last-write like a browser), and the raw `@keyframes`
/// registry keyed by animation name.
#[derive(Debug, Clone, Default)]
pub struct CssData {
    pub rules: Vec<(String, CssRule)>,
    pub keyframes: HashMap<String, Vec<CssKeyframe>>,
}
