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
pub mod text_shadow;
pub mod transforms;

pub use builder::{
    binding_ident, is_instance_slot, qualified_binding_ref, IRBuilder, MODULE_NS_ROOT,
};
pub use gradient::{
    CenterCoord, GradientAxis, GradientKind, GradientPosition, GradientStop, IRGradient,
    RadialShape, RadialSize,
};
pub use grid::{
    grid_areas_to_css, grid_line_to_css, track_list_to_css, GridArea, GridAreaValue, GridAutoFlow,
    GridLine, GridRepeatCount, GridTrackList, GridTrackMax, GridTrackMin, GridTrackSize,
};
pub use node::{
    IRAnimation, IRConditionalClassEffect, IREvent, IRKeyframe, IRMediaStyle, IRNode, IRWindow,
};
pub use serializer::IRSerializer;
pub use shadow::{IRBoxShadow, MAX_BOX_SHADOWS};
pub use style::{IRStyle, Length};
pub use text_shadow::{IRTextShadow, MAX_TEXT_SHADOWS};
