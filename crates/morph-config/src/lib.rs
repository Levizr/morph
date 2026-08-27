use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_title")]
    pub title: String,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { width: 800, height: 600, title: "Morph App".to_string() }
    }
}

fn default_width() -> u32 { 800 }
fn default_height() -> u32 { 600 }
fn default_title() -> String { "Morph App".to_string() }

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
    #[serde(default)]
    pub cxx: String,
    #[serde(default)]
    pub dev_cxx: String,
    #[serde(default)]
    pub cmake: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self { wayland: false, system_freetype: false, upx: true, upx_version: String::new(), cxx: String::new(), dev_cxx: String::new(), cmake: String::new() }
    }
}

fn default_true() -> bool { true }

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

fn default_runtime_type() -> String { "cpp".to_string() }
fn default_runtime_version() -> String { "0.1.0".to_string() }

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
}

fn default_name() -> String { "my-app".to_string() }
fn default_entry() -> String { "src/App.mx".to_string() }
fn default_output() -> String { ".morph/output".to_string() }

pub fn clean_app_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else if ch.is_whitespace() {
            out.push('_');
        } else {
            out.push('_');
        }
    }
    // Collapse multiple _ and trim
    let mut cleaned = String::new();
    let mut prev_us = false;
    for ch in out.chars() {
        if ch == '_' {
            if !prev_us { cleaned.push('_'); }
            prev_us = true;
        } else {
            cleaned.push(ch);
            prev_us = false;
        }
    }
    let cleaned = cleaned.trim_matches('_').to_string();
    if cleaned.is_empty() { "app".to_string() } else { cleaned }
}
fn default_renderer() -> String { "flash".to_string() }

impl Default for MorphConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            entry: default_entry(),
            output: default_output(),
            window: WindowConfig::default(),
            renderer: default_renderer(),
            dependencies: Default::default(),
            cpp_sources: Default::default(),
            native: NativeConfig::default(),
            node_bridge: false,
            build: BuildConfig::default(),
            lint: LintConfig::default(),
            runtime: RuntimeConfig::default(),
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

    pub fn from_str(s: &str) -> Result<Self> {
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
            anyhow::bail!("runtime.type must be 'cpp' or 'rust', got '{}'", self.runtime.runtime_type);
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

    pub fn from_str(s: &str) -> Result<Self> {
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
        let cfg = MorphConfig::from_str(json).unwrap();
        assert_eq!(cfg.name, "test-app");
        assert_eq!(cfg.entry, "src/App.mx");
        assert_eq!(cfg.runtime.runtime_type, "cpp");
    }

    #[test]
    fn parse_full_config() {
        let json = r#"{
            "name": "my-app",
            "entry": "src/App.mx",
            "runtime": {"type": "cpp", "version": "0.2.0"},
            "window": {"width": 1024, "height": 768, "title": "Hello"}
        }"#;
        let cfg = MorphConfig::from_str(json).unwrap();
        assert_eq!(cfg.runtime.version, "0.2.0");
        assert_eq!(cfg.window.width, 1024);
    }
}
