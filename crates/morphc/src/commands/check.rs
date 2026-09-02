use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use morph_parser::LintError;

/// True for files Morph will lint (strict TS/TSX + .mx). JS-family files are
/// intentionally excluded so they don't silently pass.
fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| morph_config::is_supported_source_ext(e))
        .unwrap_or(false)
}

fn cache_path(cwd: &Path) -> PathBuf {
    let proj_cache = cwd.join(".morph").join("lint_cache.json");
    if cwd.join(".morph").exists() {
        proj_cache
    } else {
        // fallback to global cache
        morph_cache::global_cache_root()
            .map(|p| p.join("cache").join("lint_cache.json"))
            .unwrap_or_else(|_| std::env::temp_dir().join("morph_lint_cache.json"))
    }
}

fn load_cache(path: &Path) -> HashMap<String, (String, Vec<LintError>)> {
    if let Ok(data) = std::fs::read_to_string(path) {
        if let Ok(map) = serde_json::from_str(&data) {
            return map;
        }
    }
    HashMap::new()
}

fn save_cache(path: &Path, cache: &HashMap<String, (String, Vec<LintError>)>) {
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(data) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(path, data);
    }
}

pub fn run(path: Option<PathBuf>, entry: Option<String>, _migrate: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut mx_files: Vec<PathBuf> = Vec::new();
    let mut project_root: Option<PathBuf> = None;

    if let Some(p) = path {
        let resolved = if p.is_absolute() { p.clone() } else { cwd.join(&p) };
        if !resolved.exists() {
            anyhow::bail!("Path not found: {}", p.display());
        }
        if resolved.is_file() {
            mx_files.push(resolved);
        } else if resolved.is_dir() {
            project_root = Some(resolved.clone());
            let src_dir = resolved.join("src");
            let search_root = if src_dir.exists() { src_dir } else { resolved };
            for e in walkdir::WalkDir::new(&search_root).into_iter().filter_map(|e| e.ok()) {
                if e.path().extension().map(|e| e == "mx").unwrap_or(false) {
                    mx_files.push(e.path().to_path_buf());
                }
            }
        }
    } else {
        let config_path = cwd.join("morph.config.json");
        if !config_path.exists() {
            let src_dir = cwd.join("src");
            if src_dir.exists() {
                for e in walkdir::WalkDir::new(&src_dir).into_iter().filter_map(|e| e.ok()) {
                    if is_source_file(e.path()) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            } else {
                for e in walkdir::WalkDir::new(&cwd).max_depth(2).into_iter().filter_map(|e| e.ok()) {
                    if is_source_file(e.path()) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            }
        } else {
            let config = morph_config::MorphConfig::from_file(&config_path)?;
            let entry_val = entry.unwrap_or(config.entry);
            let src_dir = cwd.join("src");
            if src_dir.exists() {
                for e in walkdir::WalkDir::new(&src_dir).into_iter().filter_map(|e| e.ok()) {
                    if is_source_file(e.path()) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            }
            let entry_path = cwd.join(&entry_val);
            if entry_path.exists() && is_source_file(&entry_path) && !mx_files.contains(&entry_path) {
                mx_files.push(entry_path);
            }
            project_root = Some(cwd.clone());
        }
    }

    let display_root = project_root.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| cwd.display().to_string());
    crate::logger::log_banner("Morph Check — Lint Source Files");
    if mx_files.len() == 1 {
        crate::logger::log_step(&format!("Checking {}", mx_files[0].display()));
    } else {
        crate::logger::log_step(&format!("Checking {} file(s) in {}", mx_files.len(), display_root));
    }

    if mx_files.is_empty() {
        crate::logger::log_warn("No supported source files found (.ts/.tsx/.mx).");
        if let Some(root) = project_root {
            crate::logger::log_dim(&format!("Searched: {}", root.display()));
        } else {
            crate::logger::log_dim(&format!("Searched: {}/src", cwd.display()));
        }
        println!();
        return Ok(());
    }

    // ── Lint cache (JSX errors) ──
    let cpath = cache_path(&cwd);
    let mut cache = load_cache(&cpath);
    let mut cache_dirty = false;
    let mut all_errors: Vec<LintError> = Vec::new();
    let mut contents: HashMap<String, String> = HashMap::new();
    let mut ok_files = Vec::new();

    for f in &mx_files {
        let content = std::fs::read_to_string(f)?;
        let hash = morph_cache::sha256_bytes(content.as_bytes());
        let key = f.display().to_string();
        contents.insert(key.clone(), content.clone());

        let errors = if let Some((cached_hash, cached_errors)) = cache.get(&key) {
            if cached_hash == &hash {
                // cache hit — reuse
                cached_errors.clone()
            } else {
                let errs = morph_parser::linter::check(&content, &key);
                cache.insert(key.clone(), (hash.clone(), errs.clone()));
                cache_dirty = true;
                errs
            }
        } else {
            let errs = morph_parser::linter::check(&content, &key);
            cache.insert(key.clone(), (hash.clone(), errs.clone()));
            cache_dirty = true;
            errs
        };

        if errors.is_empty() {
            ok_files.push(f.clone());
        } else {
            all_errors.extend(errors);
        }
    }

    if cache_dirty {
        save_cache(&cpath, &cache);
    }

    // ── Per-file success for clean files ──
    for f in &ok_files {
        // Get component/import counts for nice message
        let content = contents.get(&f.display().to_string()).unwrap();
        let filename = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if let Ok(src) = morph_parser::parse_mx_str(content, &filename) {
            crate::logger::log_success(&format!("{} — OK ({} component(s), {} import(s))", f.display(), src.components.len(), src.imports.len()));
        } else {
            crate::logger::log_success(&format!("{} — OK", f.display()));
        }
    }

    // ── Display aggregated lint errors with Python-style code frames ──
    if !all_errors.is_empty() {
        crate::logger::log_lint_errors(&all_errors, &contents);
        let n_err = all_errors.iter().filter(|e| e.severity == "error").count();
        let n_warn = all_errors.len() - n_err;
        println!();
        if n_err > 0 {
            crate::logger::log_error(&format!("{n_err} error(s), {n_warn} warning(s) — fix and save to hot reload"));
            // Return error to block build pipeline (like Python)
            // But for `morph check` we don't fail, just report
        } else {
            crate::logger::log_warn(&format!("{n_warn} warning(s)."));
        }
        println!();
        return Ok(());
    }

    println!();
    crate::logger::log_success(&format!("All {} file(s) passed.", mx_files.len()));
    println!();
    Ok(())
}
