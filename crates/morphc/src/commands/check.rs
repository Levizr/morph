use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use morph_parser::LintError;

/// True for files Morph will lint (strict TS/TSX + .mx). JS-family files are
/// intentionally excluded so they don't silently pass.
fn is_source_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(morph_config::is_supported_source_ext)
}

/// Accept one explicitly selected file for checking. Unsupported extensions
/// are hard errors rather than silently skipped or linted.
fn push_source_file(files: &mut Vec<PathBuf>, path: PathBuf) -> Result<()> {
    if is_source_file(&path) {
        files.push(path);
        Ok(())
    } else {
        Err(anyhow::anyhow!(morph_config::validate_entry_ext(&path).unwrap_err()))
    }
}

fn cache_path(cwd: &Path) -> PathBuf {
    let proj_cache = cwd.join(".morph").join("lint_cache.json");
    if cwd.join(".morph").exists() {
        proj_cache
    } else {
        // fallback to global cache
        morph_cache::global_cache_root().map_or_else(
            |_| std::env::temp_dir().join("morph_lint_cache.json"),
            |p| p.join("cache").join("lint_cache.json"),
        )
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
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(cache) {
        let _ = std::fs::write(path, data);
    }
}

// Long lint pipeline: collect → cache → graph → report.
// Function kept whole to preserve check ordering.
#[allow(clippy::too_many_lines)]
pub(crate) fn run(path: Option<&PathBuf>, entry: Option<String>, _migrate: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut mx_files: Vec<PathBuf> = Vec::new();
    let mut project_root: Option<PathBuf> = None;

    if let Some(p) = path {
        let resolved = if p.is_absolute() { p.clone() } else { cwd.join(p) };
        if !resolved.exists() {
            anyhow::bail!("Path not found: {}", p.display());
        }
        if resolved.is_file() {
            push_source_file(&mut mx_files, resolved)?;
        } else if resolved.is_dir() {
            project_root = Some(resolved.clone());
            let src_dir = resolved.join("src");
            let search_root = if src_dir.exists() { src_dir } else { resolved };
            for e in
                walkdir::WalkDir::new(&search_root).into_iter().filter_map(std::result::Result::ok)
            {
                if is_source_file(e.path()) {
                    mx_files.push(e.path().to_path_buf());
                }
            }
        }
    } else {
        let config_path = cwd.join("morph.config.json");
        if config_path.exists() {
            let config = morph_config::MorphConfig::from_file(&config_path)?;
            let entry_val = entry.unwrap_or(config.entry);
            let src_dir = cwd.join("src");
            if src_dir.exists() {
                for e in
                    walkdir::WalkDir::new(&src_dir).into_iter().filter_map(std::result::Result::ok)
                {
                    if is_source_file(e.path()) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            }
            let entry_path = cwd.join(&entry_val);
            if entry_path.exists() && is_source_file(&entry_path) && !mx_files.contains(&entry_path)
            {
                mx_files.push(entry_path);
            }
            project_root = Some(cwd.clone());
        } else {
            let src_dir = cwd.join("src");
            if src_dir.exists() {
                for e in
                    walkdir::WalkDir::new(&src_dir).into_iter().filter_map(std::result::Result::ok)
                {
                    if is_source_file(e.path()) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            } else {
                for e in walkdir::WalkDir::new(&cwd)
                    .max_depth(2)
                    .into_iter()
                    .filter_map(std::result::Result::ok)
                {
                    if is_source_file(e.path()) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            }
        }
    }

    let display_root = project_root
        .as_ref()
        .map_or_else(|| cwd.display().to_string(), |p| p.display().to_string());
    crate::logger::log_banner("Morph Check — Lint Source Files");
    if mx_files.len() == 1 {
        crate::logger::log_step(&format!("Checking {}", mx_files[0].display()));
    } else {
        crate::logger::log_step(&format!(
            "Checking {} file(s) in {}",
            mx_files.len(),
            display_root
        ));
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
        // Bump when lint rules change so stale diagnostics are re-computed.
        let hash = morph_cache::sha256_bytes(format!("v4:{content}").as_bytes());
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

    // ── Cross-file component validation over the resolved module graph ──
    // Single-file lints accept imported names without prop checks; the graph
    // pass resolves them to declarations so prop mismatches surface here.
    if let Some(entry_path) = resolve_check_entry(&cwd, path) {
        if let Ok(graph) = morph_parser::resolve_graph(&entry_path, &cwd) {
            // Only the entry module renders as a window; component files
            // must not carry `windowConfig`, so drop that hint for them.
            all_errors.retain(|e| {
                if e.code != "mx-window-missing" {
                    return true;
                }
                let canon = std::fs::canonicalize(&e.file_path)
                    .unwrap_or_else(|_| PathBuf::from(&e.file_path));
                !graph.all_paths().any(|p| *p == canon && *p != graph.entry)
            });
            for err in morph_parser::linter::lint_graph(&graph) {
                let duplicate = all_errors.iter().any(|e| {
                    e.code == err.code
                        && e.file_path == err.file_path
                        && e.line == err.line
                        && e.col == err.col
                        && e.message == err.message
                });
                if !duplicate {
                    all_errors.push(err);
                }
            }
        }
    }

    // ── Per-file success for clean files ──
    for f in &ok_files {
        // Get component/import counts for nice message
        let content = contents.get(&f.display().to_string()).unwrap();
        let filename = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        if let Ok(src) = morph_parser::parse_mx_str(content, &filename) {
            crate::logger::log_success(&format!(
                "{} — OK ({} component(s), {} import(s))",
                f.display(),
                src.components.len(),
                src.imports.len()
            ));
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
            crate::logger::log_error(&format!(
                "{n_err} error(s), {n_warn} warning(s) — fix and save to hot reload"
            ));
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

/// Entry file for graph-aware checks: an explicitly selected supported source
/// wins, otherwise the project config entry.
fn resolve_check_entry(cwd: &Path, path: Option<&PathBuf>) -> Option<PathBuf> {
    if let Some(p) = path {
        let resolved = if p.is_absolute() { p.clone() } else { cwd.join(p) };
        if is_source_file(&resolved) && resolved.is_file() {
            return Some(resolved);
        }
    }
    let config_path = cwd.join("morph.config.json");
    let config = morph_config::MorphConfig::from_file(&config_path).ok()?;
    let entry_path = cwd.join(&config.entry);
    if is_source_file(&entry_path) && entry_path.is_file() {
        Some(entry_path)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_check_project(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("morph_check_test_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        root
    }

    #[test]
    fn explicit_js_source_is_a_hard_error() {
        let root = scratch_check_project("js");
        let path = root.join("src/app.js");
        std::fs::write(&path, "let x = 1;\n").unwrap();
        let mut files = Vec::new();
        let err = push_source_file(&mut files, path).unwrap_err();
        assert!(err.to_string().contains("app.js"), "{err}");
        assert!(files.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn check_entry_accepts_typescript_but_not_javascript() {
        let root = scratch_check_project("entry");
        let ts_entry = root.join("src/App.tsx");
        let js_entry = root.join("src/app.js");
        std::fs::write(&ts_entry, "export default function App() { return <div/> }\n").unwrap();
        std::fs::write(&js_entry, "let x = 1;\n").unwrap();
        std::fs::write(root.join("morph.config.json"), r#"{"entry": "src/app.js"}"#).unwrap();
        assert_eq!(resolve_check_entry(&root, Some(&PathBuf::from("src/App.tsx"))), Some(ts_entry));
        assert_eq!(resolve_check_entry(&root, Some(&PathBuf::from("src/app.js"))), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
