use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::IRStyle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRNode {
    pub node_id: String,
    pub node_type: String,
    pub style: IRStyle,
    pub hover_style: Option<IRStyle>,
    pub active_style: Option<IRStyle>,
    pub children: Vec<Self>,
    pub events: Vec<IREvent>,
    pub text_content: String,
    pub attrs: HashMap<String, String>,
    pub reactive_attrs: HashMap<String, String>,
    pub reactive_text: String,
    pub reactive_class: String,
    pub reactive_style: HashMap<String, String>,
    pub class_conditional_effects: Vec<IRConditionalClassEffect>,
    pub condition_expr: String,
    pub then_nodes: Vec<Self>,
    pub else_nodes: Vec<Self>,
    pub list_expr: String,
    pub list_key_expr: String,
    pub item_template: Option<Box<Self>>,
    pub list_item_param: String,
    pub list_index_param: String,
    pub animations: Vec<IRAnimation>,
    pub hover_animations: Vec<IRAnimation>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub transition_duration: f32,
    pub transition_easing: String,
}

impl Default for IRNode {
    fn default() -> Self {
        Self {
            node_id: String::new(),
            node_type: String::new(),
            style: IRStyle::new(),
            hover_style: None,
            active_style: None,
            children: vec![],
            events: vec![],
            text_content: String::new(),
            attrs: HashMap::new(),
            reactive_attrs: HashMap::new(),
            reactive_text: String::new(),
            reactive_class: String::new(),
            reactive_style: HashMap::new(),
            class_conditional_effects: vec![],
            condition_expr: String::new(),
            then_nodes: vec![],
            else_nodes: vec![],
            list_expr: String::new(),
            list_key_expr: String::new(),
            item_template: None,
            list_item_param: "item".to_string(),
            list_index_param: String::new(),
            animations: vec![],
            hover_animations: vec![],
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
            transition_duration: 0.0,
            transition_easing: "ease-in-out".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IRWindow {
    pub window_id: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub min_width: Option<u32>,
    pub max_width: Option<u32>,
    pub min_height: Option<u32>,
    pub max_height: Option<u32>,
    pub modal: bool,
    pub renderer: String,
    pub nodes: Vec<IRNode>,
    pub startup_logs: Vec<String>,
    pub premain_functions: Vec<String>,
    pub extra_headers: Vec<String>,
    pub state_vars: Vec<HashMap<String, String>>,
    pub reactive_consts: Vec<String>,
    /// Route root prop declarations (empty for entry windows): one entry
    /// per declared prop — `name`, `class` (C++ type class), `optional`.
    /// Props are context members (`ctx-><name>`, like state); the mount
    /// prologue assigns each from the runtime `__props` object.
    #[serde(default)]
    pub route_props: Vec<HashMap<String, String>>,
    pub effect_decls: Vec<HashMap<String, String>>,
    /// morphShared declarations, one entry per declaration site.
    pub shared_vars: Vec<HashMap<String, String>>,
    /// morphEvent declarations, one entry per visible event binding:
    /// `key` (module path + name identity), `ns` (module namespace),
    /// `accessor` (`evt_<name>`), `event` (source name), `channel`
    /// (legacy string id, dev-TU only), `module` (display path for
    /// header mapping comments). The build TU emits one static
    /// `Channel` per entry and lowers string channel references to
    /// the accessor; the dev TU keeps the string registry.
    #[serde(default)]
    pub event_decls: Vec<HashMap<String, String>>,
    /// Event subscriptions: `channel` + `body` (a `[](const JsValue&)`
    /// listener). Emitters subscribe at startup.
    pub channel_subs: Vec<HashMap<String, String>>,
    /// `mid` native index assignments, one entry per (tagged instance ×
    /// state slot): `ns` (component scope), `comp`, `mid`, `const`
    /// (`MID_*`), `index` (ordinal among tagged instances), `signal`
    /// (backing `__st_` static), `getter`/`setter`/`init`, `module`, `loc`.
    #[serde(default)]
    pub mid_assignments: Vec<HashMap<String, String>>,
    /// Universal module bindings (functions/vars/classes + re-export
    /// aliases): `key` (module path + name identity), `kind`
    /// (`function`/`var`/`class`/`alias`), `ns`, `name`, `module`, plus
    /// `target_ns`/`target_name` for aliases. Codegen emits definitions
    /// in the defining namespace and `using`-aliases for re-exports.
    #[serde(default)]
    pub module_bindings: Vec<HashMap<String, String>>,
    pub cpp_imports: Vec<HashMap<String, String>>,
    pub keyframes: HashMap<String, Vec<IRKeyframe>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IREvent {
    pub trigger: String,
    pub action: String,
    pub target: String,
}

/// A conditional class style effect: when `condition` (a C++ bool expr) is true,
///
/// apply `on_styles`; otherwise apply `off_styles` (CSS property → value),
/// or reset affected fields to defaults when `off_styles` is empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRConditionalClassEffect {
    pub condition: String,
    pub on_styles: HashMap<String, String>,
    pub off_styles: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRAnimation {
    pub name: String,
    pub easing: String,
    pub direction: String,
    pub fill_mode: String,
    pub play_state: String,
    pub duration: f32,
    pub delay: f32,
    pub iterations: f32,
}

impl Default for IRAnimation {
    fn default() -> Self {
        Self {
            name: String::new(),
            easing: "linear".into(),
            direction: "normal".into(),
            fill_mode: "none".into(),
            play_state: "running".into(),
            duration: 0.0,
            delay: 0.0,
            iterations: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IRKeyframe {
    pub offset: f32,
    pub style: IRStyle,
    /// IR style field names explicitly declared (e.g. "opacity", "bg_color").
    pub declared: Vec<String>,
    /// Raw CSS values needing layout-time resolution (transforms, % lengths).
    /// Property name (CSS) → CSS string.
    pub raw: HashMap<String, String>,
}
