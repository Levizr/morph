//! Background gradient IR — linear, radial and conic single layers.
//!
//! Parsing here is intentionally strict: anything beyond one
//! `*-gradient()` / `repeating-*-gradient()` value (multiple backgrounds,
//! `url()` layers) returns `None` and the caller falls back to the
//! previous solid-color behavior.

use serde::{Deserialize, Serialize};

/// Maximum color stops per gradient (v1 shader limit).
pub const MAX_GRADIENT_STOPS: usize = 8;

/// Gradient shape. Linear is axis-driven; radial and conic carry their
/// own geometry below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum GradientKind {
    #[default]
    Linear,
    Radial,
    Conic,
}

/// Center coordinate: percentages resolve against the box, pixels are absolute.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum CenterCoord {
    #[default]
    Center,
    Percent(f32),
    Px(f32),
}

impl CenterCoord {
    /// Resolve to a 0..1 fraction along `dim` pixels.
    pub fn resolve(&self, dim: f32) -> f32 {
        match *self {
            CenterCoord::Center => 0.5,
            CenterCoord::Percent(p) => p / 100.0,
            CenterCoord::Px(v) => {
                if dim > 1e-6 {
                    v / dim
                } else {
                    0.5
                }
            }
        }
    }
}

/// Radial ending shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RadialShape {
    #[default]
    Circle,
    Ellipse,
}

/// Radial size: keywords, or explicit radii (one length = circle radius;
/// two lengths = ellipse radii; `y` defaults to `x` for ellipse).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub enum RadialSize {
    ClosestSide,
    ClosestCorner,
    FarthestSide,
    #[default]
    FarthestCorner,
    Explicit {
        x: GradientPosition,
        y: Option<GradientPosition>,
    },
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

/// A parsed single-layer gradient (linear, radial or conic).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IRGradient {
    pub kind: GradientKind,
    pub repeating: bool,
    pub axis: GradientAxis,
    /// Radial/conic center. Defaults to 50%/50%.
    pub center: (CenterCoord, CenterCoord),
    /// Radial shape/size. Defaults to circle farthest-corner.
    pub radial_shape: RadialShape,
    pub radial_size: RadialSize,
    /// Conic start angle in CSS degrees (0 = up, clockwise).
    pub conic_from: f32,
    pub stops: Vec<GradientStop>,
}

impl IRGradient {
    /// Solid fallback for runtimes without gradient support: first stop.
    pub fn fallback_color(&self) -> [f32; 4] {
        self.stops.first().map_or([0.0, 0.0, 0.0, 0.0], |s| s.color)
    }
}

/// Parse a whole `background` / `background-image` value as a single
/// gradient layer. Returns `None` for anything else (solid colors,
/// `url()`, multi-layer).
pub fn parse_gradient(value: &str) -> Option<IRGradient> {
    let v = value.trim();
    let open = v.find('(')?;
    let close = v.rfind(')')?;
    if close <= open {
        return None;
    }
    let name = v[..open].trim().to_lowercase();
    let (kind, repeating) = match name.as_str() {
        "linear-gradient" => (GradientKind::Linear, false),
        "repeating-linear-gradient" => (GradientKind::Linear, true),
        "radial-gradient" => (GradientKind::Radial, false),
        "repeating-radial-gradient" => (GradientKind::Radial, true),
        "conic-gradient" => (GradientKind::Conic, false),
        "repeating-conic-gradient" => (GradientKind::Conic, true),
        _ => return None,
    };
    let parts = split_top_level(&v[open + 1..close], ',');
    if parts.len() < 2 {
        return None;
    }
    let mut parts: Vec<&str> = parts;
    let mut gradient = IRGradient { kind, repeating, ..Default::default() };
    match kind {
        GradientKind::Linear => {
            let axis = parse_axis(parts[0]).map_or(GradientAxis::Bottom, |a| a);
            if parse_axis(parts[0]).is_some() {
                parts.remove(0);
            }
            gradient.axis = axis;
            gradient.stops = parse_stops(&parts, StopPositions::LinearRadial)?;
        }
        GradientKind::Radial => {
            // A leading shape/size/`at` header is consumed; otherwise the
            // first part is a stop and defaults apply (`circle` alone can
            // never be a stop — no color is named like a shape keyword).
            if let Some((shape, size, center)) = parse_radial_prelude(parts[0]) {
                gradient.radial_shape = shape;
                gradient.radial_size = size;
                gradient.center = center;
                parts.remove(0);
            }
            gradient.stops = parse_stops(&parts, StopPositions::LinearRadial)?;
        }
        GradientKind::Conic => {
            // Same deal: only `from`/`at` headers consume. A stop never
            // starts with those words, so no ambiguity.
            if let Some((from, center)) = parse_conic_prelude(parts[0]) {
                gradient.conic_from = from;
                gradient.center = center;
                parts.remove(0);
            }
            gradient.stops = parse_stops(&parts, StopPositions::Conic)?;
        }
    }
    if gradient.stops.len() < 2 || gradient.stops.len() > MAX_GRADIENT_STOPS {
        return None;
    }
    Some(gradient)
}

/// Which stop positions a gradient shape accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum StopPositions {
    /// Percentages and px (linear, radial).
    LinearRadial,
    /// Percentages and angles (conic); px has no meaning on a dial.
    Conic,
}

/// Shared color-stop list parser. Double positions expand to two stops
/// sharing the color; hints attach to the following stop.
fn parse_stops(parts: &[&str], mode: StopPositions) -> Option<Vec<GradientStop>> {
    if parts.len() < 2 {
        return None;
    }
    let mut stops: Vec<GradientStop> = Vec::new();
    let mut pending_hint: Option<f32> = None;
    for part in parts {
        let tokens = split_top_level_ws(part);
        if tokens.is_empty() {
            return None;
        }
        if tokens.len() == 1 {
            if let Some(pos) = parse_stop_position(tokens[0], mode) {
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
            positions.push(parse_stop_position(tok, mode)?);
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
    Some(stops)
}

/// Parse one stop position for the given shape family.
fn parse_stop_position(tok: &str, mode: StopPositions) -> Option<GradientPosition> {
    match mode {
        StopPositions::LinearRadial => parse_position(tok),
        StopPositions::Conic => {
            let t = tok.trim();
            if let Some(n) = t.strip_suffix('%') {
                return n.trim().parse::<f32>().ok().map(GradientPosition::Percent);
            }
            // Angles resolve to percent-of-turn, matching every other
            // Percent payload downstream (offsets divide by 100).
            parse_angle_deg(&t.to_lowercase()).map(|d| GradientPosition::Percent(d * 100.0 / 360.0))
        }
    }
}

/// Parse a radial prelude (`circle`, `ellipse 50% 40%`, `farthest-side
/// at top left`, `at 20% 80%`). Returns `None` when the part is a color
/// stop instead — no stop starts with these keywords.
fn parse_radial_prelude(
    part: &str,
) -> Option<(RadialShape, RadialSize, (CenterCoord, CenterCoord))> {
    let (head, pos) = split_at_position(part)?;
    let center = match pos {
        Some(p) => parse_center_position(p)?,
        None => (CenterCoord::Center, CenterCoord::Center),
    };
    // Empty head with a position (`at center`) is valid.
    let tokens: Vec<&str> = head.split_whitespace().collect();
    if tokens.is_empty() {
        if pos.is_none() {
            return None;
        }
        return Some((RadialShape::Circle, RadialSize::FarthestCorner, center));
    }
    let mut shape = RadialShape::Circle;
    let mut shape_seen = false;
    let mut size: Option<RadialSize> = None;
    let mut lengths: Vec<GradientPosition> = Vec::new();
    for tok in tokens {
        let t = tok.to_lowercase();
        match t.as_str() {
            "circle" if !shape_seen => {
                shape = RadialShape::Circle;
                shape_seen = true;
            }
            "ellipse" if !shape_seen => {
                shape = RadialShape::Ellipse;
                shape_seen = true;
            }
            "closest-side" if size.is_none() => size = Some(RadialSize::ClosestSide),
            "closest-corner" if size.is_none() => size = Some(RadialSize::ClosestCorner),
            "farthest-side" if size.is_none() => size = Some(RadialSize::FarthestSide),
            "farthest-corner" if size.is_none() => size = Some(RadialSize::FarthestCorner),
            _ => match parse_position(tok) {
                Some(p) => lengths.push(p),
                None => return None,
            },
        }
    }
    let size = match (size, lengths.len()) {
        (Some(kw), 0) => kw,
        (None, 0) => RadialSize::FarthestCorner,
        (None, 1) => RadialSize::Explicit { x: lengths[0], y: None },
        (None, 2) if shape == RadialShape::Ellipse && shape_seen => {
            RadialSize::Explicit { x: lengths[0], y: Some(lengths[1]) }
        }
        (None, 2) if !shape_seen => {
            // Two radii without a shape imply an ellipse.
            shape = RadialShape::Ellipse;
            RadialSize::Explicit { x: lengths[0], y: Some(lengths[1]) }
        }
        // Explicit circle takes a single radius; keywords take none.
        _ => return None,
    };
    if shape == RadialShape::Circle {
        if let RadialSize::Explicit { y: Some(_), .. } = size {
            return None;
        }
    }
    Some((shape, size, center))
}

/// Split `head at position` on the top-level `at`. Returns `(head, pos)`.
/// An `at` with an empty position is always invalid.
fn split_at_position(part: &str) -> Option<(&str, Option<&str>)> {
    let low = part.to_lowercase();
    let bytes = low.as_bytes();
    let mut depth = 0i32;
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b' ' | b'\t' if depth == 0 && low[i..].starts_with(" at ") => {
                let head = part[..i].trim();
                let pos = part[i + 4..].trim();
                if pos.is_empty() {
                    return None;
                }
                return Some((head, Some(pos)));
            }
            _ => {}
        }
        i += 1;
    }
    Some((part.trim(), None))
}

/// Parse a center position: 1–2 of keywords / `%` / px.
fn parse_center_position(pos: &str) -> Option<(CenterCoord, CenterCoord)> {
    let tokens: Vec<&str> = pos.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 2 {
        return None;
    }
    let mut x: Option<CenterCoord> = None;
    let mut y: Option<CenterCoord> = None;
    for tok in tokens {
        let t = tok.to_lowercase();
        match t.as_str() {
            "left" if x.is_none() => x = Some(CenterCoord::Percent(0.0)),
            "right" if x.is_none() => x = Some(CenterCoord::Percent(100.0)),
            "top" if y.is_none() => y = Some(CenterCoord::Percent(0.0)),
            "bottom" if y.is_none() => y = Some(CenterCoord::Percent(100.0)),
            "center" => {
                if x.is_none() {
                    x = Some(CenterCoord::Center);
                } else if y.is_none() {
                    y = Some(CenterCoord::Center);
                } else {
                    return None;
                }
            }
            _ => match parse_position(tok) {
                Some(GradientPosition::Percent(p)) => {
                    let c = CenterCoord::Percent(p);
                    if x.is_none() {
                        x = Some(c);
                    } else if y.is_none() {
                        y = Some(c);
                    } else {
                        return None;
                    }
                }
                Some(GradientPosition::Px(v)) => {
                    let c = CenterCoord::Px(v);
                    if x.is_none() {
                        x = Some(c);
                    } else if y.is_none() {
                        y = Some(c);
                    } else {
                        return None;
                    }
                }
                None => return None,
            },
        }
    }
    Some((x.unwrap_or(CenterCoord::Center), y.unwrap_or(CenterCoord::Center)))
}

/// Parse a conic prelude (`from 45deg`, `at 20% 80%`, `from 0.25turn at
/// center`). Only `from`/`at` headers consume; anything else is a stop.
fn parse_conic_prelude(part: &str) -> Option<(f32, (CenterCoord, CenterCoord))> {
    let trimmed = part.trim();
    let low = trimmed.to_lowercase();
    if let Some(after) = low.strip_prefix("from ") {
        let after = after.trim();
        // Angle runs to `at` or end.
        let (angle_str, tail) = match split_at_position(after) {
            Some((h, Some(p))) => (h, Some(p)),
            _ => (after, None),
        };
        // `from` with no angle is garbage, not a stop — but the stop
        // parser rejects it identically, so plain None is fine either way.
        let from = parse_angle_deg(&angle_str.trim().to_lowercase())?;
        match tail {
            None => Some((from, (CenterCoord::Center, CenterCoord::Center))),
            Some(p) => Some((from, parse_center_position(p)?)),
        }
    } else if low.starts_with("at ") {
        Some((0.0, parse_center_position(trimmed[3..].trim())?))
    } else {
        None
    }
}
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
    fn rejects_non_gradients() {
        assert!(parse_gradient("#fff").is_none());
        assert!(parse_gradient("url(a.png)").is_none());
        assert!(parse_gradient("linear-gradient(red)").is_none());
        assert!(parse_gradient("radial-gradient(circle)").is_none());
        assert!(parse_gradient("conic-gradient(from bogus, red, blue)").is_none());
    }

    #[test]
    fn radial_shapes_sizes_positions() {
        let g = parse_gradient("radial-gradient(circle, red, blue)").expect("parses");
        assert_eq!(g.kind, GradientKind::Radial);
        assert!(!g.repeating);
        assert_eq!(g.radial_shape, RadialShape::Circle);
        assert_eq!(g.radial_size, RadialSize::FarthestCorner);
        assert_eq!(g.center, (CenterCoord::Center, CenterCoord::Center));
        assert_eq!(g.stops.len(), 2);
        let g = parse_gradient("radial-gradient(ellipse 50% 40% at 20% 80%, red, blue)")
            .expect("parses");
        assert_eq!(g.radial_shape, RadialShape::Ellipse);
        assert_eq!(
            g.radial_size,
            RadialSize::Explicit {
                x: GradientPosition::Percent(50.0),
                y: Some(GradientPosition::Percent(40.0))
            }
        );
        assert_eq!(g.center, (CenterCoord::Percent(20.0), CenterCoord::Percent(80.0)));
        let g =
            parse_gradient("repeating-radial-gradient(circle 10px, red 0 10px, blue 10px 20px)")
                .expect("parses");
        assert!(g.repeating);
        assert_eq!(g.radial_size, RadialSize::Explicit { x: GradientPosition::Px(10.0), y: None });
        let g =
            parse_gradient("radial-gradient(closest-side at top left, red, blue)").expect("parses");
        assert_eq!(g.radial_size, RadialSize::ClosestSide);
        assert_eq!(g.center, (CenterCoord::Percent(0.0), CenterCoord::Percent(0.0)));
        // Two radii with an explicit circle is invalid.
        assert!(parse_gradient("radial-gradient(circle 10px 20px, red, blue)").is_none());
    }

    #[test]
    fn conic_angles_positions() {
        let g = parse_gradient("conic-gradient(red, blue)").expect("parses");
        assert_eq!(g.kind, GradientKind::Conic);
        assert!(!g.repeating);
        assert!((g.conic_from - 0.0).abs() < 0.001);
        let g = parse_gradient("conic-gradient(from 45deg, red, blue)").expect("parses");
        assert!((g.conic_from - 45.0).abs() < 0.001);
        let g =
            parse_gradient("repeating-conic-gradient(from 0.25turn at center, red 0%, blue 25%)")
                .expect("parses");
        assert!(g.repeating);
        assert!((g.conic_from - 90.0).abs() < 0.001);
        // Angles resolve to percent-of-turn for stops.
        let g = parse_gradient("conic-gradient(red 0deg, blue 180deg)").expect("parses");
        assert_eq!(g.stops[0].position, Some(GradientPosition::Percent(0.0)));
        assert_eq!(g.stops[1].position, Some(GradientPosition::Percent(50.0)));
        // px has no meaning on a dial.
        assert!(parse_gradient("conic-gradient(red 10px, blue)").is_none());
    }
}
