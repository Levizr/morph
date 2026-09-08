use crate::codegen::context::Ctx;

/// Maps JavaScript string methods to C++ equivalents for native std::string
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringMethod {
    ToUpperCase,
    ToLowerCase,
    CharAt,
    IndexOf,
    LastIndexOf,
    Substring,
    Substr,
    Slice,
    Trim,
    TrimStart,
    TrimEnd,
    Replace,
    ReplaceAll,
    Split,
    Match,
    MatchAll,
    Search,
    PadStart,
    PadEnd,
    Repeat,
    StartsWith,
    EndsWith,
    Includes,
    LocaleCompare,
    Normalize,
    ToString,
    ToLocaleUpperCase,
    ToLocaleLowerCase,
}

impl StringMethod {
    pub fn from_js_name(name: &str) -> Option<Self> {
        match name {
            "toUpperCase" => Some(Self::ToUpperCase),
            "toLowerCase" => Some(Self::ToLowerCase),
            "charAt" => Some(Self::CharAt),
            "indexOf" => Some(Self::IndexOf),
            "lastIndexOf" => Some(Self::LastIndexOf),
            "substring" => Some(Self::Substring),
            "substr" => Some(Self::Substr),
            "slice" => Some(Self::Slice),
            "trim" => Some(Self::Trim),
            "trimStart" => Some(Self::TrimStart),
            "trimEnd" => Some(Self::TrimEnd),
            "replace" => Some(Self::Replace),
            "replaceAll" => Some(Self::ReplaceAll),
            "split" => Some(Self::Split),
            "match" => Some(Self::Match),
            "matchAll" => Some(Self::MatchAll),
            "search" => Some(Self::Search),
            "padStart" => Some(Self::PadStart),
            "padEnd" => Some(Self::PadEnd),
            "repeat" => Some(Self::Repeat),
            "startsWith" => Some(Self::StartsWith),
            "endsWith" => Some(Self::EndsWith),
            "includes" => Some(Self::Includes),
            "localeCompare" => Some(Self::LocaleCompare),
            "normalize" => Some(Self::Normalize),
            "toLocaleUpperCase" => Some(Self::ToLocaleUpperCase),
            "toLocaleLowerCase" => Some(Self::ToLocaleLowerCase),
            "toString" => Some(Self::ToString),
            _ => None,
        }
    }

    pub fn is_string_method(name: &str) -> bool {
        Self::from_js_name(name).is_some()
    }

    /// Generate C++ code for the method call using morph::str helpers
    pub fn emit_cpp(&self, receiver: &str, args: &[String]) -> String {
        match self {
            Self::ToUpperCase => format!("morph::str::to_upper({})", receiver),
            Self::ToLowerCase => format!("morph::str::to_lower({})", receiver),
            Self::CharAt => {
                let idx = args.first().cloned().unwrap_or("0".to_string());
                format!("morph::str::char_at({}, {})", receiver, idx)
            }
            Self::IndexOf => {
                let search = args.first().cloned().unwrap_or("\"\"".to_string());
                let from = args.get(1).cloned().unwrap_or("0".to_string());
                format!("morph::str::index_of({}, {}, {})", receiver, search, from)
            }
            Self::LastIndexOf => {
                let search = args.first().cloned().unwrap_or("\"\"".to_string());
                let from = args.get(1).cloned().unwrap_or("-1".to_string());
                format!("morph::str::last_index_of({}, {}, {})", receiver, search, from)
            }
            Self::Substring => {
                let start = args.first().cloned().unwrap_or("0".to_string());
                let end = args.get(1).map(|s| s.as_str()).unwrap_or("");
                if end.is_empty() {
                    format!("morph::str::substring({}, {})", receiver, start)
                } else {
                    format!("morph::str::substring({}, {}, {})", receiver, start, end)
                }
            }
            Self::Substr => {
                let start = args.first().cloned().unwrap_or("0".to_string());
                let len = args.get(1).map(|s| s.as_str()).unwrap_or("");
                if len.is_empty() {
                    format!("morph::str::substr({}, {})", receiver, start)
                } else {
                    format!("morph::str::substr({}, {}, {})", receiver, start, len)
                }
            }
            Self::Slice => {
                let start = args.first().cloned().unwrap_or("0".to_string());
                let end = args.get(1).map(|s| s.as_str()).unwrap_or("");
                if end.is_empty() {
                    format!("morph::str::slice({}, {})", receiver, start)
                } else {
                    format!("morph::str::slice({}, {}, {})", receiver, start, end)
                }
            }
            Self::Trim => format!("morph::str::trim({})", receiver),
            Self::TrimStart => format!("morph::str::trim_start({})", receiver),
            Self::TrimEnd => format!("morph::str::trim_end({})", receiver),
            Self::Replace => {
                let search = args.first().cloned().unwrap_or("\"\"".to_string());
                let replace = args.get(1).cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::replace({}, {}, {})", receiver, search, replace)
            }
            Self::ReplaceAll => {
                let search = args.first().cloned().unwrap_or("\"\"".to_string());
                let replace = args.get(1).cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::replace_all({}, {}, {})", receiver, search, replace)
            }
            Self::Split => {
                let sep = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::split({}, {})", receiver, sep)
            }
            Self::Match => {
                let regex = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::match_regex({}, {})", receiver, regex)
            }
            Self::MatchAll => {
                let regex = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::match_all({}, {})", receiver, regex)
            }
            Self::Search => {
                let regex = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::search({}, {})", receiver, regex)
            }
            Self::PadStart => {
                let len = args.first().cloned().unwrap_or("0".to_string());
                let pad = args.get(1).cloned().unwrap_or("\" \"".to_string());
                format!("morph::str::pad_start({}, {}, {})", receiver, len, pad)
            }
            Self::PadEnd => {
                let len = args.first().cloned().unwrap_or("0".to_string());
                let pad = args.get(1).cloned().unwrap_or("\" \"".to_string());
                format!("morph::str::pad_end({}, {}, {})", receiver, len, pad)
            }
            Self::Repeat => {
                let count = args.first().cloned().unwrap_or("0".to_string());
                format!("morph::str::repeat({}, {})", receiver, count)
            }
            Self::StartsWith => {
                let prefix = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::starts_with({}, {})", receiver, prefix)
            }
            Self::EndsWith => {
                let suffix = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::ends_with({}, {})", receiver, suffix)
            }
            Self::Includes => {
                let substr = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::includes({}, {})", receiver, substr)
            }
            Self::LocaleCompare => {
                let other = args.first().cloned().unwrap_or("\"\"".to_string());
                format!("morph::str::locale_compare({}, {})", receiver, other)
            }
            Self::Normalize => format!("morph::str::normalize({})", receiver),
            Self::ToLocaleUpperCase => format!("morph::str::to_locale_upper({})", receiver),
            Self::ToLocaleLowerCase => format!("morph::str::to_locale_lower({})", receiver),
            Self::ToString => format!("morph::str::to_string({})", receiver),
        }
    }
}

/// Handler for translating string methods on native std::string types
pub struct StringMethodHandler;

impl StringMethodHandler {
    /// Check if a method name is a known string method
    pub fn is_string_method(name: &str) -> bool {
        StringMethod::is_string_method(name)
    }

    /// Translate a string method call to C++
    pub fn translate_method(
        ctx: &mut Ctx,
        receiver: &str,
        method: &str,
        args: &[String], // Pre-emitted argument strings
        is_jsstring: bool,
    ) -> String {
        // For JsString, use the existing JsString methods
        if is_jsstring {
            return format!("{}.{}({})", receiver, method, args.join(", "));
        }

        // For native std::string, use morph::str helpers
        if let Some(sm) = StringMethod::from_js_name(method) {
            // Add the helper header
            ctx.needed.insert("\"../../runtime/cpp/types/js_string_helpers.h\"".to_string());
            // Also need js_types.h for JsValue and other types
            ctx.needed.insert("\"../../runtime/cpp/types/js_types.h\"".to_string());

            sm.emit_cpp(receiver, args)
        } else {
            // Unknown method, fallback to standard call
            format!("{}.{}({})", receiver, method, args.join(", "))
        }
    }
}

/// Tracks string method usage to decide between inline vs reusable function
#[derive(Debug, Default)]
pub struct StringMethodTracker {
    /// method_name -> (count, receiver_type)
    usage: std::collections::HashMap<String, (usize, String)>,
}

impl StringMethodTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_usage(&mut self, method: &str, receiver_type: &str) {
        let entry = self.usage.entry(method.to_string()).or_insert((0, receiver_type.to_string()));
        entry.0 += 1;
    }

    pub fn should_inline(&self, method: &str) -> bool {
        if let Some((count, _)) = self.usage.get(method) { *count <= 1 } else { true }
    }

    pub fn get_receiver_type(&self, method: &str) -> Option<&str> {
        self.usage.get(method).map(|(_, t)| t.as_str())
    }
}
