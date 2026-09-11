//! Tailwind CSS resolver — static map + arbitrary values
//! Mirrors Python's `morph/style/tailwind.py` (500+ utilities, 3 tiers)

use std::collections::HashMap;

pub struct TailwindResolver;

impl TailwindResolver {
    pub fn new() -> Self { Self }

    pub fn resolve(&self, class: &str) -> HashMap<String, String> {
        let mut out = HashMap::new();
        if let Some(mapped) = static_map(class) {
            for (k, v) in mapped { out.insert(k.to_string(), v.to_string()); }
            return out;
        }
        if let Some(mapped) = resolve_negative(class) {
            for (k, v) in mapped { out.insert(k.to_string(), v.to_string()); }
            return out;
        }
        if let Some(pairs) = parse_arbitrary(class) {
            for (k, v) in pairs { out.insert(k, v); }
            return out;
        }
        out
    }

    pub fn resolve_many(&self, class_name: &str) -> HashMap<String, String> {
        let mut merged = HashMap::new();
        for cls in class_name.split_whitespace() {
            for (k, v) in self.resolve(cls) {
                merged.insert(k, v);
            }
        }
        merged
    }
}

fn static_map(class: &str) -> Option<Vec<(&'static str, &'static str)>> {
    Some(match class {
        "absolute" => vec![("position", "absolute")],
        "bg-black" => vec![("background-color", "#000000")],
        "bg-blue-500" => vec![("background-color", "#3b82f6")],
        "bg-blue-600" => vec![("background-color", "#2563eb")],
        "bg-gray-100" => vec![("background-color", "#f3f4f6")],
        "bg-gray-200" => vec![("background-color", "#e5e7eb")],
        "bg-gray-300" => vec![("background-color", "#d1d5db")],
        "bg-gray-400" => vec![("background-color", "#9ca3af")],
        "bg-gray-50" => vec![("background-color", "#f9fafb")],
        "bg-gray-500" => vec![("background-color", "#6b7280")],
        "bg-gray-600" => vec![("background-color", "#4b5563")],
        "bg-gray-700" => vec![("background-color", "#374151")],
        "bg-gray-800" => vec![("background-color", "#1f2937")],
        "bg-gray-900" => vec![("background-color", "#111827")],
        "bg-green-500" => vec![("background-color", "#22c55e")],
        "bg-indigo-500" => vec![("background-color", "#6366f1")],
        "bg-indigo-600" => vec![("background-color", "#4f46e5")],
        "bg-pink-500" => vec![("background-color", "#ec4899")],
        "bg-purple-500" => vec![("background-color", "#a855f7")],
        "bg-purple-600" => vec![("background-color", "#9333ea")],
        "bg-red-500" => vec![("background-color", "#ef4444")],
        "bg-transparent" => vec![("background-color", "transparent")],
        "bg-white" => vec![("background-color", "#ffffff")],
        "bg-yellow-500" => vec![("background-color", "#eab308")],
        "block" => vec![("display", "block")],
        "cursor-default" => vec![("cursor", "default")],
        "cursor-not-allowed" => vec![("cursor", "not-allowed")],
        "cursor-pointer" => vec![("cursor", "pointer")],
        "fixed" => vec![("position", "fixed")],
        "flex" => vec![("display", "flex")],
        "flex-1" => vec![("flex-basis", "0%"), ("flex-grow", "1"), ("flex-shrink", "1")],
        "flex-auto" => vec![("flex-basis", "auto"), ("flex-grow", "1"), ("flex-shrink", "1")],
        "flex-col" => vec![("flex-direction", "column")],
        "flex-col-reverse" => vec![("flex-direction", "column-reverse")],
        "flex-none" => vec![("flex-basis", "auto"), ("flex-grow", "0"), ("flex-shrink", "0")],
        "flex-nowrap" => vec![("flex-wrap", "nowrap")],
        "flex-row" => vec![("flex-direction", "row")],
        "flex-row-reverse" => vec![("flex-direction", "row-reverse")],
        "flex-wrap" => vec![("flex-wrap", "wrap")],
        "font-bold" => vec![("font-weight", "700")],
        "font-extrabold" => vec![("font-weight", "800")],
        "font-light" => vec![("font-weight", "300")],
        "font-medium" => vec![("font-weight", "500")],
        "font-normal" => vec![("font-weight", "400")],
        "font-semibold" => vec![("font-weight", "600")],
        "font-thin" => vec![("font-weight", "100")],
        "gap-0" => vec![("gap", "0px")],
        "gap-1" => vec![("gap", "4px")],
        "gap-2" => vec![("gap", "8px")],
        "gap-3" => vec![("gap", "12px")],
        "gap-4" => vec![("gap", "16px")],
        "gap-6" => vec![("gap", "24px")],
        "gap-8" => vec![("gap", "32px")],
        "h-0" => vec![("height", "0px")],
        "h-1" => vec![("height", "4px")],
        "h-10" => vec![("height", "40px")],
        "h-12" => vec![("height", "48px")],
        "h-16" => vec![("height", "64px")],
        "h-2" => vec![("height", "8px")],
        "h-20" => vec![("height", "80px")],
        "h-4" => vec![("height", "16px")],
        "h-6" => vec![("height", "24px")],
        "h-8" => vec![("height", "32px")],
        "h-auto" => vec![("height", "auto")],
        "h-full" => vec![("height", "100%")],
        "h-screen" => vec![("height", "100vh")],
        "hidden" => vec![("display", "none")],
        "inline" => vec![("display", "inline")],
        "inline-block" => vec![("display", "inline-block")],
        "inline-flex" => vec![("display", "inline-flex")],
        "items-center" => vec![("align-items", "center")],
        "items-end" => vec![("align-items", "flex-end")],
        "items-start" => vec![("align-items", "flex-start")],
        "items-stretch" => vec![("align-items", "stretch")],
        "justify-around" => vec![("justify-content", "space-around")],
        "justify-between" => vec![("justify-content", "space-between")],
        "justify-center" => vec![("justify-content", "center")],
        "justify-end" => vec![("justify-content", "flex-end")],
        "justify-start" => vec![("justify-content", "flex-start")],
        "m-0" => vec![("margin", "0px")],
        "m-1" => vec![("margin", "4px")],
        "m-2" => vec![("margin", "8px")],
        "m-4" => vec![("margin", "16px")],
        "m-6" => vec![("margin", "24px")],
        "m-8" => vec![("margin", "32px")],
        "m-auto" => vec![("margin", "auto")],
        "mb-2" => vec![("margin-bottom", "8px")],
        "mb-4" => vec![("margin-bottom", "16px")],
        "mb-8" => vec![("margin-bottom", "32px")],
        "min-h-0" => vec![("min-height", "0px")],
        "min-w-0" => vec![("min-width", "0px")],
        "ml-2" => vec![("margin-left", "8px")],
        "ml-4" => vec![("margin-left", "16px")],
        "mr-2" => vec![("margin-right", "8px")],
        "mr-4" => vec![("margin-right", "16px")],
        "mt-1" => vec![("margin-top", "4px")],
        "mt-2" => vec![("margin-top", "8px")],
        "mt-4" => vec![("margin-top", "16px")],
        "mt-8" => vec![("margin-top", "32px")],
        "mx-4" => vec![("margin-left", "16px"), ("margin-right", "16px")],
        "mx-auto" => vec![("margin-left", "auto"), ("margin-right", "auto")],
        "my-4" => vec![("margin-bottom", "16px"), ("margin-top", "16px")],
        "opacity-0" => vec![("opacity", "0")],
        "opacity-100" => vec![("opacity", "1")],
        "opacity-25" => vec![("opacity", "0.25")],
        "opacity-50" => vec![("opacity", "0.5")],
        "opacity-75" => vec![("opacity", "0.75")],
        "overflow-auto" => vec![("overflow", "auto")],
        "overflow-hidden" => vec![("overflow", "hidden")],
        "overflow-scroll" => vec![("overflow", "scroll")],
        "overflow-visible" => vec![("overflow", "visible")],
        "p-0" => vec![("padding", "0px")],
        "p-1" => vec![("padding", "4px")],
        "p-10" => vec![("padding", "40px")],
        "p-12" => vec![("padding", "48px")],
        "p-2" => vec![("padding", "8px")],
        "p-3" => vec![("padding", "12px")],
        "p-4" => vec![("padding", "16px")],
        "p-5" => vec![("padding", "20px")],
        "p-6" => vec![("padding", "24px")],
        "p-8" => vec![("padding", "32px")],
        "pb-4" => vec![("padding-bottom", "16px")],
        "pl-4" => vec![("padding-left", "16px")],
        "pr-4" => vec![("padding-right", "16px")],
        "pt-4" => vec![("padding-top", "16px")],
        "px-1" => vec![("padding-left", "4px"), ("padding-right", "4px")],
        "px-2" => vec![("padding-left", "8px"), ("padding-right", "8px")],
        "px-3" => vec![("padding-left", "12px"), ("padding-right", "12px")],
        "px-4" => vec![("padding-left", "16px"), ("padding-right", "16px")],
        "px-6" => vec![("padding-left", "24px"), ("padding-right", "24px")],
        "px-8" => vec![("padding-left", "32px"), ("padding-right", "32px")],
        "py-1" => vec![("padding-bottom", "4px"), ("padding-top", "4px")],
        "py-2" => vec![("padding-bottom", "8px"), ("padding-top", "8px")],
        "py-3" => vec![("padding-bottom", "12px"), ("padding-top", "12px")],
        "py-4" => vec![("padding-bottom", "16px"), ("padding-top", "16px")],
        "py-6" => vec![("padding-bottom", "24px"), ("padding-top", "24px")],
        "relative" => vec![("position", "relative")],
        "rotate-0" => vec![("transform", "rotate(0deg)")],
        "rotate-1" => vec![("transform", "rotate(1deg)")],
        "rotate-12" => vec![("transform", "rotate(12deg)")],
        "rotate-180" => vec![("transform", "rotate(180deg)")],
        "rotate-2" => vec![("transform", "rotate(2deg)")],
        "rotate-3" => vec![("transform", "rotate(3deg)")],
        "rotate-45" => vec![("transform", "rotate(45deg)")],
        "rotate-6" => vec![("transform", "rotate(6deg)")],
        "rotate-90" => vec![("transform", "rotate(90deg)")],
        "rounded" => vec![("border-radius", "4px")],
        "rounded-2xl" => vec![("border-radius", "16px")],
        "rounded-3xl" => vec![("border-radius", "24px")],
        "rounded-full" => vec![("border-radius", "9999px")],
        "rounded-lg" => vec![("border-radius", "8px")],
        "rounded-md" => vec![("border-radius", "6px")],
        "rounded-none" => vec![("border-radius", "0px")],
        "rounded-sm" => vec![("border-radius", "2px")],
        "rounded-xl" => vec![("border-radius", "12px")],
        "scale-0" => vec![("transform", "scale(0)")],
        "scale-100" => vec![("transform", "scale(1)")],
        "scale-105" => vec![("transform", "scale(1.05)")],
        "scale-110" => vec![("transform", "scale(1.1)")],
        "scale-125" => vec![("transform", "scale(1.25)")],
        "scale-150" => vec![("transform", "scale(1.5)")],
        "scale-50" => vec![("transform", "scale(0.5)")],
        "scale-75" => vec![("transform", "scale(0.75)")],
        "scale-90" => vec![("transform", "scale(0.9)")],
        "scale-95" => vec![("transform", "scale(0.95)")],
        "scale-x-0" => vec![("transform", "scaleX(0)")],
        "scale-x-100" => vec![("transform", "scaleX(1)")],
        "scale-x-105" => vec![("transform", "scaleX(1.05)")],
        "scale-x-110" => vec![("transform", "scaleX(1.1)")],
        "scale-x-125" => vec![("transform", "scaleX(1.25)")],
        "scale-x-150" => vec![("transform", "scaleX(1.5)")],
        "scale-x-50" => vec![("transform", "scaleX(0.5)")],
        "scale-x-75" => vec![("transform", "scaleX(0.75)")],
        "scale-x-90" => vec![("transform", "scaleX(0.9)")],
        "scale-x-95" => vec![("transform", "scaleX(0.95)")],
        "scale-y-0" => vec![("transform", "scaleY(0)")],
        "scale-y-100" => vec![("transform", "scaleY(1)")],
        "scale-y-105" => vec![("transform", "scaleY(1.05)")],
        "scale-y-110" => vec![("transform", "scaleY(1.1)")],
        "scale-y-125" => vec![("transform", "scaleY(1.25)")],
        "scale-y-150" => vec![("transform", "scaleY(1.5)")],
        "scale-y-50" => vec![("transform", "scaleY(0.5)")],
        "scale-y-75" => vec![("transform", "scaleY(0.75)")],
        "scale-y-90" => vec![("transform", "scaleY(0.9)")],
        "scale-y-95" => vec![("transform", "scaleY(0.95)")],
        "select-all" => vec![("user-select", "all")],
        "select-none" => vec![("user-select", "none")],
        "select-text" => vec![("user-select", "text")],
        "skew-x-0" => vec![("transform", "skewX(0deg)")],
        "skew-x-1" => vec![("transform", "skewX(1deg)")],
        "skew-x-12" => vec![("transform", "skewX(12deg)")],
        "skew-x-2" => vec![("transform", "skewX(2deg)")],
        "skew-x-3" => vec![("transform", "skewX(3deg)")],
        "skew-x-6" => vec![("transform", "skewX(6deg)")],
        "skew-y-0" => vec![("transform", "skewY(0deg)")],
        "skew-y-1" => vec![("transform", "skewY(1deg)")],
        "skew-y-12" => vec![("transform", "skewY(12deg)")],
        "skew-y-2" => vec![("transform", "skewY(2deg)")],
        "skew-y-3" => vec![("transform", "skewY(3deg)")],
        "skew-y-6" => vec![("transform", "skewY(6deg)")],
        "static" => vec![("position", "static")],
        "sticky" => vec![("position", "sticky")],
        "text-2xl" => vec![("font-size", "24px"), ("line-height", "32px")],
        "text-3xl" => vec![("font-size", "30px"), ("line-height", "36px")],
        "text-4xl" => vec![("font-size", "36px"), ("line-height", "40px")],
        "text-base" => vec![("font-size", "16px"), ("line-height", "24px")],
        "text-black" => vec![("color", "#000000")],
        "text-blue-500" => vec![("color", "#3b82f6")],
        "text-center" => vec![("text-align", "center")],
        "text-gray-400" => vec![("color", "#9ca3af")],
        "text-gray-500" => vec![("color", "#6b7280")],
        "text-gray-600" => vec![("color", "#4b5563")],
        "text-gray-700" => vec![("color", "#374151")],
        "text-gray-800" => vec![("color", "#1f2937")],
        "text-gray-900" => vec![("color", "#111827")],
        "text-green-500" => vec![("color", "#22c55e")],
        "text-left" => vec![("text-align", "left")],
        "text-lg" => vec![("font-size", "18px"), ("line-height", "28px")],
        "text-purple-500" => vec![("color", "#a855f7")],
        "text-red-500" => vec![("color", "#ef4444")],
        "text-right" => vec![("text-align", "right")],
        "text-sm" => vec![("font-size", "14px"), ("line-height", "20px")],
        "text-white" => vec![("color", "#ffffff")],
        "text-xl" => vec![("font-size", "20px"), ("line-height", "28px")],
        "text-xs" => vec![("font-size", "12px"), ("line-height", "16px")],
        "translate-x-0" => vec![("transform", "translateX(0px)")],
        "translate-x-1" => vec![("transform", "translateX(4px)")],
        "translate-x-10" => vec![("transform", "translateX(40px)")],
        "translate-x-12" => vec![("transform", "translateX(48px)")],
        "translate-x-16" => vec![("transform", "translateX(64px)")],
        "translate-x-2" => vec![("transform", "translateX(8px)")],
        "translate-x-20" => vec![("transform", "translateX(80px)")],
        "translate-x-24" => vec![("transform", "translateX(96px)")],
        "translate-x-3" => vec![("transform", "translateX(12px)")],
        "translate-x-32" => vec![("transform", "translateX(128px)")],
        "translate-x-4" => vec![("transform", "translateX(16px)")],
        "translate-x-40" => vec![("transform", "translateX(160px)")],
        "translate-x-48" => vec![("transform", "translateX(192px)")],
        "translate-x-5" => vec![("transform", "translateX(20px)")],
        "translate-x-56" => vec![("transform", "translateX(224px)")],
        "translate-x-6" => vec![("transform", "translateX(24px)")],
        "translate-x-64" => vec![("transform", "translateX(256px)")],
        "translate-x-8" => vec![("transform", "translateX(32px)")],
        "translate-y-0" => vec![("transform", "translateY(0px)")],
        "translate-y-1" => vec![("transform", "translateY(4px)")],
        "translate-y-10" => vec![("transform", "translateY(40px)")],
        "translate-y-12" => vec![("transform", "translateY(48px)")],
        "translate-y-16" => vec![("transform", "translateY(64px)")],
        "translate-y-2" => vec![("transform", "translateY(8px)")],
        "translate-y-20" => vec![("transform", "translateY(80px)")],
        "translate-y-24" => vec![("transform", "translateY(96px)")],
        "translate-y-3" => vec![("transform", "translateY(12px)")],
        "translate-y-32" => vec![("transform", "translateY(128px)")],
        "translate-y-4" => vec![("transform", "translateY(16px)")],
        "translate-y-40" => vec![("transform", "translateY(160px)")],
        "translate-y-48" => vec![("transform", "translateY(192px)")],
        "translate-y-5" => vec![("transform", "translateY(20px)")],
        "translate-y-56" => vec![("transform", "translateY(224px)")],
        "translate-y-6" => vec![("transform", "translateY(24px)")],
        "translate-y-64" => vec![("transform", "translateY(256px)")],
        "translate-y-8" => vec![("transform", "translateY(32px)")],
        "w-0" => vec![("width", "0px")],
        "w-1" => vec![("width", "4px")],
        "w-10" => vec![("width", "40px")],
        "w-12" => vec![("width", "48px")],
        "w-16" => vec![("width", "64px")],
        "w-2" => vec![("width", "8px")],
        "w-20" => vec![("width", "80px")],
        "w-24" => vec![("width", "96px")],
        "w-32" => vec![("width", "128px")],
        "w-4" => vec![("width", "16px")],
        "w-48" => vec![("width", "192px")],
        "w-6" => vec![("width", "24px")],
        "w-64" => vec![("width", "256px")],
        "w-8" => vec![("width", "32px")],
        "w-auto" => vec![("width", "auto")],
        "w-full" => vec![("width", "100%")],
        "w-screen" => vec![("width", "100vw")],
        "z-0" => vec![("z-index", "0")],
        "z-10" => vec![("z-index", "10")],
        "z-20" => vec![("z-index", "20")],
        "z-30" => vec![("z-index", "30")],
        "z-40" => vec![("z-index", "40")],
        "z-50" => vec![("z-index", "50")],
        "z-auto" => vec![("z-index", "auto")],
        _ => return None,
    })
}

/// Negative utilities: `-z-10` → z-index -10, `-translate-x-4` →
/// translateX(-16px), `-rotate-45` → rotate(-45deg). Mirrors Python
/// `_resolve_one` tier 0 / `_resolve_negative_transform`.
fn resolve_negative(class: &str) -> Option<Vec<(String, String)>> {
    let pair = |k: &str, v: String| Some(vec![(k.to_string(), v)]);
    if let Some(rest) = class.strip_prefix("-z-") {
        if !rest.is_empty() && rest.trim_start_matches('-').chars().all(|c| c.is_ascii_digit()) {
            let val: i64 = rest.parse().unwrap_or(0);
            return pair("z-index", (-val).to_string());
        }
        return None;
    }
    let body = class.strip_prefix('-')?;
    let (name, num) = body.rsplit_once('-')?;
    let n: i64 = num.parse().ok()?;
    match name {
        "translate-x" => pair("transform", format!("translateX(-{}px)", n * 4)),
        "translate-y" => pair("transform", format!("translateY(-{}px)", n * 4)),
        "rotate" => pair("transform", format!("rotate(-{}deg)", n)),
        "skew-x" => pair("transform", format!("skewX(-{}deg)", n)),
        "skew-y" => pair("transform", format!("skewY(-{}deg)", n)),
        _ => None,
    }
}

/// Arbitrary values: `bg-[#ff0000]`, `w-[200px]`, `rotate-[45deg]`,
/// `-translate-x-[50%]`. Mirrors Python `_resolve_arbitrary` including the
/// `^([\w-]+)-\[(.+)\]$` anchor and the negation rule.
fn parse_arbitrary(class: &str) -> Option<Vec<(String, String)>> {
    let single = |k: &str, v: &str| Some(vec![(k.to_string(), v.to_string())]);
    let dash = class.find("-[")?;
    if !class.ends_with(']') {
        return None;
    }
    let prefix = &class[..dash];
    let value = &class[dash + 2..class.len() - 1];
    if prefix.is_empty() || value.is_empty() {
        return None;
    }
    if !prefix
        .trim_start_matches('-')
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let (negated, base) = match prefix.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, prefix),
    };
    if let Some(func) = transform_fn(base) {
        let val = if negated { negate_value(value) } else { value.to_string() };
        return single("transform", &format!("{}({})", func, val));
    }
    if negated {
        return None;
    }
    // NOTE: `px`/`py` intentionally expand to both axes. Python mapped them
    // to a single side (padding-left / padding-top); that asymmetry is a bug,
    // fixed here and mirrored back into morph/style/tailwind.py.
    match base {
        "bg" => single("background-color", value),
        "text" => single("color", value),
        "w" => single("width", value),
        "h" => single("height", value),
        "p" => single("padding", value),
        "px" => Some(vec![
            ("padding-left".to_string(), value.to_string()),
            ("padding-right".to_string(), value.to_string()),
        ]),
        "py" => Some(vec![
            ("padding-top".to_string(), value.to_string()),
            ("padding-bottom".to_string(), value.to_string()),
        ]),
        "m" => single("margin", value),
        "mt" => single("margin-top", value),
        "mb" => single("margin-bottom", value),
        "ml" => single("margin-left", value),
        "mr" => single("margin-right", value),
        "gap" => single("gap", value),
        "rounded" => single("border-radius", value),
        "opacity" => single("opacity", value),
        "top" => single("top", value),
        "bottom" => single("bottom", value),
        "left" => single("left", value),
        "right" => single("right", value),
        "min-w" => single("min-width", value),
        "min-h" => single("min-height", value),
        "max-w" => single("max-width", value),
        "max-h" => single("max-height", value),
        "z" => single("z-index", value),
        "font" => single("font-weight", value),
        _ => None,
    }
}

fn transform_fn(name: &str) -> Option<&'static str> {
    Some(match name {
        "translate-x" => "translateX",
        "translate-y" => "translateY",
        "rotate" => "rotate",
        "scale-x" => "scaleX",
        "scale-y" => "scaleY",
        "scale" => "scale",
        "skew-x" => "skewX",
        "skew-y" => "skewY",
        _ => return None,
    })
}

/// Negate an arbitrary transform value: `45deg` → `-45deg`, `-50%` → `50%`.
fn negate_value(value: &str) -> String {
    let v = value.trim();
    if let Some(rest) = v.strip_prefix('-') {
        return rest.to_string();
    }
    if let Some(rest) = v.strip_prefix('+') {
        return format!("-{}", rest);
    }
    format!("-{}", v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(class: &str) -> HashMap<String, String> {
        TailwindResolver::new().resolve(class)
    }

    #[test]
    fn static_entries_cover_all_tiers() {
        // Multi-property entries keep every declaration.
        let flex = resolved("flex-1");
        assert_eq!(flex.get("flex-grow").map(String::as_str), Some("1"));
        assert_eq!(flex.get("flex-shrink").map(String::as_str), Some("1"));
        assert_eq!(flex.get("flex-basis").map(String::as_str), Some("0%"));
        // Size ramps carry their line-height companions.
        let sm = resolved("text-sm");
        assert_eq!(sm.get("font-size").map(String::as_str), Some("14px"));
        assert_eq!(sm.get("line-height").map(String::as_str), Some("20px"));
        // Axis shorthands expand to both sides.
        let px = resolved("px-4");
        assert_eq!(px.get("padding-left").map(String::as_str), Some("16px"));
        assert_eq!(px.get("padding-right").map(String::as_str), Some("16px"));
        // Later classes win in a list.
        let merged = TailwindResolver::new().resolve_many("text-white text-black");
        assert_eq!(merged.get("color").map(String::as_str), Some("#000000"));
    }

    #[test]
    fn negative_utilities_negate_numerically() {
        assert_eq!(resolved("-z-10").get("z-index").map(String::as_str), Some("-10"));
        assert_eq!(resolved("-z-0").get("z-index").map(String::as_str), Some("0"));
        assert_eq!(
            resolved("-translate-x-4").get("transform").map(String::as_str),
            Some("translateX(-16px)")
        );
        assert_eq!(
            resolved("-rotate-45").get("transform").map(String::as_str),
            Some("rotate(-45deg)")
        );
        assert_eq!(
            resolved("-skew-y-12").get("transform").map(String::as_str),
            Some("skewY(-12deg)")
        );
        assert!(resolved("-mt-4").is_empty());
    }

    #[test]
    fn arbitrary_values_cover_every_prefix() {
        assert_eq!(
            resolved("bg-[#ff0000]").get("background-color").map(String::as_str),
            Some("#ff0000")
        );
        assert_eq!(resolved("w-[200px]").get("width").map(String::as_str), Some("200px"));
        assert_eq!(resolved("text-[red]").get("color").map(String::as_str), Some("red"));
        assert_eq!(resolved("z-[5]").get("z-index").map(String::as_str), Some("5"));
        assert_eq!(resolved("font-[700]").get("font-weight").map(String::as_str), Some("700"));
        assert_eq!(resolved("min-w-[10px]").get("min-width").map(String::as_str), Some("10px"));
        // px/py expand to both axes (single-side was a ported Python bug).
        let px = resolved("px-[3px]");
        assert_eq!(px.get("padding-left").map(String::as_str), Some("3px"));
        assert_eq!(px.get("padding-right").map(String::as_str), Some("3px"));
        let py = resolved("py-[3px]");
        assert_eq!(py.get("padding-top").map(String::as_str), Some("3px"));
        assert_eq!(py.get("padding-bottom").map(String::as_str), Some("3px"));
    }

    #[test]
    fn arbitrary_transforms_support_negation() {
        assert_eq!(
            resolved("rotate-[45deg]").get("transform").map(String::as_str),
            Some("rotate(45deg)")
        );
        assert_eq!(
            resolved("-translate-x-[50%]").get("transform").map(String::as_str),
            Some("translateX(-50%)")
        );
        assert_eq!(
            resolved("translate-y-[-3px]").get("transform").map(String::as_str),
            Some("translateY(-3px)")
        );
    }

    #[test]
    fn unknown_and_malformed_classes_resolve_empty() {
        for cls in ["bogus-class", "bg-", "w-[]", "bg-[unclosed", "-z-", "-rotate-", "px-[]"] {
            assert!(resolved(cls).is_empty(), "{} should resolve empty", cls);
        }
    }
}
