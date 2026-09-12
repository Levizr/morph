use std::collections::HashMap;

use anyhow::Result;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;
#[allow(unused_imports)]
use lightningcss::rules::keyframes::KeyframesName;

use super::ast_types::{CssData, CssKeyframe, CssRule};

pub fn parse_css(source: &str) -> Result<CssData> {
    let mut rules: Vec<(String, CssRule)> = Vec::new();
    let mut keyframes: HashMap<String, Vec<CssKeyframe>> = HashMap::new();

    // Lightningcss borrows the source; leak to satisfy 'static bound on DefaultAtRule
    let leaked: &'static str = Box::leak(source.to_owned().into_boxed_str());
    let stylesheet = StyleSheet::parse(leaked, ParserOptions::default())?;

    for rule in &stylesheet.rules.0 {
        if let lightningcss::rules::CssRule::Style(style_rule) = rule {
            let selector = style_rule.selectors.to_string();
            let mut properties = HashMap::new();
            for prop in &style_rule.declarations.declarations {
                let name = prop.property_id().to_css_string(PrinterOptions::default()).unwrap_or_default();
                let value = prop.value_to_css_string(PrinterOptions::default()).unwrap_or_default();
                properties.insert(name, value);
            }
            for prop in &style_rule.declarations.important_declarations {
                let name = prop.property_id().to_css_string(PrinterOptions::default()).unwrap_or_default();
                let value = prop.value_to_css_string(PrinterOptions::default()).unwrap_or_default();
                properties.insert(name, value);
            }
            if !properties.is_empty() {
                rules.push((selector.clone(), CssRule { selector, properties }));
            }
        } else if let lightningcss::rules::CssRule::Keyframes(kf_rule) = rule {
            let name = match &kf_rule.name {
                KeyframesName::Ident(i) => i.0.as_ref().to_string(),
                KeyframesName::Custom(s) => s.as_ref().to_owned(),
            };
            let entry = keyframes.entry(name).or_default();
            for kf in &kf_rule.keyframes {
                // A grouped block (`0%, 100% { ... }`) fans out to one
                // entry per selector; duplicate offsets merge with
                // later blocks winning per-property (mirrors Python's
                // `_merge_keyframe`).
                let mut offsets = Vec::new();
                for s in &kf.selectors {
                    let offset = match s {
                        lightningcss::rules::keyframes::KeyframeSelector::Percentage(p) => Some(p.0),
                        lightningcss::rules::keyframes::KeyframeSelector::From => Some(0.0),
                        lightningcss::rules::keyframes::KeyframeSelector::To => Some(1.0),
                        _ => None,
                    };
                    if let Some(offset) = offset {
                        offsets.push(offset);
                    }
                }
                if offsets.is_empty() {
                    continue;
                }
                let mut properties = HashMap::new();
                for prop in &kf.declarations.declarations {
                    let name = prop.property_id().to_css_string(PrinterOptions::default()).unwrap_or_default();
                    let value = prop.value_to_css_string(PrinterOptions::default()).unwrap_or_default();
                    properties.insert(name, value);
                }
                for prop in &kf.declarations.important_declarations {
                    let name = prop.property_id().to_css_string(PrinterOptions::default()).unwrap_or_default();
                    let value = prop.value_to_css_string(PrinterOptions::default()).unwrap_or_default();
                    properties.insert(name, value);
                }
                for offset in offsets {
                    match entry.iter_mut().find(|kf| (kf.offset - offset).abs() < 1e-9) {
                        Some(existing) => {
                            existing.properties.extend(properties.clone());
                        }
                        None => entry.push(CssKeyframe { offset, properties: properties.clone() }),
                    }
                }
            }
        }
    }

    Ok(CssData { rules, keyframes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grouped_keyframe_selectors_fan_out() {
        let data = parse_css(
            "@keyframes bounce-x { 0%, 100% { left: 0; } 25% { left: 100%; } 50% { left: 30%; } }",
        )
        .unwrap();
        let frames = &data.keyframes["bounce-x"];
        assert_eq!(frames.len(), 4);
        let offsets: Vec<f32> = frames.iter().map(|kf| kf.offset).collect();
        assert!(offsets.contains(&0.0));
        assert!(offsets.contains(&1.0));
    }

    #[test]
    fn duplicate_keyframe_offsets_merge_later_wins() {
        let data = parse_css(
            "@keyframes dupe { 50% { left: 10px; top: 1px; } 50% { left: 20px; } }",
        )
        .unwrap();
        let frames = &data.keyframes["dupe"];
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].properties.get("left").map(String::as_str), Some("20px"));
        assert_eq!(frames[0].properties.get("top").map(String::as_str), Some("1px"));
    }
}
