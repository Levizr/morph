use serde::{Deserialize, Serialize};

/// Stub IR — will be expanded with full pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRNode {
    pub id: String,
    pub tag: String,
    pub props: std::collections::HashMap<String, String>,
    pub children: Vec<IRNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRWindow {
    pub width: u32,
    pub height: u32,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRPage {
    pub window: IRWindow,
    pub root: IRNode,
}
