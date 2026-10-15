#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::collections::HashMap;

pub mod builder;
pub mod gradient;
pub mod node;
pub mod serializer;
pub mod shadow;
pub mod style;
pub mod tailwind;
pub mod transforms;

pub use builder::{
    binding_ident, is_instance_slot, qualified_binding_ref, IRBuilder, MODULE_NS_ROOT,
};
pub use gradient::{
    CenterCoord, GradientAxis, GradientKind, GradientPosition, GradientStop, IRGradient,
    RadialShape, RadialSize,
};
pub use node::{IRAnimation, IRConditionalClassEffect, IREvent, IRKeyframe, IRNode, IRWindow};
pub use serializer::IRSerializer;
pub use shadow::{IRBoxShadow, MAX_BOX_SHADOWS};
pub use style::{IRStyle, Length};
