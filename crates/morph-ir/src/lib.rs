use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod style;
pub mod node;

pub use style::IRStyle;
pub use node::{IRNode, IRWindow, IREvent, IRAnimation, IRKeyframe};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRPage {
    pub page_id: String,
    pub nodes: Vec<IRNode>,
}
