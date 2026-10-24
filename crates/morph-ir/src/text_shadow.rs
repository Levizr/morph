//! `text-shadow` parsing: `none`, or a comma-separated list of
//! `<offset-x> <offset-y> [<blur>] [<color>]`.
//!
//! Unlike `box-shadow`, the color is optional and may appear anywhere in the
//! layer; when omitted it inherits the text color at paint time. Offsets can
//! be negative; blur must be non-negative. At most [`MAX_TEXT_SHADOWS`]
//! layers are kept (text ops carry a fixed-size array).

use serde::{Deserialize, Serialize};

use super::style::Length;

/// Maximum text shadows per element (fixed-size op payload).
pub const MAX_TEXT_SHADOWS: usize = 4;

/// One parsed `text-shadow` layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IRTextShadow {
    pub offset_x: Length,
    pub offset_y: Length,
    pub blur: Length,
    /// `None` means `currentColor`.
    pub color: Option<[f32; 4]>,
}

fn non_negative(length: Length) -> bool {
    match length {
        Length::Px(v)
        | Length::Pct(v)
        | Length::Em(v)
        | Length::Rem(v)
        | Length::Vw(v)
        | Length::Vh(v)
        | Length::Vmin(v)
        | Length::Vmax(v)
        | Length::Ch(v)
        | Length::Ex(v) => v >= 0.0,
    }
}

fn parse_text_shadow_layer(s: &str) -> Option<IRTextShadow> {
    let mut lengths: Vec<Length> = Vec::new();
    let mut color: Option<[f32; 4]> = None;
    for tok in super::shadow::split_top_ws(s) {
        if let Some(l) = super::builder::parse_length_no_pct(&tok) {
            lengths.push(l);
            continue;
        }
        if color.is_none() {
            // `currentcolor` is the same as an omitted color at paint time.
            if tok.eq_ignore_ascii_case("currentcolor") {
                continue;
            }
            if let Some(c) = super::builder::parse_color(&tok) {
                color = Some(c);
                continue;
            }
        }
        return None;
    }
    // Two offsets are required; blur is optional. A third length is blur.
    if !(2..=3).contains(&lengths.len()) {
        return None;
    }
    let blur = lengths.get(2).copied().unwrap_or(Length::Px(0.0));
    if !non_negative(blur) {
        return None;
    }
    let mut it = lengths.into_iter();
    Some(IRTextShadow { offset_x: it.next()?, offset_y: it.next()?, blur, color })
}

/// Parse a full `text-shadow` value. `none`/empty clears; anything
/// unparseable (or beyond the cap) drops the declaration.
pub fn parse_text_shadow(s: &str) -> Option<Vec<IRTextShadow>> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in super::shadow::split_top_commas(t) {
        out.push(parse_text_shadow_layer(&part)?);
    }
    if out.is_empty() || out.len() > MAX_TEXT_SHADOWS {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_only_uses_current_color() {
        let v = parse_text_shadow("2px 3px").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].offset_x, Length::Px(2.0));
        assert_eq!(v[0].offset_y, Length::Px(3.0));
        assert_eq!(v[0].blur, Length::Px(0.0));
        assert_eq!(v[0].color, None);
    }

    #[test]
    fn color_can_lead_or_trail() {
        let a = parse_text_shadow("red 1px 2px 3px").unwrap();
        let b = parse_text_shadow("1px 2px 3px red").unwrap();
        assert_eq!(a, b);
        assert_eq!(a[0].color, Some([1.0, 0.0, 0.0, 1.0]));
        let c = parse_text_shadow("1px 2px currentcolor").unwrap();
        assert_eq!(c[0].color, None);
    }

    #[test]
    fn multiple_layers_and_none() {
        assert_eq!(parse_text_shadow("0 1px #000, 0 2px 4px rgba(0,0,0,.4)").unwrap().len(), 2);
        assert_eq!(parse_text_shadow("none").unwrap(), []);
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(parse_text_shadow("1px").is_none());
        assert!(parse_text_shadow("1px 2px -3px red").is_none());
        assert!(parse_text_shadow("1px 2px 3px 4px red").is_none());
        assert!(parse_text_shadow("1px solid red").is_none());
    }
}
