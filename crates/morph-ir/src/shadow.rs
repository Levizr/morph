//! `box-shadow` parsing: `none`, or a comma-separated list of
//! `[inset]? <ox> <oy> [<blur>] [<spread>] [<color>]`.
//!
//! Lengths reuse the builder parser minus percentages (invalid CSS for
//! shadows, like `border-width`). At most [`MAX_BOX_SHADOWS`] shadows;
//! longer lists drop the declaration, mirroring the gradient stop cap.

use serde::{Deserialize, Serialize};

use super::style::Length;

/// Maximum shadows per box (v1 shader limit: unrolled uniform arrays).
pub const MAX_BOX_SHADOWS: usize = 4;

/// One parsed `box-shadow` layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IRBoxShadow {
    pub offset_x: Length,
    pub offset_y: Length,
    pub blur: Length,
    pub spread: Length,
    pub color: [f32; 4],
    pub inset: bool,
}

/// Split on commas that are not inside `(...)` (colors carry commas).
fn split_top_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => {
                parts.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !parts.is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

/// Split on whitespace that is not inside `(...)` (`rgba(0, 0, 0, .5)`).
fn split_top_ws(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            _ if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    parts.push(cur.clone());
                    cur.clear();
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

fn parse_shadow_layer(s: &str) -> Option<IRBoxShadow> {
    let mut inset = false;
    let mut lengths: Vec<Length> = Vec::new();
    let mut color: Option<[f32; 4]> = None;
    for tok in split_top_ws(s) {
        if tok.eq_ignore_ascii_case("inset") {
            inset = true;
            continue;
        }
        if let Some(l) = super::builder::parse_length_no_pct(&tok) {
            lengths.push(l);
            continue;
        }
        if color.is_none() {
            if let Some(c) = super::builder::parse_color(&tok) {
                color = Some(c);
                continue;
            }
        }
        return None;
    }
    // Offsets required; blur/spread optional. Blur must be non-negative
    // (negative blur invalidates the whole declaration per CSS).
    if lengths.len() < 2 || lengths.len() > 4 {
        return None;
    }
    let nonneg = |l: &Length| -> bool {
        let v = match l {
            Length::Px(v)
            | Length::Pct(v)
            | Length::Em(v)
            | Length::Rem(v)
            | Length::Vw(v)
            | Length::Vh(v)
            | Length::Vmin(v)
            | Length::Vmax(v)
            | Length::Ch(v)
            | Length::Ex(v) => *v,
        };
        v >= 0.0
    };
    if lengths.len() >= 3 && !nonneg(&lengths[2]) {
        return None;
    }
    let zero = Length::Px(0.0);
    let mut it = lengths.into_iter();
    Some(IRBoxShadow {
        offset_x: it.next().unwrap_or(Length::Px(0.0)),
        offset_y: it.next().unwrap_or(Length::Px(0.0)),
        blur: it.next().unwrap_or(zero),
        spread: it.next().unwrap_or(Length::Px(0.0)),
        color: color.unwrap_or([0.0, 0.0, 0.0, 0.5]),
        inset,
    })
}

/// Parse a full `box-shadow` value. `none`/empty clears; anything
/// unparseable (or beyond the cap) drops the declaration.
pub fn parse_box_shadow(s: &str) -> Option<Vec<IRBoxShadow>> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for part in split_top_commas(t) {
        out.push(parse_shadow_layer(&part)?);
    }
    if out.is_empty() || out.len() > MAX_BOX_SHADOWS {
        return None;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 4], b: [f32; 4]) -> bool {
        a.iter().zip(b.iter()).all(|(x, y)| (x - y).abs() < 1e-4)
    }

    #[test]
    fn offsets_only_default_rest() {
        let v = parse_box_shadow("10px 20px").unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].offset_x, Length::Px(10.0));
        assert_eq!(v[0].offset_y, Length::Px(20.0));
        assert_eq!(v[0].blur, Length::Px(0.0));
        assert_eq!(v[0].spread, Length::Px(0.0));
        assert!(close(v[0].color, [0.0, 0.0, 0.0, 0.5]));
        assert!(!v[0].inset);
    }

    #[test]
    fn full_layer_with_inset_and_rgba() {
        let v = parse_box_shadow("inset 0 8px 24px 4px rgba(0, 0, 0, 0.5)").unwrap();
        assert_eq!(v.len(), 1);
        assert!(v[0].inset);
        assert_eq!(v[0].offset_x, Length::Px(0.0));
        assert_eq!(v[0].blur, Length::Px(24.0));
        assert_eq!(v[0].spread, Length::Px(4.0));
        assert!(close(v[0].color, [0.0, 0.0, 0.0, 0.5]));
    }

    #[test]
    fn named_color_and_multi_list() {
        let v = parse_box_shadow("2px 2px 4px red, 0 0 0 2px blue").unwrap();
        assert_eq!(v.len(), 2);
        assert!(close(v[0].color, [1.0, 0.0, 0.0, 1.0]));
        assert!(close(v[1].color, [0.0, 0.0, 1.0, 1.0]));
    }

    #[test]
    fn none_clears() {
        assert_eq!(parse_box_shadow("none").unwrap().len(), 0);
    }

    #[test]
    fn rejects_bad_values() {
        assert!(parse_box_shadow("10px").is_none());
        assert!(parse_box_shadow("10px 10px -4px black").is_none());
        assert!(parse_box_shadow("10px 10px 4px 2px 1px black").is_none());
        assert!(parse_box_shadow("10px solid red").is_none());
        assert!(parse_box_shadow("10% 10px black").is_none());
        assert!(parse_box_shadow("1px 1px black, 2px 2px black, 3px 3px black, 4px 4px black, 5px 5px black").is_none());
    }
}
