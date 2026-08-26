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
        for entry in walkdir::WalkDir::new(&src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry
                .path()
                .extension()
                .map(|e| e == "mx")
                .unwrap_or(false)
            {
                mx_files.push(entry.path().to_path_buf());
            }
        }
    }

    if mx_files.is_empty() {
        crate::logger::log_warn("No .mx files found in src/");
        println!();
        return Ok(());
    }

    let mut warnings = 0;
    for f in &mx_files {
        crate::logger::log_dim(&format!("Checking {}...", f.display()));
        let content = std::fs::read_to_string(f)?;

        // Simple lint: check for deprecated morphState
        if content.contains("morphState") {
            warnings += 1;
            crate::logger::log_warn(&format!(
                "Line contains {} (deprecated, use {})",
                "morphState".yellow(),
                "morph.state".green()
            ));
            if migrate {
                let new = content.replace("morphState", "morph.state");
                std::fs::write(f, new)?;
                crate::logger::log_success(&format!("Migrated {}", f.display()));
            }
        } else {
            crate::logger::log_success(&format!("{} — no issues", f.display()));
        }
    }

    println!();
    if migrate {
        crate::logger::log_success(&format!(
            "Migration complete. {} files checked.",
            mx_files.len()
        ));
    } else if warnings > 0 {
        crate::logger::log_warn(&format!(
            "{} file(s) with warnings. Run {} to auto-fix.",
            warnings,
            "morph check --migrate".yellow().bold()
        ));
    } else {
        crate::logger::log_success(&format!(
            "All {} file(s) passed.",
            mx_files.len()
        ));
    }
    println!();

    Ok(())
}
