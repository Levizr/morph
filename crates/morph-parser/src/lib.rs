use anyhow::Result;
use std::path::Path;

/// Stub parser — will be replaced with Oxc + lightningcss
pub struct MorphParser;

impl MorphParser {
    pub fn parse_mx_file(path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path)?;
        // TODO: Use oxc_parser for JSX/TSX + lightningcss for CSS
        Ok(content)
    }

    pub fn parse_mx_str(source: &str, filename: &str) -> Result<ParsedMx> {
        // Stub: return raw source
        Ok(ParsedMx {
            filename: filename.to_string(),
            source: source.to_string(),
            has_errors: false,
            errors: vec![],
        })
    }
}

#[derive(Debug)]
pub struct ParsedMx {
    pub filename: String,
    pub source: String,
    pub has_errors: bool,
    pub errors: Vec<String>,
}
