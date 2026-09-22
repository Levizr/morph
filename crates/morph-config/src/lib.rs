use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_title")]
    pub title: String,
    /// Owner window id/route, or `""` for none (independent top-level —
    /// the browser/Electron default; ownership is always explicit).
    /// Resolved to a WID at lowering; `"auto"` means most-recently-focused.
    #[serde(default)]
    pub parent: String,
    /// Modal windows block every other window's close while open.
    /// Requires a resolvable parent (hard error without one).
    #[serde(default)]
    pub modal: bool,
    /// Presentation role (`""`/`"default"`/`"dialog"`/`"popup"`).
    /// Hints only — never gates behavior. Parsed via `WindowRole`.
    #[serde(default)]
    pub role: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            title: "Morph App".to_string(),
            parent: String::new(),
            modal: false,
            role: String::new(),
        }
    }
}

/// Window presentation role: interned int at codegen, window-manager
/// hints only. New roles are additive (`3, 4, …`) — behavior comes from
/// `parent` + `modal`, never from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WindowRole {
    #[default]
    Default = 0,
    Dialog = 1,
    Popup = 2,
}

impl WindowRole {
    /// Parse a role literal. Empty means default. Unknown roles are hard
    /// errors — roles are a closed set, so anything else is a typo.
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "" | "default" => Ok(Self::Default),
            "dialog" => Ok(Self::Dialog),
            "popup" => Ok(Self::Popup),
            other => anyhow::bail!(
                "unknown window role {other:?} (expected \"default\", \"dialog\", or \"popup\")"
            ),
        }
    }

    pub const fn as_int(self) -> i32 {
        self as i32
    }
}

const fn default_width() -> u32 {
    800
}
const fn default_height() -> u32 {
    600
}
fn default_title() -> String {
    "Morph App".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default)]
    pub wayland: bool,
    #[serde(default)]
    pub system_freetype: bool,
    #[serde(default = "default_true")]
    pub upx: bool,
    #[serde(default)]
    pub upx_version: String,
    /// Extra UPX flags (e.g. `["--ultra-brute"]`). Empty = built-in
    /// default (`--lzma`). Full control — passed through verbatim.
    #[serde(default)]
    pub upx_flags: Vec<String>,
    #[serde(default)]
    pub cxx: String,
    #[serde(default)]
    pub dev_cxx: String,
    #[serde(default)]
    pub cmake: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            wayland: false,
            system_freetype: false,
            upx: true,
            upx_version: String::new(),
            upx_flags: Vec::new(),
            cxx: String::new(),
            dev_cxx: String::new(),
            cmake: String::new(),
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default = "default_runtime_type")]
    #[serde(rename = "type")]
    pub runtime_type: String,
    #[serde(default = "default_runtime_version")]
    pub version: String,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { runtime_type: "cpp".to_string(), version: "0.1.0".to_string() }
    }
}

fn default_runtime_type() -> String {
    "cpp".to_string()
}
fn default_runtime_version() -> String {
    "0.1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NativeConfig {
    #[serde(default)]
    pub include_dirs: Vec<String>,
    #[serde(default)]
    pub library_dirs: Vec<String>,
    #[serde(default)]
    pub libraries: Vec<String>,
    #[serde(default)]
    pub cflags: Vec<String>,
    #[serde(default)]
    pub ldflags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LintConfig {
    #[serde(default)]
    pub disable: Vec<String>,
    #[serde(default)]
    pub severities: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NavigationConfig {
    /// Page-cache policy: `0` (default, destroy on leave), `N` (keep N
    /// last pages, LRU), or `"all"` (unbounded). Cached pages hold tree +
    /// state only — window chrome is always freed.
    #[serde(default)]
    pub cache: PageCache,
}

/// `navigation.cache` value: a count or the `"all"` keyword.
///
/// Numbers and strings share one key, so the enum is untagged; unknown
/// strings fail at `capacity()` with the offending value (not silently
/// defaulted).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PageCache {
    Count(u32),
    Keyword(String),
}

impl Default for PageCache {
    fn default() -> Self {
        Self::Count(0)
    }
}

impl PageCache {
    /// Generated-code capacity: `0` = caching off, `N` = LRU cap,
    /// `-1` = unbounded (`"all"`).
    pub fn capacity(&self) -> Result<i64> {
        match self {
            Self::Count(n) => Ok(i64::from(*n)),
            Self::Keyword(s) if s == "all" => Ok(-1),
            Self::Keyword(s) => anyhow::bail!(
                "navigation.cache must be 0, a positive integer, or \"all\" (found {s:?})"
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_entry")]
    pub entry: String,
    #[serde(default = "default_output")]
    pub output: String,
    #[serde(default)]
    pub window: WindowConfig,
    #[serde(default = "default_renderer")]
    pub renderer: String,
    #[serde(rename = "types", default = "default_type_mode")]
    pub type_mode: String,
    #[serde(default)]
    pub dependencies: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub cpp_sources: Vec<String>,
    #[serde(default)]
    pub native: NativeConfig,
    #[serde(default)]
    pub node_bridge: bool,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub lint: LintConfig,
    #[serde(default)]
    pub runtime: RuntimeConfig,
    #[serde(default)]
    pub navigation: NavigationConfig,
}

fn default_name() -> String {
    "my-app".to_string()
}
fn default_entry() -> String {
    "src/App.mx".to_string()
}
fn default_output() -> String {
    ".morph/output".to_string()
}
fn default_type_mode() -> String {
    "infer".to_string()
}

pub fn clean_app_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    // Collapse multiple _ and trim
    let mut cleaned = String::new();
    let mut prev_us = false;
    for ch in out.chars() {
        if ch == '_' {
            if !prev_us {
                cleaned.push('_');
            }
            prev_us = true;
        } else {
            cleaned.push(ch);
            prev_us = false;
        }
    }
    let cleaned = cleaned.trim_matches('_').to_string();
    if cleaned.is_empty() {
        "app".to_string()
    } else {
        cleaned
    }
}

/// Source extensions a project entry/scan may use (strict TS/TSX + Morph's .mx).
pub fn is_supported_source_ext(ext: &str) -> bool {
    matches!(ext, "mx" | "ts" | "tsx")
}

/// Detected-but-disallowed JS-family extensions. Morph intentionally only supports
/// strict TypeScript/TSX, so these trigger a hard error instead of being parsed.
pub fn is_disallowed_js_ext(ext: &str) -> bool {
    matches!(ext, "js" | "jsx" | "mjs" | "cjs")
}

/// Validate a file path's extension as a morph source entry. Returns a hard-error
/// message when the extension is disallowed, or `None` when it is supported.
pub fn validate_entry_ext(path: &Path) -> Result<(), String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_supported_source_ext(ext) {
        return Ok(());
    }
    if is_disallowed_js_ext(ext) {
        return Err(format!(
            "`.{}` files are not supported — Morph only supports strict `.ts`, `.tsx`, and `.mx`. Found: {}",
            ext,
            path.display()
        ));
    }
    Err(format!(
        "unsupported source extension `.{}` (expected `.ts`, `.tsx`, or `.mx`): {}",
        ext,
        path.display()
    ))
}

fn default_renderer() -> String {
    "flash".to_string()
}

impl Default for MorphConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            entry: default_entry(),
            output: default_output(),
            window: WindowConfig::default(),
            renderer: default_renderer(),
            type_mode: default_type_mode(),
            dependencies: HashMap::default(),
            cpp_sources: Vec::default(),
            native: NativeConfig::default(),
            node_bridge: false,
            build: BuildConfig::default(),
            lint: LintConfig::default(),
            runtime: RuntimeConfig::default(),
            navigation: NavigationConfig::default(),
        }
    }
}

impl MorphConfig {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let cfg: Self = serde_json::from_str(&content)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        Ok(cfg)
    }

    pub fn parse_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let json = self.to_json_pretty()?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        // Validate runtime version is semver
        semver::Version::parse(&self.runtime.version)
            .with_context(|| format!("invalid runtime version: {}", self.runtime.version))?;
        if self.runtime.runtime_type != "cpp" && self.runtime.runtime_type != "rust" {
            anyhow::bail!(
                "runtime.type must be 'cpp' or 'rust', got '{}'",
                self.runtime.runtime_type
            );
        }
        Ok(())
    }
}

/// Version file format for releases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFile {
    pub version: String,
    pub changelog: String,
    #[serde(default)]
    pub breaking: bool,
}

impl VersionFile {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn parse_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

/// Lock file (morph.lock)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorphLock {
    pub runtime: LockRuntime,
    #[serde(default)]
    pub generated_by: String,
    #[serde(default)]
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockRuntime {
    #[serde(rename = "type")]
    pub runtime_type: String,
    pub version: String,
    pub sha256: String,
    pub downloaded_at: String,
}

impl MorphLock {
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_config() {
        let json = r#"{"name":"test-app"}"#;
        let cfg = MorphConfig::parse_str(json).unwrap();
        assert_eq!(cfg.name, "test-app");
        assert_eq!(cfg.entry, "src/App.mx");
        assert_eq!(cfg.runtime.runtime_type, "cpp");
        assert_eq!(cfg.type_mode, "infer");
        assert_eq!(cfg.navigation.cache.capacity().unwrap(), 0);
    }

    #[test]
    fn parse_navigation_cache_forms() {
        let n = MorphConfig::parse_str(r#"{"navigation":{"cache":3}}"#).unwrap();
        assert_eq!(n.navigation.cache.capacity().unwrap(), 3);
        let all = MorphConfig::parse_str(r#"{"navigation":{"cache":"all"}}"#).unwrap();
        assert_eq!(all.navigation.cache.capacity().unwrap(), -1);
        let bad = MorphConfig::parse_str(r#"{"navigation":{"cache":"sometimes"}}"#).unwrap();
        assert!(bad.navigation.cache.capacity().is_err());
    }

    #[test]
    fn parse_window_roles() {
        assert_eq!(WindowRole::parse("").unwrap(), WindowRole::Default);
        assert_eq!(WindowRole::parse("default").unwrap(), WindowRole::Default);
        assert_eq!(WindowRole::parse("dialog").unwrap(), WindowRole::Dialog);
        assert_eq!(WindowRole::parse("popup").unwrap(), WindowRole::Popup);
        assert_eq!(WindowRole::Popup.as_int(), 2);
        assert!(WindowRole::parse("sheet").is_err());
    }

    #[test]
    fn parse_window_ownership_config() {
        let cfg = MorphConfig::parse_str(
            r#"{"window":{"width":640,"parent":"main","modal":true,"role":"popup"}}"#,
        )
        .unwrap();
        assert_eq!(cfg.window.parent, "main");
        assert!(cfg.window.modal);
        assert_eq!(WindowRole::parse(&cfg.window.role).unwrap(), WindowRole::Popup);
        let bare = MorphConfig::parse_str("{}").unwrap();
        assert_eq!(bare.window.parent, "");
        assert!(!bare.window.modal);
        assert_eq!(WindowRole::parse(&bare.window.role).unwrap(), WindowRole::Default);
    }

    #[test]
    fn parse_full_config() {
        let json = r#"{
            "name": "my-app",
            "entry": "src/App.mx",
            "runtime": {"type": "cpp", "version": "0.2.0"},
            "window": {"width": 1024, "height": 768, "title": "Hello"}
        }"#;
        let cfg = MorphConfig::parse_str(json).unwrap();
        assert_eq!(cfg.runtime.version, "0.2.0");
        assert_eq!(cfg.window.width, 1024);
    }

    #[test]
    fn parse_types_mode() {
        let cfg = MorphConfig::parse_str(r#"{"types":"strict"}"#).unwrap();
        assert_eq!(cfg.type_mode, "strict");
        let roundtrip: MorphConfig =
            serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert_eq!(roundtrip.type_mode, "strict");
    }

    #[test]
    fn default_config_serializes_runtime_block() {
        let json = MorphConfig::default().to_json_pretty().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["runtime"]["type"], "cpp");
        assert_eq!(parsed["runtime"]["version"], "0.1.0");
        let roundtrip = MorphConfig::parse_str(&json).unwrap();
        assert_eq!(roundtrip.runtime.version, "0.1.0");
    }
}
