use std::collections::{BTreeSet, HashMap, HashSet};
use std::str::FromStr;

use super::type_resolver::headers_for;

pub const INDENT: &str = "    ";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TypeMode {
    /// Respect user-declared type annotations; infer types only for unannotated vars
    Strict,
    /// Ignore user annotations and always analyze the code to pick the best native type
    #[default]
    Infer,
}

impl FromStr for TypeMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "strict" => Ok(TypeMode::Strict),
            "infer" => Ok(TypeMode::Infer),
            _ => Err(format!("invalid type mode: {} (expected 'strict' or 'infer')", s)),
        }
    }
}

impl std::fmt::Display for TypeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TypeMode::Strict => write!(f, "strict"),
            TypeMode::Infer => write!(f, "infer"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Ctx {
    pub indent_level: usize,
    pub needed: BTreeSet<String>,
    /// Absolute path to the runtime cpp directory (used to emit global includes).
    pub runtime_path: Option<String>,
    /// Type resolution mode (strict vs infer).
    pub type_mode: TypeMode,
    pub template_params: HashSet<String>,
    pub class_names: HashSet<String>,
    pub interface_props: HashMap<String, HashMap<String, String>>, // iface name -> prop -> cpp type
    pub var_types: HashMap<String, String>,
    pub shared_ptr_vars: HashSet<String>,
    pub fn_expr_depth: usize,
    pub fn_body_depth: usize,
    pub is_async_fn: usize,
    pub has_infinite_loop: bool,
    pub drop_return_value: bool,
    pub class_name: Option<String>,
    pub super_class_name: Option<String>,
    pub event_handler: bool,
    pub state_vars: HashMap<String, String>,
    pub js_object_params: HashSet<String>,
    pub async_fns: HashSet<String>,
}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            indent_level: 1,
            needed: BTreeSet::new(),
            runtime_path: None,
            type_mode: TypeMode::Infer,
            template_params: HashSet::new(),
            class_names: HashSet::new(),
            interface_props: HashMap::new(),
            var_types: HashMap::new(),
            shared_ptr_vars: HashSet::new(),
            fn_expr_depth: 0,
            fn_body_depth: 0,
            is_async_fn: 0,
            has_infinite_loop: false,
            drop_return_value: false,
            class_name: None,
            super_class_name: None,
            event_handler: false,
            state_vars: HashMap::new(),
            js_object_params: HashSet::new(),
            async_fns: HashSet::new(),
        }
    }
}

impl Ctx {
    pub fn indent(&self) -> String {
        INDENT.repeat(self.indent_level)
    }

    pub fn sub(&self) -> Self {
        let mut sub = Ctx {
            indent_level: self.indent_level + 1,
            needed: BTreeSet::new(),
            runtime_path: self.runtime_path.clone(),
            type_mode: self.type_mode,
            template_params: self.template_params.clone(),
            class_names: self.class_names.clone(),
            interface_props: self.interface_props.clone(),
            var_types: self.var_types.clone(),
            shared_ptr_vars: self.shared_ptr_vars.clone(),
            fn_expr_depth: self.fn_expr_depth,
            fn_body_depth: self.fn_body_depth,
            is_async_fn: self.is_async_fn,
            has_infinite_loop: self.has_infinite_loop,
            drop_return_value: self.drop_return_value,
            class_name: self.class_name.clone(),
            super_class_name: self.super_class_name.clone(),
            event_handler: self.event_handler,
            state_vars: self.state_vars.clone(),
            js_object_params: self.js_object_params.clone(),
            async_fns: self.async_fns.clone(),
            ..Default::default()
        };
        // Preserve indent_level correctly: sub indent = parent+1
        sub.indent_level = self.indent_level + 1;
        sub
    }

    pub fn merge(&mut self, child: Ctx) {
        for h in child.needed {
            self.needed.insert(h);
        }
        self.has_infinite_loop = self.has_infinite_loop || child.has_infinite_loop;
        self.is_async_fn = self.is_async_fn.max(child.is_async_fn);
        self.drop_return_value = self.drop_return_value || child.drop_return_value;
        // var_types and shared_ptr_vars are shared via clone, but merge new entries
        for (k, v) in child.var_types {
            self.var_types.insert(k, v);
        }
        for v in child.shared_ptr_vars {
            self.shared_ptr_vars.insert(v);
        }
        for v in child.async_fns {
            self.async_fns.insert(v);
        }
    }

    pub fn need(&mut self, cpp_type: &str) {
        for h in headers_for(cpp_type) {
            self.needed.insert(h.to_string());
        }
    }

    pub fn generate_includes(&self) -> String {
        if self.needed.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        for h in &self.needed {
            let include = if let Some(ref rt) = self.runtime_path {
                // Transform relative runtime includes to absolute paths
                if h.starts_with("\"../../runtime/cpp/") {
                    // h is like "\"../../runtime/cpp/types/js_types.h\"" (with quotes)
                    // Remove leading quote, prefix, and trailing quote
                    let without_prefix = &h[19..]; // remove "\"../../runtime/cpp/"
                    let suffix = without_prefix.trim_end_matches('"');
                    format!("\"{}/{}\"", rt.trim_end_matches('/'), suffix)
                } else {
                    h.to_string()
                }
            } else {
                h.to_string()
            };
            out.push_str(&format!("#include {}\n", include));
        }
        out.trim_end().to_string()
    }

    pub fn async_result_type(&self, resolved: &str) -> String {
        if resolved == "void" || resolved == "auto" {
            return "morph::Task".to_string();
        }
        if resolved.starts_with("morph::Result") || resolved.starts_with("morph::Task") {
            return resolved.to_string();
        }
        format!("morph::Result<{}>", resolved)
    }
}
