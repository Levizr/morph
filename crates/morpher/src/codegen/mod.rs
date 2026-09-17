pub mod analyzer;
pub mod context;
pub mod cpp;
pub mod js_comparison;
pub mod rust;
pub mod string_methods;
pub mod type_resolver;

pub use analyzer::{
    AnalysisResult, DefSite, EscapeAnalyzer, EscapeKind, NarrowSplit, StmtSite, UsageKind, VarInfo,
    WidenedType,
};
pub use cpp::CppTranslator;
pub use js_comparison::{
    ComparisonKind, ComparisonSections, ComparisonSignature, OperandClass, build_header,
    cpp_type_to_class, full_sections, hoist_js_cmp_preludes, required_includes,
    ts_annotation_to_class,
};
pub use rust::RustTranslator;
