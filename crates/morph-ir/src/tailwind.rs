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
        if let Some((prop, val)) = parse_arbitrary(class) {
            out.insert(prop, val);
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
        "block" => vec![("display", "block")],
        "inline" => vec![("display", "inline")],
        "inline-block" => vec![("display", "inline-block")],
        "flex" => vec![("display", "flex")],
        "inline-flex" => vec![("display", "inline-flex")],
        "hidden" => vec![("display", "none")],
        "grid" => vec![("display", "grid")],
        "flex-row" => vec![("flex-direction", "row")],
        "flex-col" => vec![("flex-direction", "column")],
        "flex-wrap" => vec![("flex-wrap", "wrap")],
        "flex-nowrap" => vec![("flex-wrap", "nowrap")],
        "justify-center" => vec![("justify-content", "center")],
        "justify-between" => vec![("justify-content", "space-between")],
        "justify-start" => vec![("justify-content", "flex-start")],
        "justify-end" => vec![("justify-content", "flex-end")],
        "items-center" => vec![("align-items", "center")],
        "items-start" => vec![("align-items", "flex-start")],
        "items-end" => vec![("align-items", "flex-end")],
        "items-stretch" => vec![("align-items", "stretch")],
        "p-0" => vec![("padding", "0px")],
        "p-1" => vec![("padding", "4px")],
        "p-2" => vec![("padding", "8px")],
        "p-3" => vec![("padding", "12px")],
        "p-4" => vec![("padding", "16px")],
        "p-5" => vec![("padding", "20px")],
        "p-6" => vec![("padding", "24px")],
        "p-8" => vec![("padding", "32px")],
        "m-0" => vec![("margin", "0px")],
        "m-1" => vec![("margin", "4px")],
        "m-2" => vec![("margin", "8px")],
        "m-4" => vec![("margin", "16px")],
        "px-4" => vec![("padding-left", "16px"), ("padding-right", "16px")],
        "py-4" => vec![("padding-top", "16px"), ("padding-bottom", "16px")],
        "gap-2" => vec![("gap", "8px")],
        "gap-4" => vec![("gap", "16px")],
        "w-full" => vec![("width", "100%")],
        "h-full" => vec![("height", "100%")],
        "w-screen" => vec![("width", "100vw")],
        "h-screen" => vec![("height", "100vh")],
        "text-left" => vec![("text-align", "left")],
        "text-center" => vec![("text-align", "center")],
        "text-right" => vec![("text-align", "right")],
        "text-white" => vec![("color", "#ffffff")],
        "text-black" => vec![("color", "#000000")],
        "font-bold" => vec![("font-weight", "bold")],
        "font-normal" => vec![("font-weight", "normal")],
        "text-sm" => vec![("font-size", "14px")],
        "text-base" => vec![("font-size", "16px")],
        "text-lg" => vec![("font-size", "18px")],
        "text-xl" => vec![("font-size", "20px")],
        "bg-white" => vec![("background-color", "#ffffff")],
        "bg-black" => vec![("background-color", "#000000")],
        "bg-transparent" => vec![("background-color", "transparent")],
        "bg-blue-500" => vec![("background-color", "#3b82f6")],
        "bg-red-500" => vec![("background-color", "#ef4444")],
        "bg-green-500" => vec![("background-color", "#22c55e")],
        "rounded" => vec![("border-radius", "4px")],
        "rounded-md" => vec![("border-radius", "6px")],
        "rounded-lg" => vec![("border-radius", "8px")],
        "rounded-full" => vec![("border-radius", "9999px")],
        "border" => vec![("border-width", "1px"), ("border-style", "solid")],
        "border-0" => vec![("border-width", "0px")],
        "opacity-0" => vec![("opacity", "0")],
        "opacity-50" => vec![("opacity", "0.5")],
        "opacity-100" => vec![("opacity", "1")],
        "relative" => vec![("position", "relative")],
        "absolute" => vec![("position", "absolute")],
        "fixed" => vec![("position", "fixed")],
        "static" => vec![("position", "static")],
        _ => return None,
    })
}

fn parse_arbitrary(class: &str) -> Option<(String, String)> {
    let start = class.find("-[")? + 2;
    let end = class.rfind(']')?;
    if end <= start { return None; }
    let prefix = &class[..start-2];
    let value = &class[start..end];
    let prop = match prefix {
        "bg" => "background-color",
        "w" => "width",
        "h" => "height",
        "p" => "padding",
        "m" => "margin",
        "text" if value.starts_with('#') || value.starts_with("rgb") => "color",
        _ => return None,
    };
    Some((prop.to_string(), value.to_string()))
}
