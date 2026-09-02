use std::collections::HashMap;

/// A parsed .mx source file.
#[derive(Debug, Clone)]
pub struct MxSource {
    pub filename: String,
    pub imports: Vec<MxImport>,
    pub window_config: Option<WindowConfig>,
    pub components: Vec<MxComponent>,
    pub state_vars: Vec<StateVar>,
    pub effects: Vec<MxEffect>,
    pub inner_functions: Vec<InnerFunction>,
    pub function_declarations: Vec<InnerFunction>,
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
}

#[derive(Debug, Clone)]
pub enum MxImportKind {
    CssLocal { path: String },
    CssUrl { url: String },
    CppLocal { path: String, specifiers: Vec<String> },
    Component { path: String, specifiers: Vec<String> },
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

#[derive(Debug, Clone)]
pub struct MxComponent {
    pub name: String,
    pub exported: bool,
    pub params: Vec<String>,
    pub jsx: JsxNode,
    pub state_vars: Vec<StateVar>,
    pub effects: Vec<MxEffect>,
    pub inner_functions: Vec<InnerFunction>,
    pub consts: Vec<ComponentConst>,
    pub console_logs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StateVar {
    pub getter: String,
    pub setter: String,
    pub init: String,
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
        children: Vec<JsxNode>,
        self_closing: bool,
        line: usize,
        col: usize,
    },
    Fragment {
        props: HashMap<String, JsxPropValue>,
        children: Vec<JsxNode>,
        line: usize,
        col: usize,
    },
    Text(String),
    Expression(String),
    Conditional {
        condition: String,
        then_branch: Vec<JsxNode>,
        else_branch: Vec<JsxNode>,
        line: usize,
        col: usize,
    },
    List {
        array_expr: String,
        item_param: String,
        index_param: String,
        key_expr: String,
        item_template: Box<JsxNode>,
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
