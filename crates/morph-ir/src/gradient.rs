//! Background gradient IR — linear gradients first, radial/conic later.
//!
//! Parsing here is intentionally strict: anything beyond a single
//! `linear-gradient()` / `repeating-linear-gradient()` value (multiple
//! backgrounds, `url()` layers, radial/conic) returns `None` and the caller
//! falls back to the previous solid-color behavior.

use serde::{Deserialize, Serialize};

/// Maximum color stops per gradient (v1 shader limit).
pub const MAX_GRADIENT_STOPS: usize = 8;

/// Gradient shape. Only linear is parsed today; the enum reserves the rest
/// so later shapes don't churn the IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GradientKind {
    #[default]
    Linear,
}

/// Gradient axis in CSS `y-down` space.
///
/// `Angle` is CSS degrees: `0` points up (`to top`), `90` points right.
/// `Corner` holds `(x_sign, y_sign)` toward the target corner (`to top right`
/// is `(1.0, -1.0)`); the runtime resolves it against the real box aspect so
/// the axis aims exactly at the corner.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum GradientAxis {
    #[default]
    Bottom,
    Angle(f32),
    Corner(f32, f32),
}

impl GradientAxis {
    /// Default axis: `to bottom` (CSS initial value).
    pub fn angle_deg(&self) -> Option<f32> {
        match *self {
            Self::Bottom => Some(180.0),
            Self::Angle(d) => Some(d),
            Self::Corner(_, _) => None,
        }
    }
}

/// A stop position: percentages resolve against the gradient-line length,
/// pixels are absolute along it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum GradientPosition {
    Percent(f32),
    Px(f32),
}

/// One color stop. `hint` is a color-interpolation hint (`red, 30%, blue`)
/// applying between the previous stop and this one; the v1 shader lerps
/// linearly and ignores it.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct GradientStop {
    pub color: [f32; 4],
    pub position: Option<GradientPosition>,
    pub hint: Option<f32>,
}

/// A parsed single-layer linear gradient.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IRGradient {
    pub kind: GradientKind,
    pub repeating: bool,
    pub axis: GradientAxis,
    pub stops: Vec<GradientStop>,
}

impl IRGradient {
    /// Solid fallback for runtimes without gradient support: first stop.
    pub fn fallback_color(&self) -> [f32; 4] {
        self.stops.first().map_or([0.0, 0.0, 0.0, 0.0], |s| s.color)
    }
}

/// Parse a whole `background` / `background-image` value as a single linear
/// gradient. Returns `None` for anything else (solid colors, `url()`,
/// multi-layer, radial/conic).
pub fn parse_gradient(value: &str) -> Option<IRGradient> {
    let v = value.trim();
    let open = v.find('(')?;
    let close = v.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = v[..open].trim().to_lowercase();
    let repeating = match name.as_str() {
        "linear-gradient" => false,
        "repeating-linear-gradient" => true,
        _ => return None,
    };
    let parts = split_top_level(&v[open + 1..close], ',');
    if parts.len() < 2 {
        return None;
    }
    let mut parts: Vec<&str> = parts;
    let axis = parse_axis(parts[0]).map_or(GradientAxis::Bottom, |a| a);
    if parse_axis(parts[0]).is_some() {
        parts.remove(0);
    }
    let mut stops: Vec<GradientStop> = Vec::new();
    let mut pending_hint: Option<f32> = None;
    for part in parts {
        let tokens = split_top_level_ws(part);
        if tokens.is_empty() {
            return None;
        }
        if tokens.len() == 1 {
            if let Some(pos) = parse_position(tokens[0]) {
                if stops.is_empty() {
                    return None;
                }
                // Hints resolve against the surrounding interval, which needs
                // the box size — keep percent hints as fractions, decline px.
                match pos {
                    GradientPosition::Percent(p) => pending_hint = Some(p / 100.0),
                    GradientPosition::Px(_) => return None,
                }
                continue;
            }
        }
        let color = super::builder::parse_color(tokens[0])?;
        let mut positions: Vec<GradientPosition> = Vec::new();
        for tok in &tokens[1..] {
            positions.push(parse_position(tok)?);
        }
        if positions.len() > 2 {
            return None;
        }
        // A double position (`red 10px 20px`) is a solid color block:
        // expand to two stops sharing the color.
        if positions.is_empty() {
            stops.push(GradientStop { color, position: None, hint: pending_hint.take() });
        } else {
            for pos in positions {
                stops.push(GradientStop { color, position: Some(pos), hint: pending_hint.take() });
            }
        }
    }
    if stops.len() < 2 || stops.len() > MAX_GRADIENT_STOPS {
        return None;
    }
    Some(IRGradient { kind: GradientKind::Linear, repeating, axis, stops })
}

/// Parse a gradient direction or angle. `None` means "not a direction" (the
/// part is a color stop instead).
fn parse_axis(part: &str) -> Option<GradientAxis> {
    let p = part.trim().to_lowercase();
    if let Some(rest) = p.strip_prefix("to ") {
        let mut horiz: Option<f32> = None;
        let mut vert: Option<f32> = None;
        for word in rest.split_whitespace() {
            match word {
                "left" => horiz = Some(-1.0),
                "right" => horiz = Some(1.0),
                "top" => vert = Some(-1.0),
                "bottom" => vert = Some(1.0),
                _ => return None,
            }
        }
        match (horiz, vert) {
            (None, None) => None,
            (Some(h), None) => {
                Some(if h > 0.0 { GradientAxis::Angle(90.0) } else { GradientAxis::Angle(270.0) })
            }
            (None, Some(v)) => {
                Some(if v > 0.0 { GradientAxis::Bottom } else { GradientAxis::Angle(0.0) })
            }
            (Some(h), Some(v)) => Some(GradientAxis::Corner(h, v)),
        }
    } else {
        parse_angle_deg(&p).map(GradientAxis::Angle)
    }
}

fn parse_angle_deg(p: &str) -> Option<f32> {
    if let Some(n) = p.strip_suffix("deg") {
        n.trim().parse::<f32>().ok()
    } else if let Some(n) = p.strip_suffix("turn") {
        n.trim().parse::<f32>().ok().map(|t| t * 360.0)
    } else if let Some(n) = p.strip_suffix("grad") {
        n.trim().parse::<f32>().ok().map(|g| g * 0.9)
    } else if let Some(n) = p.strip_suffix("rad") {
        n.trim().parse::<f32>().ok().map(|r| r * 180.0 / std::f32::consts::PI)
    } else {
        None
    }
}

/// Parse a stop position: `50%`, `10px`, or bare `0`.
fn parse_position(tok: &str) -> Option<GradientPosition> {
    let t = tok.trim();
    if let Some(n) = t.strip_suffix('%') {
        n.trim().parse::<f32>().ok().map(GradientPosition::Percent)
    } else if let Some(n) = t.strip_suffix("px") {
        n.trim().parse::<f32>().ok().map(GradientPosition::Px)
    } else if t == "0" || t == "0.0" {
        Some(GradientPosition::Px(0.0))
    } else {
        None
    }
}

/// Split on a delimiter, ignoring delimiters nested inside `(...)`.
fn split_top_level(s: &str, delim: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        if ch == delim && depth == 0 {
            out.push(s[start..i].trim());
            start = i + ch.len_utf8();
        }
    }
    out.push(s[start..].trim());
    out
}

/// Split on whitespace, ignoring whitespace nested inside `(...)` so
/// `rgb(255, 0, 0)` stays one token.
fn split_top_level_ws(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start: Option<usize> = None;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        let is_space = ch.is_whitespace() && depth == 0;
        if is_space {
            if let Some(st) = start.take() {
                out.push(s[st..i].trim());
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(st) = start {
        out.push(s[st..].trim());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: [f32; 4], b: [f32; 4]) -> bool {
        (0..4).all(|i| (a[i] - b[i]).abs() < 0.001)
    }

    #[test]
    fn basic_two_stop_defaults_to_bottom() {
        let g = parse_gradient("linear-gradient(red, blue)").expect("parses");
        assert!(!g.repeating);
        assert_eq!(g.axis.angle_deg(), Some(180.0));
        assert_eq!(g.stops.len(), 2);
        assert!(close(g.stops[0].color, [1.0, 0.0, 0.0, 1.0]));
        assert!(close(g.stops[1].color, [0.0, 0.0, 1.0, 1.0]));
        assert!(close(g.fallback_color(), [1.0, 0.0, 0.0, 1.0]));
    }

    #[test]
    fn direction_keywords_and_angles() {
        let g = parse_gradient("linear-gradient(to right, red, blue)").expect("parses");
        assert_eq!(g.axis.angle_deg(), Some(90.0));
        let g = parse_gradient("linear-gradient(45deg, red, blue)").expect("parses");
        assert_eq!(g.axis.angle_deg(), Some(45.0));
        let g = parse_gradient("linear-gradient(0.25turn, red, blue)").expect("parses");
        assert!((g.axis.angle_deg().unwrap_or(-1.0) - 90.0).abs() < 0.001);
        let g = parse_gradient("linear-gradient(to top right, red, blue)").expect("parses");
        assert_eq!(g.axis, GradientAxis::Corner(1.0, -1.0));
    }

    #[test]
    fn repeating_and_positions() {
        let g = parse_gradient("repeating-linear-gradient(45deg, black 0 10px, white 10px 20px)")
            .expect("parses");
        assert!(g.repeating);
        assert_eq!(g.stops.len(), 4);
        assert_eq!(g.stops[0].position, Some(GradientPosition::Px(0.0)));
        assert_eq!(g.stops[1].position, Some(GradientPosition::Px(10.0)));
    }

    #[test]
    fn percent_stops_and_hints() {
        let g = parse_gradient("linear-gradient(red 0%, 30%, blue 100%)").expect("parses");
        assert_eq!(g.stops.len(), 2);
        assert_eq!(g.stops[1].hint, Some(0.3));
    }

    #[test]
    fn rejects_non_linear() {
        assert!(parse_gradient("#fff").is_none());
        assert!(parse_gradient("url(a.png)").is_none());
        assert!(parse_gradient("radial-gradient(circle, red, blue)").is_none());
        assert!(parse_gradient("linear-gradient(red)").is_none());
    }
}
