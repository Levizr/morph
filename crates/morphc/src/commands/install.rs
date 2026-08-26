use anyhow::{Context, Result};
use colored::Colorize;
use morph_config::MorphConfig;
use std::path::{Path, PathBuf};

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    run_with_dir(&cwd)
}

pub fn run_with_dir(project_dir: &Path) -> Result<()> {
    let config_path = project_dir.join("morph.config.json");
    let morph_dir = project_dir.join(".morph");

    if !config_path.exists() {
        anyhow::bail!(
            "morph.config.json not found in {}. Run `morph new` first.",
            project_dir.display()
        );
    }

    let config = MorphConfig::from_file(&config_path)?;
    config.validate()?;

    let runtime_type = &config.runtime.runtime_type;
    let version = &config.runtime.version;

    crate::logger::log_banner("Morph Install — Runtime Setup");

    crate::logger::log_step("Reading configuration");
    crate::logger::log_key("Runtime", &format!("{} v{}", runtime_type.cyan(), version.dimmed()));

    crate::logger::log_step("Checking global cache");
    let cached_dir = morph_cache::global_runtime_version_dir(runtime_type, version)?;

    if morph_cache::is_runtime_cached(runtime_type, version) {
        crate::logger::log_success(&format!(
            "Found in cache: {}",
            cached_dir.display().to_string().dimmed()
        ));
    } else {
        crate::logger::log_info(&format!(
            "Not cached, downloading {} v{}...",
            runtime_type.cyan(),
            version.dimmed()
        ));
        let pb = crate::logger::spinner("Downloading runtime...");
        morph_cache::download_runtime(runtime_type, version)
            .with_context(|| {
                pb.finish_and_clear();
                format!("failed to install runtime {} v{}", runtime_type, version)
            })?;
        pb.finish_and_clear();
        crate::logger::log_success("Download complete");
    }

    // Link to project .morph/runtime
    crate::logger::log_step("Linking runtime to project");
    std::fs::create_dir_all(&morph_dir)?;
    let project_runtime =
        morph_cache::link_runtime_to_project(runtime_type, version, &morph_dir)?;

    crate::logger::log_success(&format!(
        "Linked to {}",
        project_runtime.display().to_string().dimmed()
    ));

    // Write lock file
    crate::logger::log_step("Writing lock file");
    let sha = if cached_dir.join("manifest.json").exists() {
        let m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(cached_dir.join("manifest.json"))?)?;
        m.get("sha256")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    };

    morph_cache::write_lock_file(project_dir, runtime_type, version, &sha)?;
    crate::logger::log_success(&format!("Wrote {}", "morph.lock".cyan()));

    println!();
    crate::logger::log_success(&format!(
        "Runtime {} v{} installed!",
        runtime_type.cyan(),
        version.green().bold()
    ));
    println!(
        "\n    {} {}",
        "→".dimmed(),
        "morph dev".cyan().bold()
    );
    println!();

    // Check compatibility warning
    check_version_compatibility(version);

    Ok(())
}

fn check_version_compatibility(runtime_version: &str) {
    let morphc_version = env!("CARGO_PKG_VERSION");
    let compat = crate::versions::check_compatibility(morphc_version, runtime_version);
    match compat {
        crate::versions::Compatibility::Deprecated => {
            crate::logger::log_warn(&format!(
                "Runtime v{} is deprecated for morphc v{}",
                runtime_version, morphc_version
            ));
            crate::logger::log_bullet(&format!(
                "Run {} to update.",
                "morph update --runtime".yellow().bold()
            ));
            println!();
        }
        crate::versions::Compatibility::Incompatible => {
            crate::logger::log_error(&format!(
                "Runtime v{} is incompatible with morphc v{}",
                runtime_version, morphc_version
            ));
            crate::logger::log_bullet(&format!(
                "Run {} or {}.",
                "morph update --runtime".yellow(),
                "morph update --self".yellow()
            ));
            println!();
        }
        _ => {}
    }
}

/// Helper: ensure runtime is installed for dev/build commands
pub fn ensure_runtime(project_dir: &Path) -> Result<PathBuf> {
    let config_path = project_dir.join("morph.config.json");
    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }
    let config = MorphConfig::from_file(&config_path)?;
    let morph_dir = project_dir.join(".morph");
    let project_runtime = morph_dir.join("runtime");

    if !morph_cache::is_project_runtime_installed(&morph_dir) {
        let cached = morph_cache::global_runtime_version_dir(
            &config.runtime.runtime_type,
            &config.runtime.version,
        )?;
        if cached.exists() {
            crate::logger::log_info("Runtime not linked, linking from cache...");
            morph_cache::link_runtime_to_project(
                &config.runtime.runtime_type,
                &config.runtime.version,
                &morph_dir,
            )?;
            return Ok(project_runtime);
        }
        anyhow::bail!(
            "Runtime {} v{} not installed. Run `morph install` first.",
            config.runtime.runtime_type,
            config.runtime.version
        );
    }

    // Check compatibility
    check_version_compatibility(&config.runtime.version);

    Ok(project_runtime)
}
