//! CSS Grid track-list / placement / areas parsing.
//!
//! Covers the full template syntax: fixed lengths, `fr`, `%`, `auto`,
//! `min-content`, `max-content`, `minmax()`, `fit-content()`,
//! `repeat()` (fixed + `auto-fill`/`auto-fit`), line names (`[a b]`),
//! placement (`<line>`, `span <n>`, named lines, `auto`) and
//! `grid-template-areas` validation. Lengths reuse the builder parser;
//! `%` stays symbolic and resolves at layout against its own base.

use serde::{Deserialize, Serialize};

use super::style::Length;

/// One track size inside `grid-template-*` / auto tracks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridTrackSize {
    Fixed(Length),
    Flex(f32),
    Auto,
    MinContent,
    MaxContent,
    FitContent(Length),
    MinMax {
        min: GridTrackMin,
        max: GridTrackMax,
    },
}

/// `minmax()` minimum: fixed, `%`, content keywords, `auto`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridTrackMin {
    Fixed(Length),
    Auto,
    MinContent,
    MaxContent,
}

/// `minmax()` maximum: everything the minimum takes, plus `fr`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridTrackMax {
    Fixed(Length),
    Auto,
    MinContent,
    MaxContent,
    Flex(f32),
}

/// `repeat()` count.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridRepeatCount {
    Fixed(u32),
    AutoFill,
    AutoFit,
}

/// A parsed track list: sizes plus line names (`names[i]` sits before
/// track `i`; one extra trailing entry) plus symbolic repeats.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridTrackList {
    pub tracks: Vec<GridTrackSize>,
    pub line_names: Vec<Vec<String>>,
    pub repeats: Vec<GridRepeat>,
}

/// A `repeat()` kept symbolic for layout-time expansion (`auto-fill`
/// needs the container size; fixed counts expand here eagerly).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridRepeat {
    /// Track index in `tracks` where the expansion inserts.
    pub at: usize,
    pub count: GridRepeatCount,
    pub inner: Vec<GridTrackSize>,
}

/// A grid line reference: `auto`, `<n>`, `span <n>`, named, `span name`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum GridLine {
    #[default]
    Auto,
    Line(i32),
    Span(u32),
    Named(String),
    SpanNamed(String),
}

/// One `grid-template-areas` rectangle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridArea {
    pub name: String,
    /// Inclusive start, exclusive end, 0-based.
    pub row0: usize,
    pub col0: usize,
    pub row1: usize,
    pub col1: usize,
}

/// Split on top-level whitespace (parens- and bracket-aware: `rgba()`,
/// `minmax()`, `repeat()` and `[line names]` all nest).
fn split_top_ws(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' | '[' => {
                depth += 1;
                cur.push(c);
            }
            ')' | ']' => {
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

/// Split on top-level commas (parens-aware).
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

fn parse_track_size(tok: &str) -> Option<GridTrackSize> {
    let t = tok.trim();
    let low = t.to_ascii_lowercase();
    if low == "auto" {
        return Some(GridTrackSize::Auto);
    }
    if low == "min-content" {
        return Some(GridTrackSize::MinContent);
    }
    if low == "max-content" {
        return Some(GridTrackSize::MaxContent);
    }
    if low.ends_with("fr") {
        let num = low.strip_suffix("fr")?.trim();
        let v: f32 = if num.is_empty() { 1.0 } else { num.parse().ok()? };
        if v.is_finite() && v >= 0.0 {
            return Some(GridTrackSize::Flex(v));
        }
        return None;
    }
    if low.starts_with("minmax(") && low.ends_with(')') {
        let inner = &t["minmax(".len()..t.len() - 1];
        let args = split_top_commas(inner);
        if args.len() != 2 {
            return None;
        }
        return Some(GridTrackSize::MinMax {
            min: parse_track_min(&args[0])?,
            max: parse_track_max(&args[1])?,
        });
    }
    if low.starts_with("fit-content(") && low.ends_with(')') {
        let inner = &t["fit-content(".len()..t.len() - 1];
        let l = super::builder::parse_length(inner.trim())?;
        return Some(GridTrackSize::FitContent(l));
    }
    super::builder::parse_length(t).map(GridTrackSize::Fixed)
}

fn parse_track_min(tok: &str) -> Option<GridTrackMin> {
    let low = tok.trim().to_ascii_lowercase();
    if low == "auto" {
        return Some(GridTrackMin::Auto);
    }
    if low == "min-content" {
        return Some(GridTrackMin::MinContent);
    }
    if low == "max-content" {
        return Some(GridTrackMin::MaxContent);
    }
    super::builder::parse_length(tok).map(GridTrackMin::Fixed)
}

fn parse_track_max(tok: &str) -> Option<GridTrackMax> {
    let low = tok.trim().to_ascii_lowercase();
    if low == "auto" {
        return Some(GridTrackMax::Auto);
    }
    if low == "min-content" {
        return Some(GridTrackMax::MinContent);
    }
    if low == "max-content" {
        return Some(GridTrackMax::MaxContent);
    }
    if low.ends_with("fr") {
        let num = low.strip_suffix("fr")?.trim();
        let v: f32 = if num.is_empty() { 1.0 } else { num.parse().ok()? };
        if v.is_finite() && v >= 0.0 {
            return Some(GridTrackMax::Flex(v));
        }
        return None;
    }
    super::builder::parse_length(tok).map(GridTrackMax::Fixed)
}

fn parse_repeat(tok: &str) -> Option<(GridRepeatCount, Vec<GridTrackSize>)> {
    let low = tok.to_ascii_lowercase();
    if !low.starts_with("repeat(") || !tok.ends_with(')') {
        return None;
    }
    let inner = &tok["repeat(".len()..tok.len() - 1];
    let comma = inner.find(',')?;
    let (count_s, tracks_s) = inner.split_at(comma);
    let tracks_s = &tracks_s[1..];
    let count = match count_s.trim().to_ascii_lowercase().as_str() {
        "auto-fill" => GridRepeatCount::AutoFill,
        "auto-fit" => GridRepeatCount::AutoFit,
        n => GridRepeatCount::Fixed(n.parse::<u32>().ok()?),
    };
    if matches!(count, GridRepeatCount::Fixed(0)) {
        return None;
    }
    // Nested repeats and nested auto-repeat are invalid CSS.
    let inner_low = tracks_s.to_ascii_lowercase();
    if inner_low.contains("repeat(") {
        return None;
    }
    let mut inner_tracks = Vec::new();
    for part in split_top_ws(tracks_s) {
        if part.starts_with('[') {
            // Line names inside repeat() are dropped (placement by those
            // names still resolves against the expanded lines by index;
            // documented simplification).
            continue;
        }
        inner_tracks.push(parse_track_size(&part)?);
    }
    if inner_tracks.is_empty() {
        return None;
    }
    Some((count, inner_tracks))
}

/// Parse a full track list (`none` → empty). Fixed `repeat()` expands
/// eagerly; `auto-fill`/`auto-fit` stay symbolic.
pub fn parse_track_list(s: &str) -> Option<GridTrackList> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("none") {
        return Some(GridTrackList {
            tracks: Vec::new(),
            line_names: vec![Vec::new()],
            repeats: Vec::new(),
        });
    }
    let mut out = GridTrackList {
        tracks: Vec::new(),
        line_names: vec![Vec::new()],
        repeats: Vec::new(),
    };
    for tok in split_top_ws(t) {
        if tok.starts_with('[') && tok.ends_with(']') {
            // One `[a b]` group may carry several names.
            let inner = &tok[1..tok.len() - 1];
            out.line_names
                .last_mut()
                .unwrap()
                .extend(inner.split_whitespace().map(str::to_string));
            continue;
        }
        if tok.to_ascii_lowercase().starts_with("repeat(") {
            let (count, inner) = parse_repeat(&tok)?;
            match count {
                GridRepeatCount::Fixed(n) => {
                    for _ in 0..n {
                        for tr in &inner {
                            out.tracks.push(tr.clone());
                            out.line_names.push(Vec::new());
                        }
                    }
                }
                auto_count => {
                    out.repeats.push(GridRepeat {
                        at: out.tracks.len(),
                        count: auto_count,
                        inner,
                    });
                }
            }
            continue;
        }
        // `auto-fill`/`auto-fit` outside repeat() and `subgrid` are out.
        let low = tok.to_ascii_lowercase();
        if low == "subgrid" || low == "auto-fill" || low == "auto-fit" || low == "masonry" {
            return None;
        }
        out.tracks.push(parse_track_size(&tok)?);
        out.line_names.push(Vec::new());
    }
    if out.tracks.is_empty() && out.repeats.is_empty() {
        return None;
    }
    Some(out)
}

/// Parse one placement line (`grid-column-start` etc.): `auto`, `<n>`,
/// `span <n>`, `<name>`, `span <name>`.
pub fn parse_grid_line(s: &str) -> Option<GridLine> {
    let t = s.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("auto") {
        return Some(GridLine::Auto);
    }
    let parts = split_top_ws(t);
    if parts.len() == 2 && parts[0].eq_ignore_ascii_case("span") {
        if let Ok(n) = parts[1].parse::<u32>() {
            if n >= 1 {
                return Some(GridLine::Span(n));
            }
            return None;
        }
        if !parts[1].is_empty() {
            return Some(GridLine::SpanNamed(parts[1].clone()));
        }
        return None;
    }
    if parts.len() == 1 {
        if let Ok(n) = parts[0].parse::<i32>() {
            if n != 0 {
                return Some(GridLine::Line(n));
            }
            return None;
        }
        return Some(GridLine::Named(parts[0].clone()));
    }
    None
}

/// Parse `grid-column` / `grid-row` shorthand (`<start> / <end>`, either
/// side defaulting to `auto`).
pub fn parse_grid_placement(s: &str) -> Option<(GridLine, GridLine)> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let halves: Vec<&str> = t.split('/').collect();
    if halves.len() > 2 {
        return None;
    }
    let start = if halves[0].trim().is_empty() {
        GridLine::Auto
    } else {
        parse_grid_line(halves[0])?
    };
    let end = if halves.len() < 2 || halves[1].trim().is_empty() {
        GridLine::Auto
    } else {
        parse_grid_line(halves[1])?
    };
    Some((start, end))
}

/// Parse `grid-area`: a named area or up to four placement lines
/// (`row-start / column-start / row-end / column-end`).
pub fn parse_grid_area(s: &str) -> Option<GridAreaValue> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let halves: Vec<&str> = t.split('/').collect();
    if halves.len() == 1 {
        let tok = halves[0].trim();
        // A bare number is not a valid area (must be a name or 4 lines).
        if tok.parse::<i32>().is_ok() {
            return None;
        }
        return Some(GridAreaValue::Named(tok.to_string()));
    }
    if halves.len() != 4 {
        return None;
    }
    let mut lines = Vec::new();
    for h in halves {
        lines.push(if h.trim().is_empty() {
            GridLine::Auto
        } else {
            parse_grid_line(h)?
        });
    }
    Some(GridAreaValue::Lines(lines[0].clone(), lines[1].clone(), lines[2].clone(), lines[3].clone()))
}

/// A parsed `grid-area` value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GridAreaValue {
    Named(String),
    Lines(GridLine, GridLine, GridLine, GridLine),
}

/// `grid-auto-flow` (default `row`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
}

/// Parse `grid-template-areas` rows (each row is a separate string in the
/// input slice): validates rectangular named areas; `.` is empty.
/// Returns the rectangles plus the implicit column/row counts.
pub fn parse_grid_areas(rows: &[String]) -> Option<(Vec<GridArea>, usize, usize)> {
    if rows.is_empty() {
        return None;
    }
    let mut grid: Vec<Vec<String>> = Vec::new();
    for row in rows {
        let cells: Vec<String> = split_top_ws(row).into_iter().collect();
        if cells.is_empty() {
            return None;
        }
        grid.push(cells);
    }
    let ncols = grid[0].len();
    if grid.iter().any(|r| r.len() != ncols) {
        return None;
    }
    use std::collections::HashMap;
    let mut bounds: HashMap<String, (usize, usize, usize, usize)> = HashMap::new();
    for (r, row) in grid.iter().enumerate() {
        for (c, name) in row.iter().enumerate() {
            if name == "." {
                continue;
            }
            bounds
                .entry(name.clone())
                .and_modify(|b| {
                    b.0 = b.0.min(r);
                    b.1 = b.1.min(c);
                    b.2 = b.2.max(r + 1);
                    b.3 = b.3.max(c + 1);
                })
                .or_insert((r, c, r + 1, c + 1));
        }
    }
    // Every named area must form a complete rectangle.
    let mut areas = Vec::new();
    for (name, (r0, c0, r1, c1)) in &bounds {
        for r in *r0..*r1 {
            for c in *c0..*c1 {
                if grid[r][c] != *name {
                    return None;
                }
            }
        }
        areas.push(GridArea {
            name: name.clone(),
            row0: *r0,
            col0: *c0,
            row1: *r1,
            col1: *c1,
        });
    }
    areas.sort_by(|a, b| a.name.cmp(&b.name));
    Some((areas, ncols, grid.len()))
}

/// Canonical CSS text for a track list (what codegen emits; the C++
/// mirror parses it back at layout).
pub fn track_list_to_css(list: &GridTrackList) -> String {
    let mut parts: Vec<String> = Vec::new();
    // Fixed repeats already expanded; auto-fill/fit stay symbolic at
    // their recorded insertion index.
    let mut repeat_at: std::collections::HashMap<usize, &GridRepeat> = std::collections::HashMap::new();
    for rep in &list.repeats {
        repeat_at.insert(rep.at, rep);
    }
    let mut emit_repeat = |parts: &mut Vec<String>, rep: &GridRepeat| {
        let count = match rep.count {
            GridRepeatCount::AutoFill => "auto-fill",
            GridRepeatCount::AutoFit => "auto-fit",
            GridRepeatCount::Fixed(n) => {
                // Unreachable (expanded eagerly); degrade gracefully.
                for _ in 0..n {
                    for tr in &rep.inner {
                        parts.push(track_size_to_css(tr));
                    }
                }
                return;
            }
        };
        let inner: Vec<String> = rep.inner.iter().map(track_size_to_css).collect();
        parts.push(format!("repeat({count}, {})", inner.join(" ")));
    };
    for (i, tr) in list.tracks.iter().enumerate() {
        if let Some(rep) = repeat_at.remove(&i) {
            emit_repeat(&mut parts, rep);
        }
        if let Some(names) = list.line_names.get(i) {
            if !names.is_empty() {
                parts.push(format!("[{}]", names.join(" ")));
            }
        }
        parts.push(track_size_to_css(tr));
    }
    if let Some(rep) = repeat_at.remove(&list.tracks.len()) {
        emit_repeat(&mut parts, rep);
    }
    if let Some(names) = list.line_names.last() {
        if !names.is_empty() {
            parts.push(format!("[{}]", names.join(" ")));
        }
    }
    parts.join(" ")
}

/// Canonical CSS for one placement line (`auto`, `3`, `span 2`, names).
pub fn grid_line_to_css(l: &GridLine) -> String {
    match l {
        GridLine::Auto => "auto".to_string(),
        GridLine::Line(n) => format!("{n}"),
        GridLine::Span(n) => format!("span {n}"),
        GridLine::Named(s) => s.clone(),
        GridLine::SpanNamed(s) => format!("span {s}"),
    }
}

/// Canonical CSS for an area list: `"a a" "b b"` rows.
pub fn grid_areas_to_css(areas: &[GridArea], ncols: usize, nrows: usize) -> String {
    let mut grid = vec![vec![".".to_string(); ncols]; nrows];
    for a in areas {
        for r in a.row0..a.row1.min(nrows) {
            for c in a.col0..a.col1.min(ncols) {
                grid[r][c] = a.name.clone();
            }
        }
    }
    grid.iter()
        .map(|row| format!("\"{}\"", row.join(" ")))
        .collect::<Vec<_>>()
        .join(" ")
}

fn track_size_to_css(t: &GridTrackSize) -> String {    match t {
        GridTrackSize::Fixed(l) => l.to_css(),
        GridTrackSize::Flex(v) => {
            if *v == 1.0 {
                "1fr".to_string()
            } else {
                format!("{v}fr")
            }
        }
        GridTrackSize::Auto => "auto".to_string(),
        GridTrackSize::MinContent => "min-content".to_string(),
        GridTrackSize::MaxContent => "max-content".to_string(),
        GridTrackSize::FitContent(l) => format!("fit-content({})", l.to_css()),
        GridTrackSize::MinMax { min, max } => {
            let mn = match min {
                GridTrackMin::Fixed(l) => l.to_css(),
                GridTrackMin::Auto => "auto".to_string(),
                GridTrackMin::MinContent => "min-content".to_string(),
                GridTrackMin::MaxContent => "max-content".to_string(),
            };
            let mx = match max {
                GridTrackMax::Fixed(l) => l.to_css(),
                GridTrackMax::Auto => "auto".to_string(),
                GridTrackMax::MinContent => "min-content".to_string(),
                GridTrackMax::MaxContent => "max-content".to_string(),
                GridTrackMax::Flex(v) => {
                    if *v == 1.0 {
                        "1fr".to_string()
                    } else {
                        format!("{v}fr")
                    }
                }
            };
            format!("minmax({mn}, {mx})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_basic() {
        let l = parse_track_list("100px 1fr 2fr auto").unwrap();
        assert_eq!(l.tracks.len(), 4);
        assert_eq!(l.tracks[1], GridTrackSize::Flex(1.0));
        assert_eq!(l.tracks[3], GridTrackSize::Auto);
    }

    #[test]
    fn tracks_names_and_repeat() {
        let l = parse_track_list("[a] 100px [b c] repeat(2, 1fr) [d]").unwrap();
        assert_eq!(l.tracks.len(), 3);
        assert_eq!(l.line_names[0], vec!["a".to_string()]);
        assert_eq!(l.line_names[1], vec!["b".to_string(), "c".to_string()]);
        assert_eq!(l.line_names[3], vec!["d".to_string()]);
    }

    #[test]
    fn tracks_auto_repeat_stays_symbolic() {
        let l = parse_track_list("repeat(auto-fill, minmax(100px, 1fr))").unwrap();
        assert!(l.tracks.is_empty());
        assert_eq!(l.repeats.len(), 1);
        assert_eq!(l.repeats[0].count, GridRepeatCount::AutoFill);
    }

    #[test]
    fn tracks_minmax_content() {
        let l = parse_track_list("minmax(100px, 1fr) min-content max-content fit-content(50%)").unwrap();
        assert_eq!(l.tracks.len(), 4);
        assert!(matches!(l.tracks[0], GridTrackSize::MinMax { .. }));
    }

    #[test]
    fn tracks_reject_garbage() {
        assert!(parse_track_list("").unwrap().tracks.is_empty());
        assert!(parse_track_list("none").unwrap().tracks.is_empty());
        assert!(parse_track_list("subgrid").is_none());
        assert!(parse_track_list("repeat(2, repeat(2, 1fr))").is_none());
        assert!(parse_track_list("10px bogus-unit").is_none());
    }

    #[test]
    fn placement_lines() {
        assert_eq!(parse_grid_line("auto"), Some(GridLine::Auto));
        assert_eq!(parse_grid_line("2"), Some(GridLine::Line(2)));
        assert_eq!(parse_grid_line("-1"), Some(GridLine::Line(-1)));
        assert_eq!(parse_grid_line("span 3"), Some(GridLine::Span(3)));
        assert_eq!(parse_grid_line("sidebar"), Some(GridLine::Named("sidebar".into())));
        assert_eq!(parse_grid_placement("1 / span 2"), Some((GridLine::Line(1), GridLine::Span(2))));
        assert_eq!(parse_grid_placement("span 2"), Some((GridLine::Span(2), GridLine::Auto)));
    }

    #[test]
    fn areas_rectangles() {
        let rows = vec!["a a b".to_string(), "a a b".to_string()];
        let (areas, ncols, nrows) = parse_grid_areas(&rows).unwrap();
        assert_eq!((ncols, nrows), (3, 2));
        let a = areas.iter().find(|x| x.name == "a").unwrap();
        assert_eq!((a.row0, a.col0, a.row1, a.col1), (0, 0, 2, 2));
        // Non-rectangular area is invalid.
        let bad = vec!["a b".to_string(), "b a".to_string()];
        assert!(parse_grid_areas(&bad).is_none());
    }

    #[test]
    fn canonical_roundtrip() {
        let l = parse_track_list("[a] 100px [b] 1fr minmax(10px, 2fr)").unwrap();
        // px rides bare (Length::to_css convention); the C++ mirror
        // parses bare numbers as px.
        assert_eq!(track_list_to_css(&l), "[a] 100 [b] 1fr minmax(10, 2fr)");
    }
}
