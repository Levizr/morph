#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::collections::HashMap;

pub mod builder;
pub mod css_registry;
pub mod node;
pub mod serializer;
pub mod style;
pub mod tailwind;
pub mod transforms;

pub use builder::{
    binding_ident, is_instance_slot, qualified_binding_ref, IRBuilder, MODULE_NS_ROOT,
};
pub use node::{IRAnimation, IRConditionalClassEffect, IREvent, IRKeyframe, IRNode, IRWindow};
pub use serializer::IRSerializer;
pub use style::IRStyle;
