use std::collections::HashMap;

use anyhow::Result;
use lightningcss::media_query::{
    MediaCondition, MediaFeature, MediaFeatureComparison, MediaFeatureName, MediaFeatureValue,
    MediaList, MediaType, Operator, Qualifier,
};
#[allow(unused_imports)]
use lightningcss::rules::keyframes::KeyframesName;
use lightningcss::stylesheet::{ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::traits::ToCss;
use lightningcss::values::length::{Length, LengthValue};

use super::ast_types::{CssData, CssKeyframe, CssMedia, CssRule};

/// Resolve a media-query length against a 16px initial font size. Viewport
/// units are intentionally unsupported: a query like `max-width: 50vw`
/// depends on the width being tested, which cannot be reduced to a fixed
/// breakpoint.
fn media_length_px(length: &Length) -> Option<f32> {
    let Length::Value(value) = length else {
        return None;
    };
    match value {
        LengthValue::Px(v) => Some(*v),
        LengthValue::Em(v)
        | LengthValue::Rem(v)
        | LengthValue::Ex(v)
        | LengthValue::Rex(v)
        | LengthValue::Ch(v)
        | LengthValue::Rch(v)
        | LengthValue::Cap(v)
        | LengthValue::Rcap(v)
        | LengthValue::Ic(v)
        | LengthValue::Ric(v)
        | LengthValue::Lh(v)
        | LengthValue::Rlh(v) => Some(v * 16.0),
        other => other.to_px(),
    }
}

/// CSS strict comparisons cannot be represented by the inclusive bounds in
/// `CssMedia`; nudge the threshold by one float step at this magnitude.
fn just_above(v: f32) -> f32 {
    v + f32::EPSILON * v.abs().max(1.0)
}

fn just_below(v: f32) -> f32 {
    v - f32::EPSILON * v.abs().max(1.0)
}

/// One width feature reduced to inclusive bounds.
fn width_feature(
    name: &MediaFeatureName<'_, lightningcss::media_query::MediaFeatureId>,
    value: &MediaFeatureValue<'_>,
    comparator: Option<MediaFeatureComparison>,
) -> Option<(Option<f32>, Option<f32>)> {
    if !matches!(name, MediaFeatureName::Standard(lightningcss::media_query::MediaFeatureId::Width))
    {
        return None;
    }
    let MediaFeatureValue::Length(length) = value else {
        return None;
    };
    let v = media_length_px(length)?;
    Some(match comparator {
        None | Some(MediaFeatureComparison::Equal) => (Some(v), Some(v)),
        Some(MediaFeatureComparison::GreaterThan) => (Some(just_above(v)), None),
        Some(MediaFeatureComparison::GreaterThanEqual) => (Some(v), None),
        Some(MediaFeatureComparison::LessThan) => (None, Some(just_below(v))),
        Some(MediaFeatureComparison::LessThanEqual) => (None, Some(v)),
    })
}

const fn merge_bounds(
    a: (Option<f32>, Option<f32>),
    b: (Option<f32>, Option<f32>),
) -> (Option<f32>, Option<f32>) {
    let min = match (a.0, b.0) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) | (None, x) => x,
    };
    let max = match (a.1, b.1) {
        (Some(x), Some(y)) => Some(x.min(y)),
        (x, None) | (None, x) => x,
    };
    (min, max)
}

/// Evaluate a lightningcss condition. `None` means "not one of the width
/// conditions this engine supports"; callers skip the whole media block
/// instead of treating it as true.
fn condition_bounds(condition: &MediaCondition<'_>) -> Option<(Option<f32>, Option<f32>)> {
    match condition {
        MediaCondition::Feature(feature) => match feature {
            MediaFeature::Plain { name, value } => width_feature(name, value, None),
            MediaFeature::Range { name, operator, value } => {
                width_feature(name, value, Some(*operator))
            }
            MediaFeature::Interval { name, start, start_operator, end, end_operator } => {
                let start_bounds = width_feature(name, start, Some(*start_operator))?;
                let end_bounds = width_feature(name, end, Some(*end_operator))?;
                Some(merge_bounds(start_bounds, end_bounds))
            }
            MediaFeature::Boolean { .. } => None,
        },
        MediaCondition::Operation { operator: Operator::And, conditions } => {
            let mut bounds = (None, None);
            for item in conditions {
                bounds = merge_bounds(bounds, condition_bounds(item)?);
            }
            Some(bounds)
        }
        MediaCondition::Operation { operator: Operator::Or, .. }
        | MediaCondition::Not(_)
        | MediaCondition::Unknown(_) => None,
    }
}

/// Reduce a media query list to the single width interval this engine can
/// evaluate. Multiple comma-separated queries (OR) are unsupported.
fn parse_media(list: &MediaList<'_>) -> CssMedia {
    if list.media_queries.is_empty() {
        return CssMedia { min_width: None, max_width: None, supported: true };
    }
    if list.media_queries.len() != 1 {
        return CssMedia { min_width: None, max_width: None, supported: false };
    }
    let query = &list.media_queries[0];
    if query.qualifier == Some(Qualifier::Not) {
        return CssMedia { min_width: None, max_width: None, supported: false };
    }
    if !matches!(query.media_type, MediaType::All | MediaType::Screen) {
        return CssMedia { min_width: None, max_width: None, supported: false };
    }
    let Some(condition) = &query.condition else {
        return CssMedia { min_width: None, max_width: None, supported: true };
    };
    match condition_bounds(condition) {
        Some((min_width, max_width)) => CssMedia { min_width, max_width, supported: true },
        None => CssMedia { min_width: None, max_width: None, supported: false },
    }
}

const fn intersect_media(outer: Option<&CssMedia>, inner: CssMedia) -> CssMedia {
    let Some(outer) = outer else {
        return inner;
    };
    if !outer.supported || !inner.supported {
        return CssMedia { min_width: None, max_width: None, supported: false };
    }
    let (min_width, max_width) =
        merge_bounds((outer.min_width, outer.max_width), (inner.min_width, inner.max_width));
    CssMedia { min_width, max_width, supported: true }
}

fn style_declarations(
    declarations: &lightningcss::declaration::DeclarationBlock<'_>,
) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for prop in &declarations.declarations {
        let name = prop.property_id().to_css_string(PrinterOptions::default()).unwrap_or_default();
        let value = prop.value_to_css_string(PrinterOptions::default()).unwrap_or_default();
        properties.insert(name, value);
    }
    for prop in &declarations.important_declarations {
        let name = prop.property_id().to_css_string(PrinterOptions::default()).unwrap_or_default();
        let value = prop.value_to_css_string(PrinterOptions::default()).unwrap_or_default();
        properties.insert(name, value);
    }
    properties
}

fn collect_keyframes(
    kf_rule: &lightningcss::rules::keyframes::KeyframesRule<'_>,
    keyframes: &mut HashMap<String, Vec<CssKeyframe>>,
) {
    let name = match &kf_rule.name {
        KeyframesName::Ident(i) => i.0.as_ref().to_string(),
        KeyframesName::Custom(s) => s.as_ref().to_owned(),
    };
    let entry = keyframes.entry(name).or_default();
    for kf in &kf_rule.keyframes {
        // A grouped block (`0%, 100% { ... }`) fans out to one entry per
        // selector; duplicate offsets merge with later blocks winning
        // per-property (mirrors Python's `_merge_keyframe`).
        let mut offsets = Vec::new();
        for s in &kf.selectors {
            let offset = match s {
                lightningcss::rules::keyframes::KeyframeSelector::Percentage(p) => Some(p.0),
                lightningcss::rules::keyframes::KeyframeSelector::From => Some(0.0),
                lightningcss::rules::keyframes::KeyframeSelector::To => Some(1.0),
                lightningcss::rules::keyframes::KeyframeSelector::TimelineRangePercentage(_) => {
                    None
                }
            };
            if let Some(offset) = offset {
                offsets.push(offset);
            }
        }
        if offsets.is_empty() {
            continue;
        }
        let properties = style_declarations(&kf.declarations);
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

fn walk_rules(
    rules: &[lightningcss::rules::CssRule<'_>],
    media: Option<&CssMedia>,
    out: &mut Vec<(String, CssRule)>,
    keyframes: &mut HashMap<String, Vec<CssKeyframe>>,
) {
    for rule in rules {
        match rule {
            lightningcss::rules::CssRule::Style(style_rule) => {
                let selector = style_rule.selectors.to_string();
                let properties = style_declarations(&style_rule.declarations);
                if !properties.is_empty() {
                    out.push((
                        selector.clone(),
                        CssRule { selector, properties, media: media.cloned() },
                    ));
                }
            }
            lightningcss::rules::CssRule::Keyframes(kf_rule) => {
                collect_keyframes(kf_rule, keyframes);
            }
            lightningcss::rules::CssRule::Media(media_rule) => {
                let query = parse_media(&media_rule.query);
                let combined = intersect_media(media, query);
                walk_rules(&media_rule.rules.0, Some(&combined), out, keyframes);
            }
            _ => {}
        }
    }
}

pub fn parse_css(source: &str) -> Result<CssData> {
    let mut rules: Vec<(String, CssRule)> = Vec::new();
    let mut keyframes: HashMap<String, Vec<CssKeyframe>> = HashMap::new();

    // Lightningcss borrows the source; leak to satisfy 'static bound on DefaultAtRule
    let leaked: &'static str = Box::leak(source.to_owned().into_boxed_str());
    let stylesheet = StyleSheet::parse(leaked, ParserOptions::default())?;

    walk_rules(&stylesheet.rules.0, None, &mut rules, &mut keyframes);
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
        let data =
            parse_css("@keyframes dupe { 50% { left: 10px; top: 1px; } 50% { left: 20px; } }")
                .unwrap();
        let frames = &data.keyframes["dupe"];
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].properties.get("left").map(String::as_str), Some("20px"));
        assert_eq!(frames[0].properties.get("top").map(String::as_str), Some("1px"));
    }

    #[test]
    fn media_rules_keep_bounds_and_source_order() {
        let data = parse_css(
            ".a { color: red; } @media (max-width: 600px) { .a { color: blue; } } \
             @media screen and (min-width: 900px) { .a { color: green; } }",
        )
        .unwrap();
        assert_eq!(data.rules.len(), 3);
        assert!(data.rules[0].1.media.is_none());
        let narrow = data.rules[1].1.media.as_ref().unwrap();
        assert_eq!(narrow.max_width, Some(600.0));
        assert!(narrow.matches(600.0));
        assert!(!narrow.matches(601.0));
        let wide = data.rules[2].1.media.as_ref().unwrap();
        assert_eq!(wide.min_width, Some(900.0));
        assert!(wide.matches(900.0));
    }

    #[test]
    fn unsupported_media_conditions_are_flagged() {
        let data = parse_css(
            "@media (orientation: landscape) { .a { color: red; } } \
             @media (min-width: 500px) and (max-width: 700px) { .b { color: blue; } }",
        )
        .unwrap();
        assert!(!data.rules[0].1.media.as_ref().unwrap().supported);
        let range = data.rules[1].1.media.as_ref().unwrap();
        assert!(range.supported);
        assert_eq!(range.min_width, Some(500.0));
        assert_eq!(range.max_width, Some(700.0));
    }
}
