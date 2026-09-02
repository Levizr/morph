use std::collections::{BTreeSet, HashMap, HashSet};

use super::type_resolver::headers_for;

pub const INDENT: &str = "    ";

#[derive(Debug, Clone)]
pub struct Ctx {
    pub indent_level: usize,
    pub needed: BTreeSet<String>,
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
}

impl Default for Ctx {
    fn default() -> Self {
        Self {
            indent_level: 1,
            needed: BTreeSet::new(),
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
            out.push_str(&format!("#include {}\n", h));
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
