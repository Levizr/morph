use anyhow::Result;
use colored::Colorize;

pub fn run() -> Result<()> {
    crate::logger::log_banner("Morph Cache — Management");

    let morph_cache = std::env::current_dir()?.join(".morph/cache");

    crate::logger::log_step("Project cache");
    if morph_cache.exists() {
        let size = dir_size(&morph_cache);
        crate::logger::log_key("Path", &morph_cache.display().to_string());
        crate::logger::log_key("Size", &format_bytes(size));

        let confirm = dialoguer::Confirm::new()
            .with_prompt(format!(
                "  {} Clear project cache?",
                "→".blue().bold()
            ))
            .default(false)
            .interact()?;

        if confirm {
            std::fs::remove_dir_all(&morph_cache)?;
            std::fs::create_dir_all(&morph_cache)?;
            crate::logger::log_success("Project cache cleared");
        } else {
            crate::logger::log_info("Skipped");
        }
    } else {
        crate::logger::log_info("No project cache found");
    }

    crate::logger::log_step("Global cache");
    let global = crate::cache::global_cache_root()?.join("cache");
    if global.exists() {
        let size = dir_size(&global);
        crate::logger::log_key("Path", &global.display().to_string());
        crate::logger::log_key("Size", &format_bytes(size));
        crate::logger::log_muted(&format!(
            "Run {} to clear global cache.",
            "rm -rf ~/.morph/cache".yellow()
        ));
    } else {
        crate::logger::log_info("No global cache found");
    }

    println!();
    Ok(())
}

fn dir_size(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
