pub mod analyzer;
pub mod context;
pub mod cpp;
pub mod rust;
pub mod type_resolver;

pub use analyzer::{EscapeAnalyzer, EscapeKind, WidenedType, AnalysisResult, VarInfo, UsageKind};
pub use cpp::CppTranslator;
pub use rust::RustTranslator;
