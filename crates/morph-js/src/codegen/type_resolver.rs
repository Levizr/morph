use oxc_ast::ast::*;

use std::collections::HashSet;

// Maps from codegen.py
const CPP_NATIVE_TYPES: &[(&str, &str)] = &[
    ("int", "int"),
    ("int32", "int32_t"),
    ("int64", "int64_t"),
    ("uint", "unsigned int"),
    ("uint32", "uint32_t"),
    ("uint64", "uint64_t"),
    ("float", "float"),
    ("double", "double"),
    ("bool", "bool"),
    ("char", "char"),
    ("size_t", "size_t"),
    ("byte", "uint8_t"),
];

const PRIMITIVE_MAP: &[(&str, &str)] = &[
    ("string", "JsString"),
    ("number", "JsNumber"),
    ("boolean", "JsBoolean"),
    ("void", "void"),
    ("any", "JsValue"),
    ("never", "void"),
    ("bigint", "int64_t"),
    ("symbol", "void*"),
    ("object", "JsObject"),
    ("undefined", "JsUndefined"),
    ("null", "JsNull"),
];

const CONTEXTUAL_TYPES: &[(&str, &str)] = &[
    ("Number", "JsNumber"),
    ("String", "JsString"),
    ("Boolean", "JsBoolean"),
    ("HTMLElement", "MorphNode*"),
    ("Element", "MorphNode*"),
    ("Node", "MorphNode*"),
    ("MouseEvent", "MorphEvent*"),
    ("Event", "MorphEvent*"),
    ("KeyboardEvent", "MorphEvent*"),
    ("FocusEvent", "MorphEvent*"),
    ("Promise", "auto"),
];

fn lookup(m: &[(&str, &str)], k: &str) -> Option<String> {
    m.iter().find(|(a, _)| *a == k).map(|(_, v)| v.to_string())
}

fn is_plain_cpp_type(name: &str) -> bool {
    // Mirrors Python: name.isidentifier() or startswith std::
    if name.starts_with("std::") {
        return true;
    }
    // identifier check: ascii alphanum + _
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        if !(first.is_ascii_alphabetic() || first == '_') {
            return false;
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '_') {
                return false;
            }
        }
        return true;
    }
    false
}

pub fn ts_type_name_to_string(name: &TSTypeName) -> String {
    match name {
        TSTypeName::IdentifierReference(id) => id.name.to_string(),
        TSTypeName::QualifiedName(q) => format!(
            "{}.{}",
            ts_type_name_to_string(&q.left),
            q.right.name
        ),
        TSTypeName::ThisExpression(_) => "this".to_string(),
    }
}

pub fn resolve_type<'a>(
    ty: Option<&TSType<'a>>,
    context_hint: &str,
    template_params: &HashSet<String>,
    wrap_shared: bool,
    class_names: &HashSet<String>,
) -> String {
    let Some(ty) = ty else {
        return context_hint.to_string();
    };
    match ty {
        TSType::TSAnyKeyword(_)
        | TSType::TSUnknownKeyword(_)
        | TSType::TSStringKeyword(_)
        | TSType::TSNumberKeyword(_)
        | TSType::TSBooleanKeyword(_)
        | TSType::TSVoidKeyword(_)
        | TSType::TSNullKeyword(_)
        | TSType::TSUndefinedKeyword(_)
        | TSType::TSNeverKeyword(_)
        | TSType::TSObjectKeyword(_)
        | TSType::TSBigIntKeyword(_)
        | TSType::TSSymbolKeyword(_)
        | TSType::TSIntrinsicKeyword(_)
        | TSType::TSThisType(_)
        | TSType::TSConstructorType(_)
        | TSType::TSFunctionType(_)
        | TSType::TSTypePredicate(_)
        | TSType::TSTypeQuery(_)
        | TSType::TSImportType(_)
        | TSType::TSConditionalType(_)
        | TSType::TSTypeOperatorType(_)
        | TSType::TSIndexedAccessType(_)
        | TSType::TSInferType(_)
        | TSType::TSTemplateLiteralType(_)
        | TSType::TSLiteralType(_)
        | TSType::TSMappedType(_)
        | TSType::JSDocNullableType(_)
        | TSType::JSDocNonNullableType(_)
        | TSType::JSDocUnknownType(_) => {
            // Map keyword to string via raw name
            // Use span-less fallback: extract raw via debug? Better use string representation
            // For keywords, we can match directly
            let raw = match ty {
                TSType::TSAnyKeyword(_) => "any",
                TSType::TSUnknownKeyword(_) => "unknown",
                TSType::TSStringKeyword(_) => "string",
                TSType::TSNumberKeyword(_) => "number",
                TSType::TSBooleanKeyword(_) => "boolean",
                TSType::TSVoidKeyword(_) => "void",
                TSType::TSNullKeyword(_) => "null",
                TSType::TSUndefinedKeyword(_) => "undefined",
                TSType::TSNeverKeyword(_) => "never",
                TSType::TSObjectKeyword(_) => "object",
                TSType::TSBigIntKeyword(_) => "bigint",
                TSType::TSSymbolKeyword(_) => "symbol",
                _ => "any",
            };
            if let Some(n) = lookup(CPP_NATIVE_TYPES, raw) {
                return n;
            }
            if let Some(m) = lookup(PRIMITIVE_MAP, raw) {
                return m;
            }
            if let Some(m) = lookup(CONTEXTUAL_TYPES, raw) {
                return m;
            }
            if wrap_shared && (class_names.contains(raw) || template_params.contains(raw)) {
                return format!("std::shared_ptr<{}>", raw);
            }
            raw.to_string()
        }
        TSType::TSArrayType(arr) => {
            let elem = resolve_type(Some(&arr.element_type), "auto", template_params, true, class_names);
            if elem.starts_with("Js") || !is_plain_cpp_type(&elem) {
                return "JsArray".to_string();
            }
            format!("std::vector<{}>", elem)
        }
        TSType::TSTypeReference(r) => {
            let name = ts_type_name_to_string(&r.type_name);
            let type_args: Vec<String> = r
                .type_arguments
                .as_ref()
                .map(|ta| {
                    ta.params
                        .iter()
                        .map(|p| resolve_type(Some(p), "auto", template_params, wrap_shared, class_names))
                        .collect()
                })
                .unwrap_or_default();
            if type_args.is_empty() {
                let raw = name.as_str();
                if let Some(n) = lookup(CPP_NATIVE_TYPES, raw) {
                    return n;
                }
                if let Some(m) = lookup(PRIMITIVE_MAP, raw) {
                    return m;
                }
                if let Some(m) = lookup(CONTEXTUAL_TYPES, raw) {
                    return m;
                }
                if wrap_shared && (class_names.contains(raw) || template_params.contains(raw)) {
                    return format!("std::shared_ptr<{}>", raw);
                }
                return raw.to_string();
            }
            // Generic handling
            if name == "Array" || name == "ReadonlyArray" {
                let elem = type_args.first().cloned().unwrap_or_else(|| "auto".to_string());
                // Need wrap_shared for element
                let elem_resolved = if elem == "auto" { "auto".to_string() } else { elem };
                // replicate Python: check if elem is Js or not plain
                let check = resolve_type(
                    r.type_arguments.as_ref().and_then(|ta| ta.params.first()).map(|p| p as &TSType),
                    "auto",
                    template_params,
                    true,
                    class_names,
                );
                if check.starts_with("Js") || !is_plain_cpp_type(&check) {
                    return "JsArray".to_string();
                }
                return format!("std::vector<{}>", elem_resolved);
            }
            if name == "Promise" || name == "Readonly" {
                return type_args.first().cloned().unwrap_or_else(|| "auto".to_string());
            }
            if name == "Record" {
                return "std::unordered_map<std::string, auto>".to_string();
            }
            if name == "Partial" {
                return "auto".to_string();
            }
            // Generic fallback: keep name
            if type_args.is_empty() {
                name
            } else {
                format!("{}<{}>", name, type_args.join(", "))
            }
        }
        TSType::TSUnionType(u) => {
            // Python returns "auto" for union
            if u.types.is_empty() {
                return "auto".to_string();
            }
            // Could try to be smarter but match Python
            "auto".to_string()
        }
        TSType::TSIntersectionType(_) => "auto".to_string(),
        TSType::TSTupleType(_) => "JsArray".to_string(),
        TSType::TSTypeLiteral(_) => "JsObject".to_string(),
        TSType::TSParenthesizedType(p) => resolve_type(Some(&p.type_annotation), context_hint, template_params, wrap_shared, class_names),
        _ => {
            // For any other complex type, fallback to auto
            "auto".to_string()
        }
    }
}

pub fn resolve_type_annotation<'a>(
    ann: Option<&TSTypeAnnotation<'a>>,
    hint: &str,
    template_params: &HashSet<String>,
    wrap_shared: bool,
    class_names: &HashSet<String>,
) -> String {
    ann.map(|a| resolve_type(Some(&a.type_annotation), hint, template_params, wrap_shared, class_names))
        .unwrap_or_else(|| hint.to_string())
}

pub fn headers_for(cpp_type: &str) -> Vec<&'static str> {
    // Mirrors _TYPE_TO_HEADER in python
    const MAP: &[(&str, &str)] = &[ 
        ("std::string", "<string>"),
        ("std::string_view", "<string_view>"),
        ("std::vector", "<vector>"),
        ("std::optional", "<optional>"),
        ("std::variant", "<variant>"),
        ("std::format", "<format>"),
        ("std::shared_ptr", "<memory>"),
        ("std::make_shared", "<memory>"),
        ("JsNumber", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsString", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsBoolean", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsArray", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsObject", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsValue", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsUndefined", "\"../../runtime/cpp/types/js_types.h\""),
        ("JsNull", "\"../../runtime/cpp/types/js_types.h\""),
        ("morph::Result", "\"../../runtime/cpp/reactivity/promise.h\""),
        ("morph::Task", "\"../../runtime/cpp/reactivity/task.h\""),
        ("morph::net", "\"../../runtime/cpp/net/net.h\""),
        ("morph::net::Response", "\"../../runtime/cpp/net/net.h\""),
    ];
    let mut out = Vec::new();
    for (prefix, header) in MAP {
        if cpp_type.contains(prefix) {
            out.push(*header);
        }
    }
    out
}

pub fn is_string_type(cpp_type: &str) -> bool {
    let base = cpp_type
        .trim_start_matches("const ")
        .trim_end_matches('&')
        .trim()
        .to_string();
    matches!(
        base.as_str(),
        "JsString" | "std::string" | "std::string_view" | "const char*"
    ) || base == "std::string"
}

pub fn param_type(cpp_type: &str) -> String {
    if cpp_type == "JsString" || cpp_type == "const JsString" {
        return "const JsString&".to_string();
    }
    if cpp_type == "std::string" || cpp_type == "const std::string" {
        return "std::string_view".to_string();
    }
    if is_string_type(cpp_type) {
        return format!("const {}&", cpp_type);
    }
    cpp_type.to_string()
}
