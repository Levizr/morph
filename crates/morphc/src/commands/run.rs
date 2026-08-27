use anyhow::Result;
use colored::Colorize;

pub fn run(
    binary: Option<String>,
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
) -> Result<()> {
    crate::logger::log_banner("Morph Run");

    crate::logger::log_step("Building");
    // Need to capture output/clean_name for later binary lookup — pass through
    let output_clone = output.clone();
    crate::commands::build::run(entry, output, static_, None, false)?;

    let bin_path = if let Some(bin) = binary {
        std::path::PathBuf::from(bin)
    } else {
        // Resolve from config
        let cwd = std::env::current_dir()?;
        let cfg = morph_config::MorphConfig::from_file(&cwd.join("morph.config.json")).unwrap_or_default();
        let out_raw = output_clone.unwrap_or(cfg.output);
        let clean = morph_config::clean_app_name(&cfg.name);
        let out_dir = cwd.join(&out_raw);
        let out_dir = if out_raw.ends_with('/') || std::path::Path::new(&out_raw).extension().is_none() {
            out_dir
        } else {
            out_dir.parent().map(|p| p.to_path_buf()).unwrap_or(out_dir)
        };
        out_dir.join(format!("{}{}", clean, morph_build::exe_suffix()))
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
