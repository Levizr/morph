use std::collections::HashMap;

use anyhow::Result;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;

use super::ast_types::CssRule;

pub fn parse_css(source: &str) -> Result<HashMap<String, CssRule>> {
    let mut rules = HashMap::new();

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
                rules.insert(selector, CssRule { properties });
            }
        }
    }

    Ok(rules)
}
