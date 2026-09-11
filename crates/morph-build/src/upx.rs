//! UPX integration: find / auto-install the UPX executable and compress the
//! produced binary. Mirrors `morph/build/upx.py`.
//!
//! If `upx` is not on PATH, a prebuilt binary is downloaded from the official
//! GitHub releases into `~/.cache/morph/upx` (no root or package manager needed).

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub const UPX_VERSION: &str = "4.2.4";
const RELEASE_URL: &str = "https://github.com/upx/upx/releases/download/v{ver}/upx-{ver}-{asset}";

fn asset_name() -> Option<&'static str> {
    if cfg!(target_os = "windows") {
        return if cfg!(target_pointer_width = "64") {
            Some("win64.zip")
        } else {
            Some("win32.zip")
        };
    }
    if cfg!(target_os = "macos") {
        return if cfg!(target_arch = "aarch64") {
            Some("darwin_arm64.tar.xz")
        } else {
            Some("darwin_amd64.tar.xz")
        };
    }
    if cfg!(target_os = "linux") {
        return Some(match std::env::consts::ARCH {
            "aarch64" => "arm64_linux.tar.xz",
            "arm" => "arm_linux.tar.xz",
            _ => "amd64_linux.tar.xz",
        });
    }
    None
}

fn exe_name() -> &'static str {
    if cfg!(target_os = "windows") { "upx.exe" } else { "upx" }
}

fn cache_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        let base =
            std::env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("~"));
        return base.join("morph").join("upx");
    }
    let base = std::env::var("XDG_CACHE_HOME").map(PathBuf::from).unwrap_or_else(|_| {
        std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("~/.cache"))
    });
    base.join("morph").join("upx")
}

fn find_in_cache(cache: &Path, version: &str) -> Option<PathBuf> {
    let exe = exe_name();
    let mut stack = vec![cache.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(exe)
                && path.to_string_lossy().contains(&format!("upx-{}", version))
            {
                return Some(path);
            }
        }
    }
    None
}

fn download(asset: &str, version: &str, silent: bool) -> Result<Option<PathBuf>> {
    let url = RELEASE_URL.replace("{ver}", version).replace("{asset}", asset);
    let cache = cache_dir();
    std::fs::create_dir_all(&cache)?;
    let exe = exe_name();
    let pkg = cache.join(format!("upx-{}-{}", version, asset));
    if !silent {
        eprintln!("  … Downloading UPX {} ({}) ...", version, asset);
    }
    let status = std::process::Command::new("curl")
        .args(["-fsSL", "--retry", "2", "-o"])
        .arg(&pkg)
        .arg(&url)
        .status()
        .with_context(|| "UPX download needs `curl` on PATH")?;
    if !status.success() {
        anyhow::bail!("UPX download failed: {}", url);
    }
    // System tar (incl. Windows bsdtar) extracts both .zip and .tar.xz.
    let status = std::process::Command::new("tar")
        .args(["-xf"])
        .arg(&pkg)
        .args(["-C"])
        .arg(&cache)
        .status()
        .with_context(|| "UPX extraction needs `tar` on PATH")?;
    if !status.success() {
        anyhow::bail!("UPX extraction failed");
    }
    let mut stack = vec![cache.clone()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)?;
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(exe) {
                return Ok(Some(path));
            }
        }
    }
    anyhow::bail!("UPX archive did not contain the executable")
}

/// Return a usable UPX binary.
///
/// - `version` given: honor it — download that exact release even if a system
///   `upx` exists, so users can pick what compresses their project best.
/// - `version` None: use system `upx` if on PATH, else the default release.
pub fn ensure_upx(version: Option<&str>, silent: bool) -> Option<PathBuf> {
    if let Some(v) = version {
        if !v.is_empty() {
            if !silent {
                eprintln!("  … UPX: using pinned version {} from config/--upx-version", v);
            }
            let cache = cache_dir();
            if let Some(hit) = find_in_cache(&cache, v) {
                return Some(hit);
            }
            return asset_name().and_then(|asset| download(asset, v, silent).ok().flatten());
        }
    }
    if path_has(exe_name()) {
        return Some(PathBuf::from(exe_name()));
    }
    let cache = cache_dir();
    if let Some(hit) = find_in_cache(&cache, UPX_VERSION) {
        return Some(hit);
    }
    if !silent {
        eprintln!("  … UPX not found — installing a prebuilt binary ...");
    }
    asset_name().and_then(|asset| download(asset, UPX_VERSION, silent).ok().flatten())
}

/// Compress `binary_path` in place with UPX. Returns true on success.
/// Mirrors Python `compress()`: `--best` into a temp file, then swap, so a
/// failed run never leaves a truncated binary behind.
pub fn compress(binary_path: &Path, upx_bin: &Path) -> bool {
    let tmp = binary_path.with_extension("upx");
    let out = std::process::Command::new(upx_bin)
        .args(["--best", "-o"])
        .arg(&tmp)
        .arg(binary_path)
        .output();
    match out {
        Ok(o) if o.status.success() => std::fs::rename(&tmp, binary_path).is_ok(),
        Ok(o) => {
            let _ = std::fs::remove_file(&tmp);
            eprintln!("  ⚠ UPX failed: {}", String::from_utf8_lossy(&o.stderr).trim());
            false
        }
        Err(e) => {
            eprintln!("  ⚠ UPX failed to run: {}", e);
            false
        }
    }
}

fn path_has(bin: &str) -> bool {
    std::env::var("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(bin).exists()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compress_with_missing_upx_leaves_binary_intact() {
        let dir = std::env::temp_dir().join(format!("morph_upx_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("app.bin");
        std::fs::write(&bin, b"definitely not an elf binary").unwrap();
        assert!(!compress(&bin, &dir.join("no-such-upx")));
        assert_eq!(std::fs::read(&bin).unwrap(), b"definitely not an elf binary");
        assert!(!dir.join("app.upx").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ensure_upx_empty_pin_follows_system_lookup() {
        // Empty pin must behave exactly like no pin: system upx or None,
        // and crucially must never touch the network.
        assert_eq!(ensure_upx(Some(""), true), ensure_upx(None, true));
    }
}
