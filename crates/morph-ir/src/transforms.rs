//! CSS `transform` / `transform-origin` parsing and matrix composition.
//!
//! Mirrors `morph/style/transforms.py` (`parse_transform`,
//! `compose_transform`) and the builder's `_parse_transform_origin` so the
//! Rust pipeline resolves transforms at build time instead of shipping CSS
//! strings to the runtime.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LengthUnit {
    Px,
    Pct,
}

pub type LengthComp = (f32, LengthUnit);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransformOp {
    Matrix([f32; 6]),
    Matrix3d([f32; 16]),
    Perspective(f32),
    Rotate(f32),
    RotateX(f32),
    RotateY(f32),
    RotateZ(f32),
    Rotate3d(f32, f32, f32, f32),
    Translate(LengthComp, LengthComp),
    Translate3d(LengthComp, LengthComp, LengthComp),
    Scale(f32, f32),
    Scale3d(f32, f32, f32),
    Skew(f32, f32),
}

const GLOBAL_KEYWORDS: &[&str] = &["inherit", "initial", "revert", "revert-layer", "unset"];

/// Parse a CSS `transform` value into ops.
///
/// Returns `Some(vec![])` for `none` / global keywords (no transform),
/// `Some(ops)` for a valid function list, and `None` for invalid values
/// (the property must be ignored), mirroring Python's `parse_transform`.
pub fn parse_transform(value: &str) -> Option<Vec<TransformOp>> {
    let s = value.trim();
    if s.is_empty() {
        return None;
    }
    let low = s.to_ascii_lowercase();
    if low == "none" || GLOBAL_KEYWORDS.contains(&low.as_str()) {
        return Some(Vec::new());
    }
    if low.starts_with("none") {
        return None;
    }

    let bytes = s.as_bytes();
    let n = bytes.len();
    let mut ops: Vec<TransformOp> = Vec::new();
    let mut i = 0usize;
    while i < n {
        while i < n && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= n {
            break;
        }
        let start = i;
        while i < n && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-') {
            i += 1;
        }
        let name = s[start..i].to_ascii_lowercase();
        while i < n && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= n || bytes[i] != b'(' {
            return None;
        }
        let mut depth = 1i32;
        let mut j = i + 1;
        while j < n && depth > 0 {
            if bytes[j] == b'(' {
                depth += 1;
            } else if bytes[j] == b')' {
                depth -= 1;
            }
            j += 1;
        }
        if depth != 0 {
            return None;
        }
        let inner = &s[i + 1..j - 1];
        let args = split_args(inner);
        ops.push(build_op(&name, &args)?);
        i = j;
    }
    Some(ops)
}

fn split_args(inner: &str) -> Vec<String> {
    if inner.contains(',') {
        inner.split(',').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect()
    } else {
        inner.split_whitespace().map(str::to_string).collect()
    }
}

fn angle_to_deg(token: &str) -> Option<f32> {
    let s = token.trim().to_ascii_lowercase();
    if let Some(rest) = s.strip_suffix("deg") {
        return rest.trim().parse().ok();
    }
    // grad BEFORE rad — "100grad" ends with "rad".
    if let Some(rest) = s.strip_suffix("grad") {
        return rest.trim().parse().ok().map(|v: f32| v * 0.9);
    }
    if let Some(rest) = s.strip_suffix("rad") {
        return rest.trim().parse().ok().map(|v: f32| v.to_degrees());
    }
    if let Some(rest) = s.strip_suffix("turn") {
        return rest.trim().parse().ok().map(|v: f32| v * 360.0);
    }
    s.trim().parse().ok()
}

fn length_to_component(token: &str) -> Option<LengthComp> {
    let s = token.trim().to_ascii_lowercase();
    if let Some(rest) = s.strip_suffix('%') {
        return rest.trim().parse().ok().map(|v| (v, LengthUnit::Pct));
    }
    if let Some(rest) = s.strip_suffix("px") {
        return rest.trim().parse().ok().map(|v| (v, LengthUnit::Px));
    }
    s.parse().ok().map(|v| (v, LengthUnit::Px))
}

// Splitting this builder function risks behavior change.
#[allow(clippy::too_many_lines)]
fn build_op(name: &str, args: &[String]) -> Option<TransformOp> {
    let one = |i: usize| -> Option<f32> {
        let v = args.get(i)?;
        v.trim().parse().ok()
    };
    match name {
        "matrix" => {
            if args.len() != 6 {
                return None;
            }
            let mut m = [0.0f32; 6];
            for (i, slot) in m.iter_mut().enumerate() {
                *slot = one(i)?;
            }
            Some(TransformOp::Matrix(m))
        }
        "matrix3d" => {
            if args.len() != 16 {
                return None;
            }
            let mut m = [0.0f32; 16];
            for (i, slot) in m.iter_mut().enumerate() {
                *slot = one(i)?;
            }
            Some(TransformOp::Matrix3d(m))
        }
        "perspective" => {
            if args.len() != 1 {
                return None;
            }
            let comp = length_to_component(&args[0])?;
            if comp.1 == LengthUnit::Pct {
                return None;
            }
            Some(TransformOp::Perspective(comp.0))
        }
        "rotate" => angle_to_deg(&args[0]).map(TransformOp::Rotate),
        "rotatex" => angle_to_deg(&args[0]).map(TransformOp::RotateX),
        "rotatey" => angle_to_deg(&args[0]).map(TransformOp::RotateY),
        "rotatez" => angle_to_deg(&args[0]).map(TransformOp::RotateZ),
        "rotate3d" => {
            if args.len() != 4 {
                return None;
            }
            let x = one(0)?;
            let y = one(1)?;
            let z = one(2)?;
            let deg = angle_to_deg(&args[3])?;
            Some(TransformOp::Rotate3d(x, y, z, deg))
        }
        "translate" => {
            if args.is_empty() || args.len() > 2 {
                return None;
            }
            let tx = length_to_component(&args[0])?;
            let ty = if args.len() == 2 {
                length_to_component(&args[1])?
            } else {
                (0.0, LengthUnit::Px)
            };
            Some(TransformOp::Translate(tx, ty))
        }
        "translatex" => {
            if args.len() != 1 {
                return None;
            }
            let tx = length_to_component(&args[0])?;
            Some(TransformOp::Translate(tx, (0.0, LengthUnit::Px)))
        }
        "translatey" => {
            if args.len() != 1 {
                return None;
            }
            let ty = length_to_component(&args[0])?;
            Some(TransformOp::Translate((0.0, LengthUnit::Px), ty))
        }
        "translate3d" => {
            if args.len() != 3 {
                return None;
            }
            let tx = length_to_component(&args[0])?;
            let ty = length_to_component(&args[1])?;
            let tz = length_to_component(&args[2])?;
            if tz.1 == LengthUnit::Pct {
                return None;
            }
            Some(TransformOp::Translate3d(tx, ty, tz))
        }
        "translatez" => {
            if args.len() != 1 {
                return None;
            }
            let tz = length_to_component(&args[0])?;
            if tz.1 == LengthUnit::Pct {
                return None;
            }
            Some(TransformOp::Translate3d((0.0, LengthUnit::Px), (0.0, LengthUnit::Px), tz))
        }
        "scale" => {
            if args.is_empty() || args.len() > 2 {
                return None;
            }
            let sx = one(0)?;
            let sy = if args.len() == 2 { one(1)? } else { sx };
            Some(TransformOp::Scale(sx, sy))
        }
        "scalex" => {
            if args.len() != 1 {
                return None;
            }
            Some(TransformOp::Scale(one(0)?, 1.0))
        }
        "scaley" => {
            if args.len() != 1 {
                return None;
            }
            Some(TransformOp::Scale(1.0, one(0)?))
        }
        "scale3d" => {
            if args.len() != 3 {
                return None;
            }
            Some(TransformOp::Scale3d(one(0)?, one(1)?, one(2)?))
        }
        "scalez" => {
            if args.len() != 1 {
                return None;
            }
            Some(TransformOp::Scale3d(1.0, 1.0, one(0)?))
        }
        "skew" => {
            if args.is_empty() || args.len() > 2 {
                return None;
            }
            let ax = angle_to_deg(&args[0])?;
            let ay = if args.len() == 2 { angle_to_deg(&args[1])? } else { 0.0 };
            Some(TransformOp::Skew(ax, ay))
        }
        "skewx" => {
            if args.len() != 1 {
                return None;
            }
            Some(TransformOp::Skew(angle_to_deg(&args[0])?, 0.0))
        }
        "skewy" => {
            if args.len() != 1 {
                return None;
            }
            Some(TransformOp::Skew(0.0, angle_to_deg(&args[0])?))
        }
        _ => None,
    }
}

// ── 4x4 Matrix math (column-major, 16 floats) ──────────────────

const fn identity() -> [f32; 16] {
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
}

fn multiply(a: [f32; 16], b: [f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            let mut acc = 0.0f32;
            for k in 0..4 {
                acc += a[k * 4 + row] * b[col * 4 + k];
            }
            out[col * 4 + row] = acc;
        }
    }
    out
}

const fn translate(x: f32, y: f32, z: f32) -> [f32; 16] {
    let mut m = identity();
    m[12] = x;
    m[13] = y;
    m[14] = z;
    m
}

const fn scale(x: f32, y: f32, z: f32) -> [f32; 16] {
    let mut m = identity();
    m[0] = x;
    m[5] = y;
    m[10] = z;
    m
}

fn rotate_x(deg: f32) -> [f32; 16] {
    let a = deg.to_radians();
    let (c, s) = (a.cos(), a.sin());
    [1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0]
}

fn rotate_y(deg: f32) -> [f32; 16] {
    let a = deg.to_radians();
    let (c, s) = (a.cos(), a.sin());
    [c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0]
}

fn rotate_z(deg: f32) -> [f32; 16] {
    let a = deg.to_radians();
    let (c, s) = (a.cos(), a.sin());
    [c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
}

fn rotate_axis(ax: f32, ay: f32, az: f32, deg: f32) -> [f32; 16] {
    let length = (ax * ax + ay * ay + az * az).sqrt();
    if length < 1e-12 {
        return identity();
    }
    let (nx, ny, nz) = (ax / length, ay / length, az / length);
    let angle = deg.to_radians();
    let (cos_a, sin_a) = (angle.cos(), angle.sin());
    let one_minus_cos = 1.0 - cos_a;
    [
        one_minus_cos * nx * nx + cos_a,
        one_minus_cos * nx * ny + sin_a * nz,
        one_minus_cos * nx * nz - sin_a * ny,
        0.0,
        one_minus_cos * nx * ny - sin_a * nz,
        one_minus_cos * ny * ny + cos_a,
        one_minus_cos * ny * nz + sin_a * nx,
        0.0,
        one_minus_cos * nx * nz + sin_a * ny,
        one_minus_cos * ny * nz - sin_a * nx,
        one_minus_cos * nz * nz + cos_a,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ]
}

fn skew_x(deg: f32) -> [f32; 16] {
    let mut m = identity();
    m[4] = deg.to_radians().tan();
    m
}

fn skew_y(deg: f32) -> [f32; 16] {
    let mut m = identity();
    m[1] = deg.to_radians().tan();
    m
}

fn perspective(d: f32) -> [f32; 16] {
    if d <= 0.0 {
        return identity();
    }
    let mut m = identity();
    m[11] = -1.0 / d;
    m
}

const fn matrix6(m00: f32, m01: f32, m10: f32, m11: f32, tx: f32, ty: f32) -> [f32; 16] {
    [m00, m01, 0.0, 0.0, m10, m11, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0]
}

fn op_composer(op: &TransformOp) -> [f32; 16] {
    match *op {
        TransformOp::Matrix([m00, m01, m10, m11, tx, ty]) => matrix6(m00, m01, m10, m11, tx, ty),
        TransformOp::Matrix3d(m) => m,
        TransformOp::Perspective(d) => perspective(d),
        TransformOp::Rotate(deg) | TransformOp::RotateZ(deg) => rotate_z(deg),
        TransformOp::RotateX(deg) => rotate_x(deg),
        TransformOp::RotateY(deg) => rotate_y(deg),
        TransformOp::Rotate3d(x, y, z, deg) => rotate_axis(x, y, z, deg),
        TransformOp::Scale(x, y) => scale(x, y, 1.0),
        TransformOp::Scale3d(x, y, z) => scale(x, y, z),
        TransformOp::Skew(ax, ay) => multiply(skew_x(ax), skew_y(ay)),
        TransformOp::Translate(_, _) | TransformOp::Translate3d(_, _, _) => identity(),
    }
}

fn resolve_length(comp: LengthComp, own: f32) -> f32 {
    let (value, unit) = comp;
    match unit {
        LengthUnit::Pct => value / 100.0 * own,
        LengthUnit::Px => value,
    }
}

/// Compose parsed ops into a single column-major 4x4 matrix.
///
/// `own_w` / `own_h` are the element's border-box size, used to resolve `%`
/// lengths in translate functions (0.0 when the box is unknown at build time).
pub fn compose_transform(ops: &[TransformOp], own_w: f32, own_h: f32) -> [f32; 16] {
    let mut m = identity();
    for op in ops {
        let op_m = match op {
            TransformOp::Translate(tx, ty) => {
                translate(resolve_length(*tx, own_w), resolve_length(*ty, own_h), 0.0)
            }
            TransformOp::Translate3d(tx, ty, tz) => translate(
                resolve_length(*tx, own_w),
                resolve_length(*ty, own_h),
                resolve_length(*tz, own_h),
            ),
            other => op_composer(other),
        };
        m = multiply(m, op_m);
    }
    m
}

// ── transform-origin ───────────────────────────────────────────

type OriginAxis = (f32, bool);
type OriginPair = (OriginAxis, OriginAxis);
type TransformOrigin = (OriginPair, Option<(f32, f32)>);
type AxisParsed = (OriginAxis, Option<f32>);

/// Parse a CSS `transform-origin` value into a raw `((x, is_pct), (y, is_pct))`
/// pair plus the fraction already resolvable without the element box.
///
/// Resolved is `Some` only when both axes are keywords or percentages;
/// plain lengths need the element box (unknown at build time) and stay
/// `None`, leaving the runtime's default center origin.
pub fn parse_transform_origin(value: &str) -> Option<TransformOrigin> {
    fn axis(token: &str) -> Option<AxisParsed> {
        let k = token.trim().to_ascii_lowercase();
        match k.as_str() {
            "left" | "top" => Some(((0.0, false), Some(0.0))),
            "center" => Some(((0.5, false), Some(0.5))),
            "right" | "bottom" => Some(((1.0, false), Some(1.0))),
            _ => {
                if let Some(pct) = k.strip_suffix('%') {
                    let v: f32 = pct.trim().parse().ok()?;
                    return Some(((v, true), Some(v / 100.0)));
                }
                let num = k.strip_suffix("px").unwrap_or(&k);
                let v: f32 = num.trim().parse().ok()?;
                Some(((v, false), None))
            }
        }
    }

    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.is_empty() || parts.len() > 2 {
        return None;
    }

    let (x_raw, x_fx) = axis(parts[0])?;
    let (y_raw, y_fx) = if parts.len() == 2 { axis(parts[1])? } else { ((0.5, false), Some(0.5)) };
    let resolved = match (x_fx, y_fx) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    Some(((x_raw, y_raw), resolved))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f32, b: f32, eps: f32) {
        assert!((a - b).abs() < eps, "{a} != {b}");
    }

    #[test]
    fn parse_transform_none_and_keywords() {
        assert_eq!(parse_transform("none"), Some(vec![]));
        assert_eq!(parse_transform("inherit"), Some(vec![]));
        assert_eq!(parse_transform("  "), None);
        assert_eq!(parse_transform("none garbage"), None);
    }

    #[test]
    fn parse_transform_translate_px() {
        let ops = parse_transform("translateY(-1px)").unwrap();
        assert_eq!(
            ops,
            vec![TransformOp::Translate((0.0, LengthUnit::Px), (-1.0, LengthUnit::Px))]
        );
    }

    #[test]
    fn parse_transform_comma_and_space_args() {
        let a = parse_transform("translate(10px, 20px)").unwrap();
        let b = parse_transform("translate(10px 20px)").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn parse_transform_angles_units() {
        let deg = parse_transform("rotate(90deg)").unwrap();
        let rad = parse_transform("rotate(1.5707963rad)").unwrap();
        let turn = parse_transform("rotate(0.25turn)").unwrap();
        assert_eq!(deg, vec![TransformOp::Rotate(90.0)]);
        match (&rad[0], &turn[0]) {
            (TransformOp::Rotate(r), TransformOp::Rotate(t)) => {
                assert_close(*r, 90.0, 0.01);
                assert_close(*t, 90.0, 0.01);
            }
            _ => panic!("expected Rotate ops"),
        }
    }

    #[test]
    fn parse_transform_invalid() {
        assert!(parse_transform("translateY(-1px").is_none(), "unbalanced parens must be rejected");
        assert!(parse_transform("scale()").is_none());
        assert!(parse_transform("foo(1px)").is_none(), "unknown function");
    }

    #[test]
    fn compose_translate_px() {
        let ops = parse_transform("translate(10px, 20px)").unwrap();
        let m = compose_transform(&ops, 0.0, 0.0);
        assert_close(m[12], 10.0, 1e-5);
        assert_close(m[13], 20.0, 1e-5);
        assert_close(m[0], 1.0, 1e-5);
    }

    #[test]
    fn compose_translate_pct_uses_own_box() {
        let ops = parse_transform("translate(50%, 25%)").unwrap();
        let m = compose_transform(&ops, 200.0, 100.0);
        assert_close(m[12], 100.0, 1e-4);
        assert_close(m[13], 25.0, 1e-4);
    }

    #[test]
    fn compose_rotate_matches_math() {
        let ops = parse_transform("rotate(90deg)").unwrap();
        let m = compose_transform(&ops, 0.0, 0.0);
        assert_close(m[0], 0.0, 1e-5);
        assert_close(m[1], 1.0, 1e-5);
        assert_close(m[4], -1.0, 1e-5);
        assert_close(m[5], 0.0, 1e-5);
    }

    #[test]
    fn compose_chain_order() {
        // Mirrors Python's compose_transform for `translateX(10px) rotate(90deg)`
        // (column-major): m = [0,1,0,0,-1,0,0,0,0,0,1,0,10,0,0,1].
        let ops = parse_transform("translateX(10px) rotate(90deg)").unwrap();
        let m = compose_transform(&ops, 0.0, 0.0);
        let x = m[0] * 10.0 + m[12];
        let y = m[1] * 10.0 + m[13];
        assert_close(x, 10.0, 1e-4);
        assert_close(y, 10.0, 1e-4);
    }

    #[test]
    fn compose_matches_python_output() {
        // Expected matrices captured from morph/style/transforms.py.
        let expected: &[(&str, [f32; 16])] = &[
            (
                "translate(10px, -20px) rotate(30deg) scale(1.5, 2) skewX(15deg)",
                [
                    1.299_038, 0.75, 0.0, 0.0, -0.651_924, 1.933_013, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0,
                    10.0, -20.0, 0.0, 1.0,
                ],
            ),
            (
                "matrix(1, 0.2, 0.3, 1, 5, 6)",
                [1.0, 0.2, 0.0, 0.0, 0.3, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 5.0, 6.0, 0.0, 1.0],
            ),
            (
                "rotate3d(0, 0, 1, 45deg) translateX(-5px)",
                [
                    std::f32::consts::FRAC_1_SQRT_2,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    -std::f32::consts::FRAC_1_SQRT_2,
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                    0.0,
                    -3.535_534,
                    -3.535_534,
                    0.0,
                    1.0,
                ],
            ),
        ];
        for (css, want) in expected {
            let ops = parse_transform(css).unwrap();
            let got = compose_transform(&ops, 0.0, 0.0);
            for i in 0..16 {
                assert_close(got[i], want[i], 1e-4);
            }
        }
    }

    #[test]
    fn compose_scale_and_skew() {
        let ops = parse_transform("scale(2, 3)").unwrap();
        let m = compose_transform(&ops, 0.0, 0.0);
        assert_close(m[0], 2.0, 1e-5);
        assert_close(m[5], 3.0, 1e-5);

        let ops = parse_transform("skewY(45deg)").unwrap();
        let m = compose_transform(&ops, 0.0, 0.0);
        assert_close(m[1], 1.0, 1e-5);
    }

    #[test]
    fn parse_origin_keywords_and_pct() {
        let (raw, resolved) = parse_transform_origin("left top").unwrap();
        assert_eq!(raw, ((0.0, false), (0.0, false)));
        assert_eq!(resolved, Some((0.0, 0.0)));

        let (_, resolved) = parse_transform_origin("center").unwrap();
        assert_eq!(resolved, Some((0.5, 0.5)));

        let (raw, resolved) = parse_transform_origin("25% 75%").unwrap();
        assert_eq!(raw, ((25.0, true), (75.0, true)));
        assert_eq!(resolved, Some((0.25, 0.75)));

        let (_, resolved) = parse_transform_origin("10px 20px").unwrap();
        assert_eq!(resolved, None, "px origins need the element box");
    }
}
