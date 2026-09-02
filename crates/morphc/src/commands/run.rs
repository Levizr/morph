use anyhow::Result;
use colored::Colorize;

pub fn run(
    binary: Option<String>,
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let cfg = morph_config::MorphConfig::from_file(&cwd.join("morph.config.json")).unwrap_or_default();
    crate::logger::log_banner(&format!("Morph Run — {}", cfg.name));

    // Build with the same detail logs as `morph build`, but without the
    // "Morph Build" banner or raw compiler command.
    let input_binary = binary.clone();
    let bin_path = if let Some(bin) = input_binary {
        std::path::PathBuf::from(bin)
    } else {
        crate::commands::build::run(entry, output, static_, None, false, true)?
    };

    if bin_path.exists() {
        crate::logger::log_step("Executing");
        let pb = crate::logger::spinner(&format!("Running {}...", bin_path.display().to_string().cyan()));
        let status = std::process::Command::new(&bin_path).status()?;
        pb.finish_and_clear();
        if status.success() {
            crate::logger::log_success("Process exited successfully");
        } else {
            crate::logger::log_error(&format!("Process exited with {}", status));
        }
    } else {
        crate::logger::log_warn(&format!("Binary not found at {}. Use `morph run <binary>`.", bin_path.display()));
    }

    println!();
    Ok(())
}
