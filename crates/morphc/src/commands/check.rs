use anyhow::Result;
use std::path::PathBuf;

pub fn run(path: Option<PathBuf>, entry: Option<String>, migrate: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let mut mx_files: Vec<PathBuf> = Vec::new();
    let mut project_root: Option<PathBuf> = None;

    // ── If explicit PATH provided, resolve it ──
    if let Some(p) = path {
        let resolved = if p.is_absolute() { p.clone() } else { cwd.join(&p) };
        if !resolved.exists() {
            anyhow::bail!("Path not found: {}", p.display());
        }
        if resolved.is_file() {
            // Single file — check it regardless of extension (should be .mx)
            mx_files.push(resolved);
        } else if resolved.is_dir() {
            project_root = Some(resolved.clone());
            // If it's a project dir with src/, check src; else check dir itself
            let src_dir = resolved.join("src");
            let search_root = if src_dir.exists() { src_dir } else { resolved };
            for e in walkdir::WalkDir::new(&search_root)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if e.path().extension().map(|e| e == "mx").unwrap_or(false) {
                    mx_files.push(e.path().to_path_buf());
                }
            }
        }
    } else {
        // ── No explicit path — use project config in cwd ──
        let config_path = cwd.join("morph.config.json");
        if !config_path.exists() {
            // Fallback: if no config, just scan cwd/src
            let src_dir = cwd.join("src");
            if src_dir.exists() {
                for e in walkdir::WalkDir::new(&src_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if e.path().extension().map(|e| e == "mx").unwrap_or(false) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            } else {
                // No config and no src — also check cwd for .mx files
                for e in walkdir::WalkDir::new(&cwd)
                    .max_depth(2)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if e.path().extension().map(|e| e == "mx").unwrap_or(false) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            }
        } else {
            let config = morph_config::MorphConfig::from_file(&config_path)?;
            let entry_val = entry.unwrap_or(config.entry);
            // Still scan src for all .mx, but also ensure entry is covered
            let src_dir = cwd.join("src");
            if src_dir.exists() {
                for e in walkdir::WalkDir::new(&src_dir)
                    .into_iter()
                    .filter_map(|e| e.ok())
                {
                    if e.path().extension().map(|e| e == "mx").unwrap_or(false) {
                        mx_files.push(e.path().to_path_buf());
                    }
                }
            }
            // Ensure entry file is included if not already
            let entry_path = cwd.join(&entry_val);
            if entry_path.exists()
                && entry_path.extension().map(|e| e == "mx").unwrap_or(false)
                && !mx_files.contains(&entry_path)
            {
                mx_files.push(entry_path);
            }
            project_root = Some(cwd.clone());
        }
    }

    // Banner
    let display_root = project_root
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| cwd.display().to_string());
    crate::logger::log_banner("Morph Check — Lint .mx Files");
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
        crate::logger::log_warn("No .mx files found.");
        if let Some(root) = project_root {
            crate::logger::log_dim(&format!("Searched: {}", root.display()));
        } else {
            crate::logger::log_dim(&format!("Searched: {}/src", cwd.display()));
        }
        println!();
        return Ok(());
    }

    let mut errors = 0;
    let mut warnings = 0;

    for f in &mx_files {
        let content = std::fs::read_to_string(f)?;
        let filename = f
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        match morph_parser::parse_mx_str(&content, &filename) {
            Ok(source) => {
                let mut file_warnings = Vec::new();
                if source.components.is_empty() {
                    file_warnings.push("No component found (expected `export default function App()`)");
                }
                // morphState and morphEffect are current API — do not warn
                if file_warnings.is_empty() {
                    crate::logger::log_success(&format!(
                        "{} — OK ({} component(s), {} import(s))",
                        f.display(),
                        source.components.len(),
                        source.imports.len(),
                    ));
                } else {
                    for w in &file_warnings {
                        warnings += 1;
                        crate::logger::log_warn(&format!("{}: {}", f.display(), w));
                    }
                }
            }
            Err(e) => {
                errors += 1;
                crate::logger::log_error(&format!("{}: {}", f.display(), e));
            }
        }

        if migrate {
            // No migrations currently — morphState/morphEffect are not deprecated
        }
    }

    println!();
    if errors > 0 {
        crate::logger::log_error(&format!("{errors} file(s) with parse errors."));
    } else if warnings > 0 {
        crate::logger::log_warn(&format!(
            "{warnings} warning(s). Run `morph check --migrate` to auto-fix."
        ));
    } else {
        crate::logger::log_success(&format!("All {} file(s) passed.", mx_files.len()));
    }
    println!();

    Ok(())
}
