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
                let offset = kf.selectors.first().and_then(|s| match s {
                    lightningcss::rules::keyframes::KeyframeSelector::Percentage(p) => Some(p.0),
                    lightningcss::rules::keyframes::KeyframeSelector::From => Some(0.0),
                    lightningcss::rules::keyframes::KeyframeSelector::To => Some(1.0),
                    _ => None,
                });
                if let Some(offset) = offset {
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
                    entry.push(CssKeyframe { offset, properties });
                }
            }
        }
    }

    Ok(CssData { rules, keyframes })
}

