use std::collections::HashSet;

/// Operand type class used to describe both sides of a JavaScript comparison.
///
/// Widths are intentionally collapsed (`int32_t` and `int64_t` are both
/// `Integer`) so analyzer-side classification matches codegen-side emission
/// even when escape analysis picks different integer widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OperandClass {
    Boolean,
    Integer,
    Float,
    Text,
    Null,
    Undefined,
    JsBoolean,
    JsNumber,
    JsString,
    JsValue,
    JsArray,
    JsObject,
    Vector,
    Other,
}

impl OperandClass {
    /// True for `bool`, integers, floats and text (all native C++ types).
    pub fn is_native(self) -> bool {
        matches!(
            self,
            OperandClass::Boolean
                | OperandClass::Integer
                | OperandClass::Float
                | OperandClass::Text
        )
    }

    /// True for any `Js*` runtime type.
    pub fn is_js(self) -> bool {
        matches!(
            self,
            OperandClass::JsBoolean
                | OperandClass::JsNumber
                | OperandClass::JsString
                | OperandClass::JsValue
                | OperandClass::JsArray
                | OperandClass::JsObject
                | OperandClass::Null
                | OperandClass::Undefined
        )
    }

    /// True for `std::string`-like and `JsString` operands.
    pub fn is_textual(self) -> bool {
        matches!(self, OperandClass::Text | OperandClass::JsString)
    }

    /// True for integer, float and `JsNumber` operands.
    pub fn is_numeric(self) -> bool {
        matches!(self, OperandClass::Integer | OperandClass::Float | OperandClass::JsNumber)
    }
}

/// Map an emitted C++ type name to its operand class.
///
/// Unknown or complex types (`auto`, `std::vector`, `morph::Result`, ...)
/// map to `Vector` or `Other` so callers can fall back to direct emission.
pub fn cpp_type_to_class(cpp_type: &str) -> OperandClass {
    let normalized = cpp_type.trim_start_matches("const ").trim_end_matches('&').trim();
    match normalized {
        "bool" => OperandClass::Boolean,
        "JsBoolean" => OperandClass::JsBoolean,
        "JsNumber" => OperandClass::JsNumber,
        "JsString" => OperandClass::JsString,
        "JsValue" => OperandClass::JsValue,
        "JsArray" => OperandClass::JsArray,
        "JsObject" => OperandClass::JsObject,
        "JsNull" | "std::nullptr_t" | "nullptr_t" => OperandClass::Null,
        "JsUndefined" => OperandClass::Undefined,
        "float" | "double" => OperandClass::Float,
        "std::string" | "std::string_view" | "const char*" | "char*" => OperandClass::Text,
        _ => {
            if normalized.starts_with("std::vector") {
                OperandClass::Vector
            } else if normalized.starts_with("Js") {
                OperandClass::Other
            } else if normalized == "char"
                || normalized.starts_with("int")
                || normalized.starts_with("uint")
                || normalized.starts_with("long")
                || normalized.starts_with("short")
                || normalized == "size_t"
            {
                OperandClass::Integer
            } else {
                OperandClass::Other
            }
        }
    }
}

/// Map a TypeScript annotation name to its operand class.
pub fn ts_annotation_to_class(annotation: &str) -> OperandClass {
    match annotation {
        "boolean" | "Boolean" => OperandClass::Boolean,
        "number" | "Number" | "bigint" => OperandClass::Float,
        "string" | "String" => OperandClass::Text,
        "null" => OperandClass::Null,
        "undefined" | "void" => OperandClass::Undefined,
        "any" | "unknown" | "object" => OperandClass::JsValue,
        "JsBoolean" | "boolean[]" => OperandClass::JsBoolean,
        "JsNumber" => OperandClass::JsNumber,
        "JsString" => OperandClass::JsString,
        "JsValue" => OperandClass::JsValue,
        "JsArray" => OperandClass::JsArray,
        "JsObject" => OperandClass::JsObject,
        "JsNull" => OperandClass::Null,
        "JsUndefined" => OperandClass::Undefined,
        _ => OperandClass::Other,
    }
}

/// The kind of JavaScript comparison or truthiness test observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ComparisonKind {
    LooseEqual,
    LooseNotEqual,
    StrictEqual,
    StrictNotEqual,
    LessThan,
    GreaterThan,
    LessEqual,
    GreaterEqual,
    TruthyTest,
}

impl ComparisonKind {
    /// Map a binary operator spelling to its comparison kind.
    pub fn from_binary_operator(operator: &str) -> Option<ComparisonKind> {
        match operator {
            "==" => Some(ComparisonKind::LooseEqual),
            "!=" => Some(ComparisonKind::LooseNotEqual),
            "===" => Some(ComparisonKind::StrictEqual),
            "!==" => Some(ComparisonKind::StrictNotEqual),
            "<" => Some(ComparisonKind::LessThan),
            ">" => Some(ComparisonKind::GreaterThan),
            "<=" => Some(ComparisonKind::LessEqual),
            ">=" => Some(ComparisonKind::GreaterEqual),
            _ => None,
        }
    }

    /// True for `==` / `!=`.
    pub fn is_loose(self) -> bool {
        matches!(self, ComparisonKind::LooseEqual | ComparisonKind::LooseNotEqual)
    }

    /// True for `===` / `!==`.
    pub fn is_strict(self) -> bool {
        matches!(self, ComparisonKind::StrictEqual | ComparisonKind::StrictNotEqual)
    }

    /// True for `<` / `>` / `<=` / `>=`.
    pub fn is_relational(self) -> bool {
        matches!(
            self,
            ComparisonKind::LessThan
                | ComparisonKind::GreaterThan
                | ComparisonKind::LessEqual
                | ComparisonKind::GreaterEqual
        )
    }

    /// C++ dispatcher function name emitted by codegen for this kind.
    pub fn dispatcher_name(self) -> &'static str {
        match self {
            ComparisonKind::LooseEqual => "loose_eq",
            ComparisonKind::LooseNotEqual => "loose_not_eq",
            ComparisonKind::StrictEqual => "strict_eq",
            ComparisonKind::StrictNotEqual => "strict_not_eq",
            ComparisonKind::LessThan => "loose_lt",
            ComparisonKind::GreaterThan => "loose_gt",
            ComparisonKind::LessEqual => "loose_le",
            ComparisonKind::GreaterEqual => "loose_ge",
            ComparisonKind::TruthyTest => "is_truthy",
        }
    }
}

/// One observed comparison: operator kind plus both operand classes.
///
/// For `TruthyTest` the tested operand class is stored in `left`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComparisonSignature {
    pub kind: ComparisonKind,
    pub left: OperandClass,
    pub right: OperandClass,
}

impl ComparisonSignature {
    /// Build a binary comparison signature.
    pub fn binary(
        kind: ComparisonKind,
        left: OperandClass,
        right: OperandClass,
    ) -> ComparisonSignature {
        ComparisonSignature { kind, left, right }
    }

    /// Build a truthiness-test signature for a single operand class.
    pub fn truthy(operand: OperandClass) -> ComparisonSignature {
        ComparisonSignature { kind: ComparisonKind::TruthyTest, left: operand, right: operand }
    }
}

/// Section flags derived from observed signatures.
///
/// The header builder emits only the sections whose flag is set, so generated
/// translation units never carry unused helpers.
#[derive(Debug, Clone, Copy, Default)]
pub struct ComparisonSections {
    pub loose: bool,
    pub strict: bool,
    pub relational: bool,
    pub truthy: bool,
    pub text: bool,
    pub float_ops: bool,
    pub js_types: bool,
    pub js_value: bool,
}

impl ComparisonSections {
    /// Compute section flags from the union of observed signatures.
    pub fn from_signatures(signatures: &HashSet<ComparisonSignature>) -> ComparisonSections {
        let mut sections = ComparisonSections::default();
        for signature in signatures {
            match signature.kind {
                ComparisonKind::LooseEqual | ComparisonKind::LooseNotEqual => {
                    sections.loose = true;
                }
                ComparisonKind::StrictEqual | ComparisonKind::StrictNotEqual => {
                    sections.strict = true;
                }
                ComparisonKind::LessThan
                | ComparisonKind::GreaterThan
                | ComparisonKind::LessEqual
                | ComparisonKind::GreaterEqual => {
                    sections.relational = true;
                }
                ComparisonKind::TruthyTest => {
                    sections.truthy = true;
                }
            }
            for operand in [signature.left, signature.right] {
                match operand {
                    OperandClass::Text | OperandClass::JsString => {
                        sections.text = true;
                    }
                    OperandClass::Float => {
                        sections.float_ops = true;
                    }
                    OperandClass::JsBoolean
                    | OperandClass::JsNumber
                    | OperandClass::JsString
                    | OperandClass::JsArray
                    | OperandClass::JsObject => {
                        sections.js_types = true;
                    }
                    OperandClass::JsValue => {
                        sections.js_types = true;
                        sections.js_value = true;
                    }
                    OperandClass::Null | OperandClass::Undefined => {
                        sections.js_types = true;
                    }
                    _ => {}
                }
            }
        }
        sections
    }

    /// True when at least one helper section is required.
    pub fn needs_helpers(self) -> bool {
        self.loose || self.strict || self.relational || self.truthy
    }
}

/// Build the `morph::js_cmp` helper block for the given sections.
///
/// Returns an empty string when no helper section is required. The block is
/// emitted inline in the generated translation unit (after includes, before
/// the body) so no extra header file or include path is ever needed.
pub fn build_header(sections: ComparisonSections) -> String {
    if !sections.needs_helpers() {
        return String::new();
    }
    let mut block = String::new();
    block.push_str("namespace morph::js_cmp\n{\n\n");
    block.push_str(header_preamble());
    block.push_str(native_concepts());
    // Js* helpers operate on std::string internally, so text support is
    // required whenever Js interop is emitted.
    let text_support = sections.text || sections.js_types;
    if text_support {
        block.push_str(text_helpers());
    }
    block.push_str(number_helper());
    if sections.js_types && (sections.loose || sections.strict || sections.relational) {
        block.push_str(js_concepts());
        block.push_str(js_helper_forward_declarations());
    }
    if sections.truthy {
        block.push_str(&is_truthy_native(sections.float_ops, text_support));
    }
    if sections.loose {
        block.push_str(loose_equal_native());
    }
    if sections.strict {
        block.push_str(strict_equal_native());
    }
    if sections.relational {
        block.push_str(relational_native());
    }
    if sections.js_types {
        if sections.loose || sections.strict || sections.relational {
            block.push_str(&js_number_text_helpers(text_support));
        }
        if sections.truthy {
            block.push_str(is_truthy_js_overloads());
        }
        if sections.loose {
            block.push_str(loose_forward_declarations());
            block.push_str(js_loose_overloads());
        }
        if sections.strict {
            block.push_str(strict_forward_declarations());
            block.push_str(js_strict_overloads());
        }
        if sections.relational && sections.text {
            block.push_str(js_relational_text_overloads());
        }
        if sections.js_value && sections.loose {
            block.push_str(js_value_helpers());
        }
    }
    if sections.loose {
        block.push_str(loose_not_equal_wrapper());
    }
    if sections.strict {
        block.push_str(strict_not_equal_wrapper());
    }
    block.push_str("\n} // namespace morph::js_cmp\n");
    block
}

/// Standard includes required by the emitted helper block.
pub fn required_includes(sections: ComparisonSections) -> Vec<&'static str> {
    let mut includes = vec!["<type_traits>", "<cstdint>"];
    if sections.text || sections.js_types {
        includes.push("<string>");
        includes.push("<string_view>");
        includes.push("<charconv>");
        includes.push("<limits>");
    }
    if sections.float_ops || sections.text || sections.js_types {
        includes.push("<cmath>");
    }
    includes
}

fn header_preamble() -> &'static str {
    r#"#if defined(_MSC_VER)
#define MORPH_JS_CMP_INLINE __forceinline
#else
#define MORPH_JS_CMP_INLINE inline __attribute__((always_inline))
#endif

"#
}

fn native_concepts() -> &'static str {
    r#"template <typename OperandType>
concept JsCmpBool = std::is_same_v<std::remove_cvref_t<OperandType>, bool>;

template <typename OperandType>
concept JsCmpInt = std::is_integral_v<std::remove_cvref_t<OperandType>>
    && !JsCmpBool<OperandType>;

template <typename OperandType>
concept JsCmpFloat = std::is_floating_point_v<std::remove_cvref_t<OperandType>>;

template <typename OperandType>
concept JsCmpNumber = JsCmpInt<OperandType> || JsCmpFloat<OperandType>;

template <typename OperandType>
concept JsCmpText = std::is_same_v<std::remove_cvref_t<OperandType>, std::string>
    || std::is_same_v<std::remove_cvref_t<OperandType>, std::string_view>
    || std::is_same_v<std::remove_cvref_t<OperandType>, const char*>
    || std::is_same_v<std::remove_cvref_t<OperandType>, char*>
    || (std::is_array_v<std::remove_cvref_t<OperandType>>
        && std::is_same_v<
            std::remove_all_extents_t<std::remove_cvref_t<OperandType>>,
            char>);

"#
}

fn text_helpers() -> &'static str {
    r#"MORPH_JS_CMP_INLINE std::string_view js_cmp_to_view(const std::string& text) noexcept
{
    return std::string_view(text);
}

MORPH_JS_CMP_INLINE std::string_view js_cmp_to_view(std::string_view text) noexcept
{
    return text;
}

MORPH_JS_CMP_INLINE std::string_view js_cmp_to_view(const char* text) noexcept
{
    return text != nullptr ? std::string_view(text) : std::string_view();
}

template <std::size_t Length>
MORPH_JS_CMP_INLINE std::string_view js_cmp_to_view(const char (&text)[Length]) noexcept
{
    return Length > 0 ? std::string_view(text, Length - 1) : std::string_view();
}

MORPH_JS_CMP_INLINE double js_cmp_parse_text(std::string_view text) noexcept
{
    std::size_t begin = 0;
    std::size_t finish = text.size();
    while (begin < finish && (text[begin] == ' ' || text[begin] == '\t' || text[begin] == '\n'
                              || text[begin] == '\r' || text[begin] == '\v'
                              || text[begin] == '\f'))
    {
        ++begin;
    }
    while (finish > begin && (text[finish - 1] == ' ' || text[finish - 1] == '\t'
                              || text[finish - 1] == '\n' || text[finish - 1] == '\r'
                              || text[finish - 1] == '\v' || text[finish - 1] == '\f'))
    {
        --finish;
    }
    if (begin == finish)
    {
        return 0.0;
    }
    std::string_view trimmed = text.substr(begin, finish - begin);
    if (trimmed == "Infinity" || trimmed == "+Infinity")
    {
        return std::numeric_limits<double>::infinity();
    }
    if (trimmed == "-Infinity")
    {
        return -std::numeric_limits<double>::infinity();
    }
    const char* digits = trimmed.data();
    const char* digits_end = trimmed.data() + trimmed.size();
    if (trimmed.size() > 2 && trimmed[0] == '0'
        && (trimmed[1] == 'x' || trimmed[1] == 'X'))
    {
        unsigned long long parsed = 0;
        auto result = std::from_chars(digits + 2, digits_end, parsed, 16);
        if (result.ec == std::errc() && result.ptr == digits_end)
        {
            return static_cast<double>(parsed);
        }
        return std::numeric_limits<double>::quiet_NaN();
    }
    if (trimmed.size() > 2 && trimmed[0] == '0'
        && (trimmed[1] == 'b' || trimmed[1] == 'B'))
    {
        unsigned long long parsed = 0;
        auto result = std::from_chars(digits + 2, digits_end, parsed, 2);
        if (result.ec == std::errc() && result.ptr == digits_end)
        {
            return static_cast<double>(parsed);
        }
        return std::numeric_limits<double>::quiet_NaN();
    }
    if (trimmed.size() > 2 && trimmed[0] == '0'
        && (trimmed[1] == 'o' || trimmed[1] == 'O'))
    {
        unsigned long long parsed = 0;
        auto result = std::from_chars(digits + 2, digits_end, parsed, 8);
        if (result.ec == std::errc() && result.ptr == digits_end)
        {
            return static_cast<double>(parsed);
        }
        return std::numeric_limits<double>::quiet_NaN();
    }
    double parsed = 0.0;
    auto result = std::from_chars(digits, digits_end, parsed);
    if (result.ec == std::errc() && result.ptr == digits_end)
    {
        return parsed;
    }
    return std::numeric_limits<double>::quiet_NaN();
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const std::string& text) noexcept
{
    return js_cmp_parse_text(std::string_view(text));
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(std::string_view text) noexcept
{
    return js_cmp_parse_text(text);
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const char* text) noexcept
{
    return text != nullptr ? js_cmp_parse_text(std::string_view(text)) : 0.0;
}

template <std::size_t Length>
MORPH_JS_CMP_INLINE double js_cmp_to_number(const char (&text)[Length]) noexcept
{
    return js_cmp_parse_text(js_cmp_to_view(text));
}

"#
}

fn number_helper() -> &'static str {
    r#"template <typename OperandType>
MORPH_JS_CMP_INLINE constexpr double js_cmp_to_number(const OperandType& value) noexcept
    requires JsCmpNumber<OperandType>
{
    return static_cast<double>(value);
}

MORPH_JS_CMP_INLINE constexpr double js_cmp_to_number(bool value) noexcept
{
    return value ? 1.0 : 0.0;
}

"#
}

fn is_truthy_native(include_float: bool, include_text: bool) -> String {
    let mut out = String::from(
        r#"template <typename OperandType>
MORPH_JS_CMP_INLINE constexpr bool is_truthy(const OperandType& value) noexcept
{
    if constexpr (JsCmpBool<OperandType>)
    {
        return value;
    }
    else if constexpr (JsCmpInt<OperandType>)
    {
        return value != 0;
    }
"#,
    );
    if include_float {
        out.push_str(
            r#"    else if constexpr (JsCmpFloat<OperandType>)
    {
        return value != 0.0 && !std::isnan(value);
    }
"#,
        );
    }
    if include_text {
        out.push_str(
            r#"    else if constexpr (JsCmpText<OperandType>)
    {
        return !js_cmp_to_view(value).empty();
    }
"#,
        );
    }
    out.push_str(
        r#"    else
    {
        return static_cast<bool>(value);
    }
}

"#,
    );
    out
}

fn loose_equal_native() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_eq(const LeftType& left_value,
                                            const RightType& right_value) noexcept
{
    if constexpr ((JsCmpText<LeftType> && JsCmpNumber<RightType>)
                  || (JsCmpNumber<LeftType> && JsCmpText<RightType>))
    {
        return js_cmp_to_number(left_value) == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpText<LeftType> && JsCmpText<RightType>)
    {
        return js_cmp_to_view(left_value) == js_cmp_to_view(right_value);
    }
    else if constexpr (JsCmpBool<LeftType> && !JsCmpBool<RightType>)
    {
        return loose_eq(left_value ? 1.0 : 0.0, right_value);
    }
    else if constexpr (!JsCmpBool<LeftType> && JsCmpBool<RightType>)
    {
        return loose_eq(left_value, right_value ? 1.0 : 0.0);
    }
    else
    {
        return left_value == right_value;
    }
}

"#
}

fn strict_equal_native() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool strict_eq(const LeftType& left_value,
                                             const RightType& right_value) noexcept
{
    if constexpr (JsCmpText<LeftType> && JsCmpText<RightType>)
    {
        return js_cmp_to_view(left_value) == js_cmp_to_view(right_value);
    }
    else if constexpr (!std::is_same_v<std::remove_cvref_t<LeftType>,
                                       std::remove_cvref_t<RightType>>)
    {
        if constexpr (JsCmpNumber<LeftType> && JsCmpNumber<RightType>)
        {
            return left_value == right_value;
        }
        else
        {
            return false;
        }
    }
    else
    {
        return left_value == right_value;
    }
}

"#
}

fn relational_native() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_lt(const LeftType& left_value,
                                            const RightType& right_value) noexcept
{
    if constexpr (JsCmpText<LeftType> && JsCmpText<RightType>)
    {
        return js_cmp_to_view(left_value) < js_cmp_to_view(right_value);
    }
    else
    {
        return js_cmp_to_number(left_value) < js_cmp_to_number(right_value);
    }
}

template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_gt(const LeftType& left_value,
                                            const RightType& right_value) noexcept
{
    if constexpr (JsCmpText<LeftType> && JsCmpText<RightType>)
    {
        return js_cmp_to_view(left_value) > js_cmp_to_view(right_value);
    }
    else
    {
        return js_cmp_to_number(left_value) > js_cmp_to_number(right_value);
    }
}

template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_le(const LeftType& left_value,
                                            const RightType& right_value) noexcept
{
    if constexpr (JsCmpText<LeftType> && JsCmpText<RightType>)
    {
        return js_cmp_to_view(left_value) <= js_cmp_to_view(right_value);
    }
    else
    {
        return js_cmp_to_number(left_value) <= js_cmp_to_number(right_value);
    }
}

template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_ge(const LeftType& left_value,
                                            const RightType& right_value) noexcept
{
    if constexpr (JsCmpText<LeftType> && JsCmpText<RightType>)
    {
        return js_cmp_to_view(left_value) >= js_cmp_to_view(right_value);
    }
    else
    {
        return js_cmp_to_number(left_value) >= js_cmp_to_number(right_value);
    }
}

"#
}

fn loose_not_equal_wrapper() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_not_eq(const LeftType& left_value,
                                                const RightType& right_value) noexcept
{
    return !loose_eq(left_value, right_value);
}

"#
}

fn strict_not_equal_wrapper() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool strict_not_eq(const LeftType& left_value,
                                                 const RightType& right_value) noexcept
{
    return !strict_eq(left_value, right_value);
}

"#
}

fn js_helper_forward_declarations() -> &'static str {
    r#"MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsNumber& value) noexcept;
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsBoolean& value) noexcept;
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsNull&) noexcept;
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsUndefined&) noexcept;
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsObject&) noexcept;
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsString& value);
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsArray& value);
MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsValue& value);
MORPH_JS_CMP_INLINE bool js_cmp_to_bool(const JsBoolean& value) noexcept;
MORPH_JS_CMP_INLINE bool js_cmp_to_bool(bool value) noexcept;
inline std::string js_cmp_array_join(const JsArray& array_value);
MORPH_JS_CMP_INLINE std::string_view js_cmp_to_view(const JsString& value) noexcept;

"#
}

fn loose_forward_declarations() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool loose_eq(const LeftType& left_value,
                                            const RightType& right_value) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsNumber& left_value,
                                  const JsNumber& right_value) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsString& left_value,
                                  const JsString& right_value) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsBoolean& left_value,
                                  const JsBoolean& right_value) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsArray& left_value,
                                  const JsArray& right_value) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsObject& left_value,
                                  const JsObject& right_value) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsNull&, const JsNull&) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsUndefined&, const JsUndefined&) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsNull&, const JsUndefined&) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsUndefined&, const JsNull&) noexcept;
MORPH_JS_CMP_INLINE bool loose_eq(const JsValue& left_value, const JsValue& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsNumber& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsNumber& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsString& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsString& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsBoolean& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsBoolean& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsArray& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsArray& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsObject& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsObject& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsNull&, const RightType& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsUndefined&, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpNullish<LeftType> && !JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType&, const JsNull&);
template <typename LeftType>
    requires(!JsCmpNullish<LeftType> && !JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType&, const JsUndefined&);
template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsValue& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsValue& right_value);

"#
}

fn strict_forward_declarations() -> &'static str {
    r#"template <typename LeftType, typename RightType>
MORPH_JS_CMP_INLINE constexpr bool strict_eq(const LeftType& left_value,
                                             const RightType& right_value) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsNumber& left_value,
                                   const JsNumber& right_value) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsString& left_value,
                                   const JsString& right_value) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsBoolean& left_value,
                                   const JsBoolean& right_value) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsArray& left_value,
                                   const JsArray& right_value) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsObject& left_value,
                                   const JsObject& right_value) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsNull&, const JsNull&) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsUndefined&, const JsUndefined&) noexcept;
MORPH_JS_CMP_INLINE bool strict_eq(const JsValue& left_value, const JsValue& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsNumber& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpJsNumber<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsNumber& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsString& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpJsText<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsString& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsBoolean& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpJsBoolean<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsBoolean& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsArray& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsArray& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsObject& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsObject& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsNull&, const RightType& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsUndefined&, const RightType& right_value);
template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsValue& left_value, const RightType& right_value);
template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsValue& right_value);

"#
}

fn js_concepts() -> &'static str {
    r#"template <typename OperandType>
concept JsCmpJsBoolean = std::is_same_v<std::remove_cvref_t<OperandType>, JsBoolean>;

template <typename OperandType>
concept JsCmpJsNumber = std::is_same_v<std::remove_cvref_t<OperandType>, JsNumber>;

template <typename OperandType>
concept JsCmpJsText = std::is_same_v<std::remove_cvref_t<OperandType>, JsString>;

template <typename OperandType>
concept JsCmpJsValue = std::is_same_v<std::remove_cvref_t<OperandType>, JsValue>;

template <typename OperandType>
concept JsCmpJsArray = std::is_same_v<std::remove_cvref_t<OperandType>, JsArray>;

template <typename OperandType>
concept JsCmpJsObject = std::is_same_v<std::remove_cvref_t<OperandType>, JsObject>;

template <typename OperandType>
concept JsCmpJsNull = std::is_same_v<std::remove_cvref_t<OperandType>, JsNull>;

template <typename OperandType>
concept JsCmpJsUndefined = std::is_same_v<std::remove_cvref_t<OperandType>, JsUndefined>;

template <typename OperandType>
concept JsCmpNullish = JsCmpJsNull<OperandType> || JsCmpJsUndefined<OperandType>;

template <typename OperandType>
concept JsCmpAnyJs = JsCmpJsBoolean<OperandType> || JsCmpJsNumber<OperandType>
    || JsCmpJsText<OperandType> || JsCmpJsValue<OperandType> || JsCmpJsArray<OperandType>
    || JsCmpJsObject<OperandType> || JsCmpNullish<OperandType>;

"#
}

fn js_number_text_helpers(include_text: bool) -> String {
    let mut out = String::from(
        r#"MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsNumber& value) noexcept
{
    return value.as_double();
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsBoolean& value) noexcept
{
    return value.value ? 1.0 : 0.0;
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsNull&) noexcept
{
    return 0.0;
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsUndefined&) noexcept
{
    return std::numeric_limits<double>::quiet_NaN();
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsObject&) noexcept
{
    return std::numeric_limits<double>::quiet_NaN();
}

inline std::string js_cmp_array_join(const JsArray& array_value)
{
    std::string joined;
    bool first_item = true;
    for (int64_t index = 0; index < static_cast<int64_t>(array_value.length()); ++index)
    {
        JsValue item = array_value[index];
        if (!first_item)
        {
            joined.push_back(',');
        }
        first_item = false;
        if (item.is_undefined() || item.is_null())
        {
            continue;
        }
        if (item.is_boolean())
        {
            joined += std::get<JsBoolean>(item.inner).value ? "true" : "false";
        }
        else if (item.is_number())
        {
            joined += std::get<JsNumber>(item.inner).as_string();
        }
        else if (item.is_string())
        {
            joined += std::get<JsString>(item.inner).value;
        }
        else if (item.is_array())
        {
            joined += js_cmp_array_join(std::get<JsArray>(item.inner));
        }
        else
        {
            joined += "[object Object]";
        }
    }
    return joined;
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsArray& value)
{
    return js_cmp_to_number(js_cmp_array_join(value));
}

MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsValue& value)
{
    if (value.is_undefined())
    {
        return std::numeric_limits<double>::quiet_NaN();
    }
    if (value.is_null())
    {
        return 0.0;
    }
    if (value.is_boolean())
    {
        return std::get<JsBoolean>(value.inner).value ? 1.0 : 0.0;
    }
    if (value.is_number())
    {
        return std::get<JsNumber>(value.inner).as_double();
    }
    if (value.is_string())
    {
        return js_cmp_to_number(std::get<JsString>(value.inner).value);
    }
    if (value.is_array())
    {
        return js_cmp_to_number(std::get<JsArray>(value.inner));
    }
    return std::numeric_limits<double>::quiet_NaN();
}

MORPH_JS_CMP_INLINE bool js_cmp_to_bool(const JsBoolean& value) noexcept
{
    return value.value;
}

MORPH_JS_CMP_INLINE bool js_cmp_to_bool(bool value) noexcept
{
    return value;
}

inline std::string js_cmp_value_to_primitive_text(const JsValue& value)
{
    if (value.is_string())
    {
        return std::get<JsString>(value.inner).value;
    }
    if (value.is_number())
    {
        return std::get<JsNumber>(value.inner).as_string();
    }
    if (value.is_boolean())
    {
        return std::get<JsBoolean>(value.inner).value ? "true" : "false";
    }
    if (value.is_array())
    {
        return js_cmp_array_join(std::get<JsArray>(value.inner));
    }
    return "[object Object]";
}

inline bool loose_eq_js_values(const JsValue& left_value, const JsValue& right_value)
{
    bool left_nullish = left_value.is_null() || left_value.is_undefined();
    bool right_nullish = right_value.is_null() || right_value.is_undefined();
    if (left_nullish || right_nullish)
    {
        return left_nullish && right_nullish;
    }
    if (left_value.is_array() || left_value.is_object())
    {
        if (left_value.is_array() && right_value.is_array())
        {
            return std::get<JsArray>(left_value.inner).elements
                == std::get<JsArray>(right_value.inner).elements;
        }
        if (left_value.is_object() && right_value.is_object())
        {
            return std::get<JsObject>(left_value.inner).properties
                == std::get<JsObject>(right_value.inner).properties;
        }
        return loose_eq_js_values(
            JsValue(js_cmp_value_to_primitive_text(left_value)), right_value);
    }
    if (right_value.is_array() || right_value.is_object())
    {
        return loose_eq_js_values(
            left_value, JsValue(js_cmp_value_to_primitive_text(right_value)));
    }
    if (left_value.is_boolean() || right_value.is_boolean())
    {
        return js_cmp_to_number(left_value) == js_cmp_to_number(right_value);
    }
    if (left_value.is_number() && right_value.is_number())
    {
        return std::get<JsNumber>(left_value.inner) == std::get<JsNumber>(right_value.inner);
    }
    if (left_value.is_string() && right_value.is_string())
    {
        return std::get<JsString>(left_value.inner) == std::get<JsString>(right_value.inner);
    }
    return js_cmp_to_number(left_value) == js_cmp_to_number(right_value);
}

inline bool strict_eq_js_values(const JsValue& left_value, const JsValue& right_value)
{
    if (left_value.is_number() && right_value.is_number())
    {
        return std::get<JsNumber>(left_value.inner) == std::get<JsNumber>(right_value.inner);
    }
    if (left_value.is_string() && right_value.is_string())
    {
        return std::get<JsString>(left_value.inner) == std::get<JsString>(right_value.inner);
    }
    if (left_value.is_boolean() && right_value.is_boolean())
    {
        return std::get<JsBoolean>(left_value.inner).value
            == std::get<JsBoolean>(right_value.inner).value;
    }
    if (left_value.is_null() && right_value.is_null())
    {
        return true;
    }
    if (left_value.is_undefined() && right_value.is_undefined())
    {
        return true;
    }
    if (left_value.is_array() && right_value.is_array())
    {
        return std::get<JsArray>(left_value.inner).elements
            == std::get<JsArray>(right_value.inner).elements;
    }
    if (left_value.is_object() && right_value.is_object())
    {
        return std::get<JsObject>(left_value.inner).properties
            == std::get<JsObject>(right_value.inner).properties;
    }
    return false;
}

"#,
    );
    if include_text {
        out.push_str(
            r#"MORPH_JS_CMP_INLINE double js_cmp_to_number(const JsString& value)
{
    return js_cmp_to_number(value.value);
}

MORPH_JS_CMP_INLINE std::string_view js_cmp_to_view(const JsString& value) noexcept
{
    return std::string_view(value.value);
}

"#,
        );
    }
    out
}

fn is_truthy_js_overloads() -> &'static str {
    r#"MORPH_JS_CMP_INLINE bool is_truthy(const JsBoolean& value) noexcept
{
    return value.value;
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsNumber& value) noexcept
{
    double numeric = value.as_double();
    return numeric != 0.0 && !std::isnan(numeric);
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsString& value) noexcept
{
    return !value.empty();
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsValue& value) noexcept
{
    return value.truthy();
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsArray& value) noexcept
{
    return value.length() > 0;
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsObject&) noexcept
{
    return true;
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsNull&) noexcept
{
    return false;
}

MORPH_JS_CMP_INLINE bool is_truthy(const JsUndefined&) noexcept
{
    return false;
}

"#
}

fn js_loose_overloads() -> &'static str {
    r#"MORPH_JS_CMP_INLINE bool loose_eq(const JsValue& left_value, const JsValue& right_value)
{
    return loose_eq_js_values(left_value, right_value);
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsNumber& left_value, const JsNumber& right_value) noexcept
{
    return left_value == right_value;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsString& left_value,
                                  const JsString& right_value) noexcept
{
    return left_value == right_value;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsBoolean& left_value,
                                  const JsBoolean& right_value) noexcept
{
    return left_value == right_value;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsArray& left_value, const JsArray& right_value) noexcept
{
    return left_value.elements == right_value.elements;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsObject& left_value,
                                  const JsObject& right_value) noexcept
{
    return left_value.properties == right_value.properties;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsNull&, const JsNull&) noexcept
{
    return true;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsUndefined&, const JsUndefined&) noexcept
{
    return true;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsNull&, const JsUndefined&) noexcept
{
    return true;
}

MORPH_JS_CMP_INLINE bool loose_eq(const JsUndefined&, const JsNull&) noexcept
{
    return true;
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsNumber& left_value, const RightType& right_value)
{
    if constexpr (JsCmpNumber<RightType>)
    {
        return left_value.as_double() == static_cast<double>(right_value);
    }
    else if constexpr (JsCmpBool<RightType>)
    {
        return left_value.as_double() == (right_value ? 1.0 : 0.0);
    }
    else if constexpr (JsCmpText<RightType> || JsCmpJsText<RightType>)
    {
        return left_value.as_double() == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpJsBoolean<RightType>)
    {
        return left_value.as_double() == (right_value.value ? 1.0 : 0.0);
    }
    else if constexpr (JsCmpJsArray<RightType> || JsCmpJsObject<RightType>)
    {
        return left_value.as_double() == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return loose_eq(JsValue(left_value), right_value);
    }
    else if constexpr (JsCmpNullish<RightType>)
    {
        return false;
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsNumber& right_value)
{
    return loose_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsString& left_value, const RightType& right_value)
{
    if constexpr (JsCmpText<RightType> || JsCmpJsText<RightType>)
    {
        return js_cmp_to_view(left_value) == js_cmp_to_view(right_value);
    }
    else if constexpr (JsCmpNumber<RightType> || JsCmpJsNumber<RightType>)
    {
        return js_cmp_to_number(left_value) == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpBool<RightType> || JsCmpJsBoolean<RightType>)
    {
        return js_cmp_to_number(left_value) == js_cmp_to_bool(right_value);
    }
    else if constexpr (JsCmpJsArray<RightType>)
    {
        return js_cmp_to_number(left_value) == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpJsObject<RightType>)
    {
        return js_cmp_to_view(left_value) == std::string_view("[object Object]");
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return loose_eq(JsValue(left_value), right_value);
    }
    else if constexpr (JsCmpNullish<RightType>)
    {
        return false;
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsString& right_value)
{
    return loose_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsBoolean& left_value, const RightType& right_value)
{
    if constexpr (JsCmpBool<RightType> || JsCmpJsBoolean<RightType>)
    {
        return js_cmp_to_bool(left_value) == js_cmp_to_bool(right_value);
    }
    else if constexpr (JsCmpNumber<RightType> || JsCmpJsNumber<RightType>)
    {
        return (left_value.value ? 1.0 : 0.0) == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpText<RightType> || JsCmpJsText<RightType>)
    {
        return (left_value.value ? 1.0 : 0.0) == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpJsArray<RightType> || JsCmpJsObject<RightType>)
    {
        return (left_value.value ? 1.0 : 0.0) == js_cmp_to_number(right_value);
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return loose_eq(JsValue(left_value), right_value);
    }
    else if constexpr (JsCmpNullish<RightType>)
    {
        return false;
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsBoolean& right_value)
{
    return loose_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsArray& left_value, const RightType& right_value)
{
    return loose_eq(js_cmp_array_join(left_value), right_value);
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsArray& right_value)
{
    return loose_eq(left_value, js_cmp_array_join(right_value));
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsObject&, const RightType& right_value)
{
    return loose_eq(JsString("[object Object]"), right_value);
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType&, const JsObject&)
{
    return false;
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsNull&, const RightType& right_value)
{
    if constexpr (JsCmpNullish<RightType>)
    {
        return true;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return right_value.is_null();
    }
    else
    {
        return false;
    }
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsUndefined&, const RightType& right_value)
{
    if constexpr (JsCmpNullish<RightType>)
    {
        return true;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return right_value.is_undefined();
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpNullish<LeftType> && !JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType&, const JsNull&)
{
    return false;
}

template <typename LeftType>
    requires(!JsCmpNullish<LeftType> && !JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType&, const JsUndefined&)
{
    return false;
}

"#
}

fn js_strict_overloads() -> &'static str {
    r#"MORPH_JS_CMP_INLINE bool strict_eq(const JsValue& left_value, const JsValue& right_value)
{
    return strict_eq_js_values(left_value, right_value);
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsNumber& left_value,
                                     const JsNumber& right_value) noexcept
{
    return left_value == right_value;
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsString& left_value,
                                   const JsString& right_value) noexcept
{
    return left_value == right_value;
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsBoolean& left_value,
                                   const JsBoolean& right_value) noexcept
{
    return left_value == right_value;
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsArray& left_value, const JsArray& right_value) noexcept
{
    return left_value.elements == right_value.elements;
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsObject& left_value,
                                   const JsObject& right_value) noexcept
{
    return left_value.properties == right_value.properties;
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsNull&, const JsNull&) noexcept
{
    return true;
}

MORPH_JS_CMP_INLINE bool strict_eq(const JsUndefined&, const JsUndefined&) noexcept
{
    return true;
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsNumber& left_value, const RightType& right_value)
{
    if constexpr (JsCmpNumber<RightType>)
    {
        return left_value.as_double() == static_cast<double>(right_value);
    }
    else if constexpr (JsCmpJsNumber<RightType>)
    {
        return left_value == right_value;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return strict_eq(JsValue(left_value), right_value);
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpJsNumber<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsNumber& right_value)
{
    return strict_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsString& left_value, const RightType& right_value)
{
    if constexpr (JsCmpText<RightType> || JsCmpJsText<RightType>)
    {
        return js_cmp_to_view(left_value) == js_cmp_to_view(right_value);
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return strict_eq(JsValue(left_value), right_value);
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpJsText<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsString& right_value)
{
    return strict_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsBoolean& left_value, const RightType& right_value)
{
    if constexpr (JsCmpBool<RightType> || JsCmpJsBoolean<RightType>)
    {
        return js_cmp_to_bool(left_value) == js_cmp_to_bool(right_value);
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return strict_eq(JsValue(left_value), right_value);
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpJsBoolean<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsBoolean& right_value)
{
    return strict_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsValue& left_value, const RightType& right_value)
{
    if (left_value.is_number())
    {
        return strict_eq(std::get<JsNumber>(left_value.inner), right_value);
    }
    if (left_value.is_string())
    {
        return strict_eq(std::get<JsString>(left_value.inner), right_value);
    }
    if (left_value.is_boolean())
    {
        return strict_eq(std::get<JsBoolean>(left_value.inner), right_value);
    }
    if (left_value.is_null())
    {
        if constexpr (JsCmpJsNull<RightType>)
        {
            return true;
        }
        else
        {
            return false;
        }
    }
    if (left_value.is_undefined())
    {
        if constexpr (JsCmpJsUndefined<RightType>)
        {
            return true;
        }
        else
        {
            return false;
        }
    }
    if (left_value.is_array())
    {
        if constexpr (JsCmpJsArray<RightType>)
        {
            return std::get<JsArray>(left_value.inner).elements == right_value.elements;
        }
        else
        {
            return false;
        }
    }
    if constexpr (JsCmpJsObject<RightType>)
    {
        return left_value.is_object()
            && std::get<JsObject>(left_value.inner).properties == right_value.properties;
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsValue& right_value)
{
    return strict_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsArray& left_value, const RightType& right_value)
{
    if constexpr (JsCmpJsArray<RightType>)
    {
        return left_value.elements == right_value.elements;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return strict_eq(JsValue(left_value), right_value);
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsArray& right_value)
{
    return strict_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsObject& left_value, const RightType& right_value)
{
    if constexpr (JsCmpJsObject<RightType>)
    {
        return left_value.properties == right_value.properties;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return strict_eq(JsValue(left_value), right_value);
    }
    else
    {
        return false;
    }
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool strict_eq(const LeftType& left_value, const JsObject& right_value)
{
    return strict_eq(right_value, left_value);
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsNull&, const RightType& right_value)
{
    if constexpr (JsCmpJsNull<RightType>)
    {
        return true;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return right_value.is_null();
    }
    else
    {
        return false;
    }
}

template <typename RightType>
MORPH_JS_CMP_INLINE bool strict_eq(const JsUndefined&, const RightType& right_value)
{
    if constexpr (JsCmpJsUndefined<RightType>)
    {
        return true;
    }
    else if constexpr (JsCmpJsValue<RightType>)
    {
        return right_value.is_undefined();
    }
    else
    {
        return false;
    }
}

"#
}

fn js_relational_text_overloads() -> &'static str {
    r#"template <typename RightType>
    requires(JsCmpText<RightType>)
MORPH_JS_CMP_INLINE bool loose_lt(const JsString& left_value, const RightType& right_value)
{
    return std::string_view(left_value.value) < js_cmp_to_view(right_value);
}

template <typename RightType>
    requires(JsCmpText<RightType>)
MORPH_JS_CMP_INLINE bool loose_gt(const JsString& left_value, const RightType& right_value)
{
    return std::string_view(left_value.value) > js_cmp_to_view(right_value);
}

template <typename RightType>
    requires(JsCmpText<RightType>)
MORPH_JS_CMP_INLINE bool loose_le(const JsString& left_value, const RightType& right_value)
{
    return std::string_view(left_value.value) <= js_cmp_to_view(right_value);
}

template <typename RightType>
    requires(JsCmpText<RightType>)
MORPH_JS_CMP_INLINE bool loose_ge(const JsString& left_value, const RightType& right_value)
{
    return std::string_view(left_value.value) >= js_cmp_to_view(right_value);
}

template <typename LeftType>
    requires(JsCmpText<LeftType>)
MORPH_JS_CMP_INLINE bool loose_lt(const LeftType& left_value, const JsString& right_value)
{
    return js_cmp_to_view(left_value) < std::string_view(right_value.value);
}

template <typename LeftType>
    requires(JsCmpText<LeftType>)
MORPH_JS_CMP_INLINE bool loose_gt(const LeftType& left_value, const JsString& right_value)
{
    return js_cmp_to_view(left_value) > std::string_view(right_value.value);
}

template <typename LeftType>
    requires(JsCmpText<LeftType>)
MORPH_JS_CMP_INLINE bool loose_le(const LeftType& left_value, const JsString& right_value)
{
    return js_cmp_to_view(left_value) <= std::string_view(right_value.value);
}

template <typename LeftType>
    requires(JsCmpText<LeftType>)
MORPH_JS_CMP_INLINE bool loose_ge(const LeftType& left_value, const JsString& right_value)
{
    return js_cmp_to_view(left_value) >= std::string_view(right_value.value);
}

MORPH_JS_CMP_INLINE bool loose_lt(const JsString& left_value, const JsString& right_value)
{
    return left_value.value < right_value.value;
}

MORPH_JS_CMP_INLINE bool loose_gt(const JsString& left_value, const JsString& right_value)
{
    return left_value.value > right_value.value;
}

MORPH_JS_CMP_INLINE bool loose_le(const JsString& left_value, const JsString& right_value)
{
    return left_value.value <= right_value.value;
}

MORPH_JS_CMP_INLINE bool loose_ge(const JsString& left_value, const JsString& right_value)
{
    return left_value.value >= right_value.value;
}

"#
}

fn js_value_helpers() -> &'static str {
    r#"template <typename RightType>
MORPH_JS_CMP_INLINE bool loose_eq(const JsValue& left_value, const RightType& right_value)
{
    if (left_value.is_undefined() || left_value.is_null())
    {
        if constexpr (JsCmpNullish<RightType>)
        {
            return true;
        }
        else
        {
            return false;
        }
    }
    if (left_value.is_boolean())
    {
        return loose_eq(std::get<JsBoolean>(left_value.inner).value, right_value);
    }
    if (left_value.is_number())
    {
        return loose_eq(std::get<JsNumber>(left_value.inner), right_value);
    }
    if (left_value.is_string())
    {
        return loose_eq(std::get<JsString>(left_value.inner), right_value);
    }
    if (left_value.is_array())
    {
        return loose_eq(js_cmp_array_join(std::get<JsArray>(left_value.inner)), right_value);
    }
    return loose_eq(JsString("[object Object]"), right_value);
}

template <typename LeftType>
    requires(!JsCmpAnyJs<LeftType>)
MORPH_JS_CMP_INLINE bool loose_eq(const LeftType& left_value, const JsValue& right_value)
{
    return loose_eq(right_value, left_value);
}

"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_signatures_produce_no_header() {
        let signatures = HashSet::new();
        let sections = ComparisonSections::from_signatures(&signatures);
        assert!(!sections.needs_helpers());
        assert!(build_header(sections).is_empty());
    }

    #[test]
    fn loose_native_signatures_enable_loose_section_only() {
        let mut signatures = HashSet::new();
        signatures.insert(ComparisonSignature::binary(
            ComparisonKind::LooseEqual,
            OperandClass::Integer,
            OperandClass::Integer,
        ));
        let sections = ComparisonSections::from_signatures(&signatures);
        assert!(sections.loose);
        assert!(!sections.js_types);
        assert!(!sections.text);
        let header = build_header(sections);
        assert!(header.contains("loose_eq"));
        assert!(!header.contains("JsValue"));
    }

    #[test]
    fn text_signatures_enable_text_helpers() {
        let mut signatures = HashSet::new();
        signatures.insert(ComparisonSignature::binary(
            ComparisonKind::LooseEqual,
            OperandClass::Text,
            OperandClass::Integer,
        ));
        let sections = ComparisonSections::from_signatures(&signatures);
        assert!(sections.text);
        let header = build_header(sections);
        assert!(header.contains("js_cmp_parse_text"));
    }

    #[test]
    fn js_signatures_enable_js_overloads() {
        let mut signatures = HashSet::new();
        signatures.insert(ComparisonSignature::binary(
            ComparisonKind::LooseEqual,
            OperandClass::JsNumber,
            OperandClass::Integer,
        ));
        let sections = ComparisonSections::from_signatures(&signatures);
        assert!(sections.js_types);
        let header = build_header(sections);
        assert!(header.contains("JsCmpJsNumber"));
    }

    #[test]
    fn cpp_type_names_map_to_expected_classes() {
        assert_eq!(cpp_type_to_class("bool"), OperandClass::Boolean);
        assert_eq!(cpp_type_to_class("int32_t"), OperandClass::Integer);
        assert_eq!(cpp_type_to_class("int64_t"), OperandClass::Integer);
        assert_eq!(cpp_type_to_class("double"), OperandClass::Float);
        assert_eq!(cpp_type_to_class("std::string"), OperandClass::Text);
        assert_eq!(cpp_type_to_class("JsNumber"), OperandClass::JsNumber);
        assert_eq!(cpp_type_to_class("JsValue"), OperandClass::JsValue);
        assert_eq!(cpp_type_to_class("std::vector<int32_t>"), OperandClass::Vector);
        assert_eq!(cpp_type_to_class("auto"), OperandClass::Other);
    }
}
