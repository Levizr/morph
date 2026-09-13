//! `morph_devrt` build gating and launch.
//!
//! Mirrors `morph/dev/devrt.py`: the dev runtime is a CMake project at
//! `<runtime>/dev`. It is rebuilt only when its sources change (hash-gated),
//! then spawned with piped stdio so the dev driver can stream logs and
//! parse the socket announcement.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

/// Source subdirectories (under the runtime dir) covered by the rebuild
/// hash, mirroring Python `_compute_source_hash`.
const HASHED_SUBDIRS: &[&str] = &["dev", "core", "render", "renderers", "ui", "style", "vendor"];

/// File names covered by the rebuild hash (case-sensitive, like the
/// compiler's own suffix matching).
fn is_hashed_file(name: &str) -> bool {
    if name == "CMakeLists.txt" {
        return true;
    }
    matches!(Path::new(name).extension().and_then(|ext| ext.to_str()), Some("cpp" | "h" | "hpp"))
}

/// Hash of the dev runtime sources: sorted `rel-path + NUL + contents`
/// over every hashed file. Any runtime change rebuilds the binary (and
/// invalidates logic libraries built against it).
pub fn compute_source_hash(dev_dir: &Path, runtime_dir: &Path) -> Result<String> {
    let mut paths = Vec::new();
    for sub in HASHED_SUBDIRS {
        let scan = runtime_dir.join(sub);
        if !scan.is_dir() {
            continue;
        }
        collect_hashed_files(&scan, &mut paths)?;
    }
    paths.sort();
    let mut hasher = Sha256::new();
    for path in &paths {
        let rel = path.strip_prefix(dev_dir).unwrap_or(path);
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0u8]);
        let bytes = std::fs::read(path)
            .with_context(|| format!("reading runtime source {}", path.display()))?;
        hasher.update(&bytes);
    }
    Ok(hex_digest(hasher))
}

/// Stored-hash sidecar next to the dev sources (mirrors `devrt.py`).
pub fn stored_hash_path(dev_dir: &Path) -> PathBuf {
    dev_dir.join(".devrt_source_hash")
}

fn collect_hashed_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("listing {}", dir.display()))?;
    let mut entries: Vec<_> = entries
        .collect::<std::io::Result<_>>()
        .with_context(|| format!("reading directory entries under {}", dir.display()))?;
    entries.sort_by_key(std::fs::DirEntry::path);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_hashed_files(&path, out)?;
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(is_hashed_file) {
            out.push(path);
        }
    }
    Ok(())
}

fn hex_digest(hasher: Sha256) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Resolve the cmake binary: explicit config value → `MORPH_CMAKE` → PATH.
pub fn resolve_cmake(configured: &str) -> String {
    if !configured.is_empty() {
        return configured.to_string();
    }
    std::env::var("MORPH_CMAKE")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "cmake".to_string())
}

/// Dev runtime binary path: `<runtime>/bin/morph_devrt[.exe]`, matching
/// `MORPH_BIN_DIR` in `runtime/cpp/dev/CMakeLists.txt`.
pub fn binary_path(dev_dir: &Path) -> PathBuf {
    dev_dir.join("../../bin").join(format!("morph_devrt{}", crate::platform::exe_suffix()))
}

/// Build the dev runtime when missing or stale (source hash changed).
/// Returns the binary path.
pub fn ensure_built(
    cmake_bin: &str,
    dev_dir: &Path,
    runtime_dir: &Path,
    binary: &Path,
) -> Result<PathBuf> {
    let current = compute_source_hash(dev_dir, runtime_dir)?;
    let stored =
        std::fs::read_to_string(stored_hash_path(dev_dir)).ok().map(|s| s.trim().to_string());
    if binary.exists() && stored.as_deref() == Some(current.as_str()) {
        return Ok(binary.to_path_buf());
    }
    let build_dir = dev_dir.join("build");
    std::fs::create_dir_all(&build_dir)
        .with_context(|| format!("creating {build_dir}", build_dir = build_dir.display()))?;
    let configure = Command::new(cmake_bin)
        .args(["-S", &dev_dir.display().to_string(), "-B", &build_dir.display().to_string()])
        .status()
        .with_context(|| format!("running cmake binary '{cmake_bin}'"))?;
    if !configure.success() {
        anyhow::bail!("cmake configure failed: {configure}");
    }
    let build = Command::new(cmake_bin)
        .args(["--build", &build_dir.display().to_string(), "--parallel"])
        .status()
        .with_context(|| format!("running cmake binary '{cmake_bin}'"))?;
    if !build.success() {
        anyhow::bail!("dev runtime build failed: {build}");
    }
    std::fs::write(stored_hash_path(dev_dir), &current)
        .with_context(|| format!("writing {}", stored_hash_path(dev_dir).display()))?;
    if !binary.exists() {
        anyhow::bail!("dev runtime build succeeded but {} is missing", binary.display());
    }
    Ok(binary.to_path_buf())
}

/// Spawn the dev runtime with piped stdio, working directory `cwd`.
pub fn launch(binary: &Path, cwd: &Path) -> Result<Child> {
    Command::new(binary)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("spawning dev runtime {}", binary.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_file_names() {
        assert!(is_hashed_file("main.cpp"));
        assert!(is_hashed_file("node.h"));
        assert!(is_hashed_file("types.hpp"));
        assert!(is_hashed_file("CMakeLists.txt"));
        assert!(!is_hashed_file("build"));
        assert!(!is_hashed_file("README.md"));
        assert!(!is_hashed_file("logo.png"));
    }

    #[test]
    fn resolve_cmake_prefers_config_then_env() {
        assert_eq!(resolve_cmake("/opt/cmake"), "/opt/cmake");
        std::env::remove_var("MORPH_CMAKE");
        assert_eq!(resolve_cmake(""), "cmake");
        std::env::set_var("MORPH_CMAKE", "/env/cmake");
        assert_eq!(resolve_cmake(""), "/env/cmake");
        std::env::remove_var("MORPH_CMAKE");
    }

    #[test]
    fn source_hash_changes_with_contents() {
        let root = std::env::temp_dir().join("morph_devrt_hash_test");
        let _ = std::fs::remove_dir_all(&root);
        let dev = root.join("dev");
        std::fs::create_dir_all(dev.join("core")).unwrap();
        std::fs::write(dev.join("core").join("a.cpp"), "int x;").unwrap();
        let first = compute_source_hash(&dev, &root).unwrap();
        std::fs::write(dev.join("core").join("a.cpp"), "int y;").unwrap();
        let second = compute_source_hash(&dev, &root).unwrap();
        assert_ne!(first, second);
        let _ = std::fs::remove_dir_all(&root);
    }
}
