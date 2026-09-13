//! Dev logic library orchestration.
//!
//! Mirrors `morph/dev/pipeline.py` (`compile_logic`): the generated logic
//! source is hashed together with imported user `.cpp` files and the
//! runtime-headers hash; an unchanged hash skips compilation, otherwise a
//! content-addressed `logic.<hash>.<ext>` library is compiled (via a temp
//! file + atomic rename) and old libraries are pruned.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::{Compiler, NativeFlags, shared_lib_ext};

/// In-memory hot-reload state: last compiled hash and library path.
#[derive(Debug, Default)]
pub struct LogicSession {
    last_hash: Option<String>,
    last_so_path: Option<PathBuf>,
}

/// Maximum retained logic libraries per cache dir.
const KEEP_LIBRARIES: usize = 3;

/// Compile the dev logic library when its inputs changed.
///
/// Returns the library path. Mirrors Python `compile_logic`, which compiles
/// unconditionally whenever windows exist (even a trivial library keeps
/// the runtime's dlopen path uniform).
#[allow(clippy::too_many_arguments)]
pub fn compile_logic(
    session: &mut LogicSession,
    compiler: &Compiler,
    runtime_dir: &Path,
    cache_dir: &Path,
    logic_source: &str,
    user_sources: &[PathBuf],
    runtime_hash: &str,
    native: &NativeFlags,
) -> Result<PathBuf> {
    std::fs::create_dir_all(cache_dir)
        .with_context(|| format!("creating logic cache {}", cache_dir.display()))?;
    let source_path = cache_dir.join("app_logic.cpp");
    std::fs::write(&source_path, logic_source)
        .with_context(|| format!("writing {}", source_path.display()))?;

    let mut hasher = Sha256::new();
    hasher.update(logic_source.as_bytes());
    for user in user_sources {
        if !user.exists() {
            continue;
        }
        let bytes = std::fs::read(user)
            .with_context(|| format!("reading user source {}", user.display()))?;
        hasher.update(&bytes);
    }
    hasher.update(runtime_hash.as_bytes());
    use std::fmt::Write as _;
    let mut cur_hash = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(cur_hash, "{byte:02x}");
    }

    if session.last_hash.as_deref() == Some(cur_hash.as_str()) {
        if let Some(path) = session.last_so_path.clone() {
            return Ok(path);
        }
    }
    let so_name = format!("logic.{}{}", &cur_hash[..16], shared_lib_ext());
    let so_path = cache_dir.join(&so_name);
    if so_path.exists() {
        session.last_hash = Some(cur_hash);
        session.last_so_path = Some(so_path.clone());
        return Ok(so_path);
    }
    let tmp_path = cache_dir.join(format!("{}.tmp.{}", so_name, std::process::id()));
    compiler.compile_shared(&source_path, &tmp_path, runtime_dir, &[], native)?;
    std::fs::rename(&tmp_path, &so_path)
        .with_context(|| format!("renaming {} to {}", tmp_path.display(), so_path.display()))?;
    prune_old_libraries(cache_dir, &so_name);
    session.last_hash = Some(cur_hash);
    session.last_so_path = Some(so_path.clone());
    Ok(so_path)
}

/// Remove old `logic.<hash>.<ext>` libraries, keeping the newest few.
fn prune_old_libraries(cache_dir: &Path, keep_name: &str) {
    let ext = shared_lib_ext();
    let mut olds: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
    let entries = std::fs::read_dir(cache_dir).map(|r| {
        r.filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n != keep_name && n.starts_with("logic.") && n.ends_with(ext))
            })
            .collect::<Vec<_>>()
    });
    let Ok(paths) = entries else { return };
    for path in paths {
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if let Some(mtime) = mtime {
            olds.push((mtime, path));
        }
    }
    olds.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    for (_, path) in olds.into_iter().skip(KEEP_LIBRARIES) {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_and_reuses_by_hash() {
        let dir = std::env::temp_dir().join("morph_logic_test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut session = LogicSession::default();
        let compiler = Compiler { gpp: "g++".to_string(), silent: true };
        // Skip when no compiler is available (CI without a toolchain).
        if std::process::Command::new("g++").arg("--version").output().is_err() {
            return;
        }
        let source = "extern \"C\" { void morph_logic_init() {} void morph_logic_rewire() {} void morph_logic_cleanup() {} }\n";
        let first = compile_logic(
            &mut session,
            &compiler,
            Path::new("/rt"),
            &dir,
            source,
            &[],
            "runtime-hash",
            &NativeFlags::default(),
        )
        .unwrap();
        assert!(first.exists());
        // Second call with identical inputs reuses the library (mtime kept).
        let mtime = std::fs::metadata(&first).unwrap().modified().unwrap();
        let second = compile_logic(
            &mut session,
            &compiler,
            Path::new("/rt"),
            &dir,
            source,
            &[],
            "runtime-hash",
            &NativeFlags::default(),
        )
        .unwrap();
        assert_eq!(first, second);
        assert_eq!(std::fs::metadata(&first).unwrap().modified().unwrap(), mtime);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
