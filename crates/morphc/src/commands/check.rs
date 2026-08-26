use anyhow::Result;
use colored::Colorize;

pub fn run(entry: Option<String>, migrate: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry = entry.unwrap_or(config.entry);

    crate::logger::log_banner("Morph Check — Lint .mx Files");
    crate::logger::log_step(&format!("Checking {}", entry));

    // Find .mx files
    let src_dir = cwd.join("src");
    let mut mx_files = vec![];
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

    if mx_files.is_empty() {
        crate::logger::log_warn("No .mx files found in src/");
        println!();
        return Ok(());
    }

    let mut errors = 0;
    let mut warnings = 0;

    for f in &mx_files {
        let content = std::fs::read_to_string(f)?;
        let filename = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();

        // Parse with Oxc
        match morph_parser::parse_mx_str(&content, &filename) {
            Ok(source) => {
                // Check structural rules
                let mut file_warnings = Vec::new();

                if source.components.is_empty() {
                    file_warnings.push("No component found (expected `export default function App()`)");
                }

                if source.window_config.is_none() {
                    file_warnings.push("No `windowConfig` export found");
                }

                // Check for deprecated patterns
                if content.contains("morphState") {
                    warnings += 1;
                    file_warnings.push("Uses `morphState` (deprecated, use `morph.state`)");
                }

                if file_warnings.is_empty() {
                    crate::logger::log_success(&format!("{} — OK ({} component(s), {} import(s))",
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

        // Migrate deprecated patterns
        if migrate && content.contains("morphState") {
            let new = content.replace("morphState", "morph.state");
            std::fs::write(f, new)?;
            crate::logger::log_success(&format!("Migrated {}", f.display()));
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
