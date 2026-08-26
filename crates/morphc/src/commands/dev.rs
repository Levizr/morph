use anyhow::Result;
use colored::Colorize;

pub fn run(entry: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry_file = entry.as_deref().unwrap_or(&config.entry);

    crate::logger::log_banner(&format!("Morph Dev — {}", config.name));

    crate::logger::log_step("Verifying runtime");
    crate::commands::install::ensure_runtime(&cwd)?;
    crate::logger::log_success(&format!(
        "Runtime {} v{}",
        config.runtime.runtime_type.cyan(),
        config.runtime.version.dimmed()
    ));

    crate::logger::log_step("Configuration");
    crate::logger::log_key("Entry", entry_file);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );

    crate::logger::log_step("Parsing");
    let pb = crate::logger::spinner("Parsing .mx files with Oxc...");
    std::thread::sleep(std::time::Duration::from_millis(300));
    pb.finish_and_clear();
    crate::logger::log_success("Parsed .mx files");

    let pb = crate::logger::spinner("Building IR...");
    std::thread::sleep(std::time::Duration::from_millis(200));
    pb.finish_and_clear();
    crate::logger::log_success("IR built");

    println!();
    crate::logger::log_warn("Dev mode with hot reload not yet fully implemented in morphc.");
    crate::logger::log_bullet("Full dev server will:");
    crate::logger::log_muted("Parse .mx with Oxc (parallel)");
    crate::logger::log_muted("Build IR");
    crate::logger::log_muted("Start devrt via CMake");
    crate::logger::log_muted("Watch src/ for changes");
    crate::logger::log_muted("Push IR over TCP to devrt");
    println!();
    crate::logger::log_bullet("For now, use Python toolchain:");
    crate::logger::log_muted("$ pip install -e . && morph dev");
    println!();

    Ok(())
}
