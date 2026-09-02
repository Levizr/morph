use anyhow::{Context, Result};
use morph_config::{MorphLock, VersionFile};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Global cache root: ~/.morph/
pub fn global_cache_root() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not find home directory")?;
    Ok(home.join(".morph"))
}

pub fn global_runtimes_dir() -> Result<PathBuf> {
    Ok(global_cache_root()?.join("cache").join("runtimes"))
}

pub fn global_runtime_version_dir(runtime_type: &str, version: &str) -> Result<PathBuf> {
    Ok(global_runtimes_dir()?.join(runtime_type).join(format!("v{}", version)))
}

/// Check if a runtime version is cached globally
pub fn is_runtime_cached(runtime_type: &str, version: &str) -> bool {
    if let Ok(dir) = global_runtime_version_dir(runtime_type, version) {
        dir.join("manifest.json").exists() || dir.join("include").exists()
    } else {
        false
    }
}

/// Write a manifest inside cached runtime for verification
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct RuntimeManifest {
    pub version: String,
    pub runtime_type: String,
    pub sha256: String,
    pub size: u64,
    pub cached_at: String,
}

/// Compute sha256 of a file
pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok(hex::encode(h.finalize()))
}

/// Compute sha256 of bytes
pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

/// Download runtime from GitHub Releases
pub fn download_runtime(runtime_type: &str, version: &str) -> Result<PathBuf> {
    let cached_dir = global_runtime_version_dir(runtime_type, version)?;

    if is_runtime_cached(runtime_type, version) {
        println!("  ✓ Found in cache: {}", cached_dir.display());
        return Ok(cached_dir);
    }

    // Try local runtime/ directory first (for development)
    let local_runtime = find_local_runtime(runtime_type);
    if let Some(local) = local_runtime {
        println!("  ℹ Using local runtime from {}", local.display());
        cache_local_runtime(&local, &cached_dir, runtime_type, version)?;
        return Ok(cached_dir);
    }

    // Download from GitHub
    let url = format!(
        "https://github.com/Levizr/morph/releases/download/runtime-{}-v{}/morph-runtime-{}-v{}.tar.gz",
        runtime_type, version, runtime_type, version
    );

    println!("  Downloading {} v{} from GitHub...", runtime_type, version);
    println!("  URL: {}", url);

    let bytes = download_bytes(&url).with_context(|| format!("failed to download runtime {} v{} — check that release exists or use a local runtime/", runtime_type, version))?;

    // Verify not HTML error page
    if bytes.starts_with(b"<!DOCTYPE") || bytes.starts_with(b"<html") {
        anyhow::bail!("download returned HTML (release not found): {}", url);
    }

    std::fs::create_dir_all(&cached_dir)?;
    extract_tar_gz(&bytes, &cached_dir)?;

    // Write manifest
    let manifest = RuntimeManifest {
        version: version.to_string(),
        runtime_type: runtime_type.to_string(),
        sha256: sha256_bytes(&bytes),
        size: bytes.len() as u64,
        cached_at: chrono_string(),
    };
    std::fs::write(
        cached_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    println!("  ✓ Cached to {}", cached_dir.display());
    Ok(cached_dir)
}

fn chrono_string() -> String {
    // Simple timestamp without chrono dependency
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_default()
}

fn find_local_runtime(runtime_type: &str) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from(format!("runtime/{}", runtime_type)),
        PathBuf::from(format!("../runtime/{}", runtime_type)),
        PathBuf::from(format!("../../runtime/{}", runtime_type)),
        PathBuf::from(format!("morph/runtime/{}", runtime_type)),
    ];

    // Check executable-relative path (for installed binary)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join(format!("../runtime/{}", runtime_type)));
            candidates.push(exe_dir.join(format!("../../runtime/{}", runtime_type)));
            candidates.push(exe_dir.join(format!("../../../runtime/{}", runtime_type)));
        }
    }

    // Development fallback: playground absolute path
    candidates.push(PathBuf::from(format!("/home/piyush/My_Projects/playground/morph/runtime/{}", runtime_type)));
    candidates.push(PathBuf::from(format!("/home/piyush/My_Projects/morph/runtime/{}", runtime_type)));

    // Check ancestors of current dir (up to 4 levels)
    if let Ok(cwd) = std::env::current_dir() {
        let mut cur = cwd.clone();
        for _ in 0..4 {
            candidates.push(cur.join(format!("runtime/{}", runtime_type)));
            if let Some(parent) = cur.parent() {
                cur = parent.to_path_buf();
            } else {
                break;
            }
        }
    }

    for p in candidates {
        if p.join("morph_api.h").exists() || p.join("include").exists() || p.exists() && p.is_dir() && std::fs::read_dir(&p).map(|mut d| d.next().is_some()).unwrap_or(false) {
            return Some(p);
        }
    }
    None
}

fn cache_local_runtime(src: &Path, dest: &Path, runtime_type: &str, version: &str) -> Result<()> {
    std::fs::create_dir_all(dest)?;

    // If src is a directory, copy contents
    if src.is_dir() {
        copy_dir_recursive(src, dest)?;
    }

    // Check if we actually copied something
    if !dest.exists() || std::fs::read_dir(dest)?.next().is_none() {
        // Create stub if local runtime is empty/missing
        std::fs::write(dest.join("README.md"), format!("# Morph Runtime {} v{} (stub)\n\nLocal runtime not yet populated. This is a placeholder.\n", runtime_type, version))?;
    }

    let manifest = RuntimeManifest {
        version: version.to_string(),
        runtime_type: runtime_type.to_string(),
        sha256: "local".to_string(),
        size: 0,
        cached_at: chrono_string(),
    };
    std::fs::write(dest.join("manifest.json"), serde_json::to_string_pretty(&manifest)?)?;
    Ok(())
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    for entry in WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src)?;
        let dest_path = dest.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest_path)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &dest_path)?;
        }
    }
    Ok(())
}

fn download_bytes(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("morphc/0.1.0")
        .build()?;

    let resp = client.get(url).send()?;

    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {}", resp.status(), url);
    }

    Ok(resp.bytes()?.to_vec())
}

fn extract_tar_gz(bytes: &[u8], dest: &Path) -> Result<()> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(dest)?;
    Ok(())
}

/// Link or copy global cache to project .morph/runtime
pub fn link_runtime_to_project(
    runtime_type: &str,
    version: &str,
    project_morph_dir: &Path,
) -> Result<PathBuf> {
    let cached_dir = global_runtime_version_dir(runtime_type, version)?;
    let project_runtime = project_morph_dir.join("runtime");

    // Remove existing
    if project_runtime.exists() {
        if project_runtime.is_symlink() {
            std::fs::remove_file(&project_runtime)?;
        } else {
            std::fs::remove_dir_all(&project_runtime)?;
        }
    }

    // Try symlink, fallback to copy
    #[cfg(unix)]
    {
        if std::os::unix::fs::symlink(&cached_dir, &project_runtime).is_ok() {
            return Ok(project_runtime);
        }
    }

    // Fallback: copy
    copy_dir_recursive(&cached_dir, &project_runtime)?;
    Ok(project_runtime)
}

/// Fetch latest version from GitHub via versions/ files or API
pub fn fetch_latest_runtime_version(runtime_type: &str) -> Result<String> {
    // First try local versions file
    let local_version_file = PathBuf::from(format!("versions/runtime/{}.json", runtime_type));
    if local_version_file.exists() {
        let vf = VersionFile::from_file(&local_version_file)?;
        return Ok(vf.version);
    }

    // Try GitHub API (placeholder — not yet implemented)
    let _url = format!(
        "https://api.github.com/repos/Levizr/morph/releases/tags/runtime-{}-v{}",
        runtime_type, "latest"
    );
    // For now, fallback to reading morph-config default
    anyhow::bail!("could not determine latest version for {}", runtime_type)
}

/// Check if project has runtime installed
pub fn is_project_runtime_installed(project_morph_dir: &Path) -> bool {
    let runtime = project_morph_dir.join("runtime");
    runtime.exists() && (runtime.join("manifest.json").exists() || runtime.join("include").exists() || runtime.join("morph_api.h").exists() || std::fs::read_dir(&runtime).map(|mut d| d.next().is_some()).unwrap_or(false))
}

/// Hash all files under `dir` (relative path + content). Returns an empty
/// string if the directory does not exist. Hidden/.git contents are skipped so
/// unrelated files (e.g. cached build artifacts) don't force rebuilds.
pub fn hash_tree(dir: &Path) -> String {
    if !dir.is_dir() {
        return String::new();
    }
    let mut entries: Vec<String> = Vec::new();
    for entry in WalkDir::new(dir).follow_links(false).min_depth(1) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.file_type().is_dir() {
            continue;
        }
        let rel = match entry.path().strip_prefix(dir) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };
        if rel.split('/').any(|c| c == ".git" || c.starts_with('.')) {
            continue;
        }
        let content = std::fs::read(entry.path()).unwrap_or_default();
        let mut h = Sha256::new();
        h.update(&content);
        entries.push(format!("{}:{}", rel, hex::encode(h.finalize())));
    }
    entries.sort();
    sha256_string(&entries.join("\n"))
}

/// Project build-hash dir: <cwd>/.morph/hash
pub fn project_hash_dir(cwd: &Path) -> PathBuf {
    cwd.join(".morph").join("hash")
}

/// Compute sha256 of a string
pub fn sha256_string(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

/// Compose a fingerprint over a set of (relative_path, content) inputs.
/// The hashes are fed through a final digest so file additions/removals and
/// ordering changes are reflected. `inputs` earlier in the slice must map to
/// distinct paths; content may be empty for files that were deleted.
pub fn fingerprint_inputs(inputs: &[(&str, &str)]) -> String {
    let mut entries: Vec<String> = inputs
        .iter()
        .map(|(p, c)| format!("{}:{}\n", p, sha256_string(c)))
        .collect();
    entries.sort();
    sha256_string(&entries.join(""))
}

/// Read the previously stored fingerprint for `binary_name`, if any.
pub fn read_stored_fingerprint(cwd: &Path, binary_name: &str) -> Option<String> {
    let dir = project_hash_dir(cwd);
    let file = dir.join(format!("{}.fingerprint", binary_name));
    std::fs::read_to_string(&file).ok().map(|s| s.trim().to_string())
}

/// Store the fingerprint for `binary_name` under <cwd>/.morph/hash.
pub fn write_stored_fingerprint(cwd: &Path, binary_name: &str, fingerprint: &str) -> Result<()> {
    let dir = project_hash_dir(cwd);
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(format!("{}.fingerprint", binary_name)), fingerprint)?;
    Ok(())
}

/// Create morph.lock file
pub fn write_lock_file(
    project_dir: &Path,
    runtime_type: &str,
    version: &str,
    sha256: &str,
) -> Result<()> {
    let lock = MorphLock {
        runtime: morph_config::LockRuntime {
            runtime_type: runtime_type.to_string(),
            version: version.to_string(),
            sha256: sha256.to_string(),
            downloaded_at: chrono_string(),
        },
        generated_by: format!("morphc {}", env!("CARGO_PKG_VERSION")),
        generated_at: chrono_string(),
    };
    let path = project_dir.join("morph.lock");
    std::fs::write(&path, serde_json::to_string_pretty(&lock)?)?;
    Ok(())
}


