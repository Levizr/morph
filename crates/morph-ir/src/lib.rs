use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod style;
pub mod node;
pub mod css_registry;
pub mod tailwind;
pub mod builder;

pub use style::IRStyle;
pub use node::{IRNode, IRWindow, IREvent, IRAnimation, IRKeyframe};
pub use builder::IRBuilder;
