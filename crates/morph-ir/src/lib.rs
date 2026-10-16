#[allow(unused_imports)]
use serde::{Deserialize, Serialize};
#[allow(unused_imports)]
use std::collections::HashMap;

pub mod builder;
pub mod gradient;
pub mod grid;
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
pub use grid::{
    GridArea, GridAreaValue, GridAutoFlow, GridLine, GridRepeatCount, GridTrackList,
    GridTrackMax, GridTrackMin, GridTrackSize, grid_areas_to_css, grid_line_to_css,
    track_list_to_css,
};
pub use style::{IRStyle, Length};
