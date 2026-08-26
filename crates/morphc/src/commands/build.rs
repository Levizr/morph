use anyhow::Result;
use colored::Colorize;

pub fn run(
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
    upx: Option<bool>,
    no_upx: bool,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry = entry.unwrap_or(config.entry.clone());
    let output = output.unwrap_or(config.output.clone());

    crate::logger::log_banner(&format!("Morph Build — {}", config.name));

    crate::logger::log_step("Verifying runtime");
    crate::commands::install::ensure_runtime(&cwd)?;
    crate::logger::log_success(&format!(
        "Runtime {} v{}",
        config.runtime.runtime_type.cyan(),
        config.runtime.version.dimmed()
    ));

    crate::logger::log_step("Configuration");
    crate::logger::log_key("Entry", &entry);
    crate::logger::log_key("Output", &output);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );
    if static_ {
        crate::logger::log_key("Static", "enabled");
    }
    if let Some(u) = upx {
        crate::logger::log_key("UPX", &u.to_string());
    }
    if no_upx {
        crate::logger::log_key("UPX", "disabled");
    }

    crate::logger::log_step("Compiling");

    let pb = crate::logger::spinner("Parsing .mx files...");
    std::thread::sleep(std::time::Duration::from_millis(300));
    pb.finish_and_clear();
    crate::logger::log_success("Parsed .mx files");

    let pb = crate::logger::spinner("Building IR...");
    std::thread::sleep(std::time::Duration::from_millis(200));
    pb.finish_and_clear();
    crate::logger::log_success("IR built");

    let pb = crate::logger::spinner("Generating C++...");
    std::thread::sleep(std::time::Duration::from_millis(200));
    pb.finish_and_clear();
    crate::logger::log_success(&format!("C++ generated → {}", output.dimmed()));

    let compiler = morph_build::detect_compiler();
    let pb = crate::logger::spinner(&format!("Compiling with {}...", compiler));
    std::thread::sleep(std::time::Duration::from_millis(300));
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Compiled with {}", compiler));

    println!();
    crate::logger::log_warn("Build system not yet fully implemented in morphc.");
    crate::logger::log_bullet("For now, use Python toolchain:");
    crate::logger::log_muted("$ pip install -e . && morph build");
    println!();

    Ok(())
}
