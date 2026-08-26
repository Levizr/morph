use anyhow::{Context, Result};
use colored::Colorize;
use morph_config::{MorphConfig, VersionFile};
use std::path::PathBuf;

pub fn run(runtime: bool, self_update: bool) -> Result<()> {
    if !runtime && !self_update {
        return run_status();
    }
    if runtime {
        return run_runtime_update();
    }
    if self_update {
        return run_self_update();
    }
    Ok(())
}

fn run_status() -> Result<()> {
    let morphc_version = env!("CARGO_PKG_VERSION");

    crate::logger::log_banner("Morph Update — Status");

    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    let runtime_version = if config_path.exists() {
        MorphConfig::from_file(&config_path)?.runtime.version
    } else {
        "unknown".to_string()
    };

    let latest_runtime =
        get_latest_runtime_version("cpp").unwrap_or_else(|_| "unknown".to_string());
    let latest_morphc =
        get_latest_morphc_version().unwrap_or_else(|_| morphc_version.to_string());

    crate::logger::log_step("Versions");
    crate::logger::log_version("morphc", morphc_version, &latest_morphc);
    crate::logger::log_version("Runtime", &runtime_version, &latest_runtime);

    if runtime_version != latest_runtime {
        println!();
        crate::logger::log_info(&format!(
            "New runtime available: {}",
            format!("v{}", latest_runtime).green().bold()
        ));
        crate::logger::log_bullet(&format!(
            "Run {} to update.",
            "morph update --runtime".yellow().bold()
        ));
    }

    if morphc_version != latest_morphc {
        println!();
        crate::logger::log_info(&format!(
            "New morphc available: {}",
            format!("v{}", latest_morphc).green().bold()
        ));
        crate::logger::log_bullet(&format!(
            "Run {} to update.",
            "morph update --self".yellow().bold()
        ));
    }

    println!();
    Ok(())
}

fn run_runtime_update() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let mut config = MorphConfig::from_file(&config_path)?;
    let current = config.runtime.version.clone();
    let runtime_type = config.runtime.runtime_type.clone();

    crate::logger::log_banner(&format!("Morph Update — Runtime {} → {}", current.dimmed(), "latest".cyan()));

    let latest = get_latest_runtime_version(&runtime_type)
        .with_context(|| format!("could not determine latest runtime version for {}", runtime_type))?;

    if current == latest {
        crate::logger::log_success(&format!(
            "Runtime already at latest version {}",
            format!("v{}", current).cyan().bold()
        ));
        println!();
        return Ok(());
    }

    crate::logger::log_step("Downloading");
    crate::logger::log_key("From", &format!("v{}", current));
    crate::logger::log_key("To", &format!("v{}", latest));

    let pb = crate::logger::spinner(&format!("Downloading runtime v{}...", latest));
    morph_cache::download_runtime(&runtime_type, &latest)?;
    pb.finish_and_clear();

    crate::logger::log_success("Download complete");

    // Update config
    crate::logger::log_step("Updating configuration");
    config.runtime.version = latest.clone();
    config.save(&config_path)?;
    crate::logger::log_success(&format!(
        "Updated {} from {} to {}",
        "morph.config.json".dimmed(),
        format!("v{}", current).dimmed(),
        format!("v{}", latest).green().bold()
    ));

    // Update morph.lock
    let sha = "updated".to_string();
    morph_cache::write_lock_file(&cwd, &runtime_type, &latest, &sha)?;

    // Link to project
    crate::logger::log_step("Linking runtime");
    let morph_dir = cwd.join(".morph");
    std::fs::create_dir_all(&morph_dir)?;
    morph_cache::link_runtime_to_project(&runtime_type, &latest, &morph_dir)?;
    crate::logger::log_success("Runtime linked");

    // Show migration notes
    let version_file = PathBuf::from(format!("versions/runtime/{}.json", runtime_type));
    if version_file.exists() {
        if let Ok(vf) = VersionFile::from_file(&version_file) {
            crate::logger::log_step(&format!("Migration Notes (v{} → v{})", current, latest));
            crate::logger::divider_thick();
            if vf.breaking {
                crate::logger::log_warn("Breaking changes!");
            } else {
                crate::logger::log_success("No breaking changes");
            }
            crate::logger::log_bullet(&vf.changelog.dimmed().to_string());
            println!();
            crate::logger::log_bullet(&format!(
                "Run {} to auto-fix deprecated patterns.",
                "morph check --migrate".yellow().bold()
            ));
            crate::logger::divider_thick();
        }
    }

    println!();
    crate::logger::log_success(&format!(
        "Restart {} to use new runtime.",
        "morph dev".cyan().bold()
    ));
    println!();

    Ok(())
}

fn run_self_update() -> Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let latest = get_latest_morphc_version().unwrap_or_else(|_| current.to_string());

    crate::logger::log_banner("Morph Update — Self");

    if current == latest {
        crate::logger::log_success(&format!(
            "morphc already at latest version {}",
            format!("v{}", current).cyan().bold()
        ));
        println!();
        return Ok(());
    }

    crate::logger::log_step("Downloading");
    crate::logger::log_key("From", &format!("v{}", current));
    crate::logger::log_key("To", &format!("v{}", latest));

    let platform = detect_platform();
    let url = format!(
        "https://github.com/Levizr/morph/releases/download/v{}/morph-{}-{}.tar.gz",
        latest, platform.0, platform.1
    );

    crate::logger::log_key("Platform", &format!("{}-{}", platform.0, platform.1));
    crate::logger::log_dim(&url);

    let pb = crate::logger::spinner(&format!("Downloading morphc v{}...", latest));
    match download_and_install(&url) {
        Ok(_) => {
            pb.finish_and_clear();
            crate::logger::log_success("Updated! Run `morph --version` to verify.");
        }
        Err(e) => {
            pb.finish_and_clear();
            crate::logger::log_error(&format!("Self-update failed: {}", e));
            println!();
            crate::logger::log_info("Please update manually:");
            crate::logger::log_muted(&format!(
                "curl -fsSL https://get.morph.dev | sh  # installs v{}",
                latest
            ));
            crate::logger::log_dim(&format!(
                "https://github.com/Levizr/morph/releases/tag/v{}",
                latest
            ));
        }
    }

    println!();
    Ok(())
}

fn detect_platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "windows"
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        "arm64"
    };
    (os, arch)
}

fn download_and_install(url: &str) -> Result<()> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent(format!("morphc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client.get(url).send()?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {}", resp.status(), url);
    }
    let bytes = resp.bytes()?;

    let tmp = std::env::temp_dir().join(format!(
        "morph-{}-update.tar.gz",
        env!("CARGO_PKG_VERSION")
    ));
    std::fs::write(&tmp, &bytes)?;

    crate::logger::log_success(&format!("Downloaded to {}", tmp.display()));

    let gz = flate2::read::GzDecoder::new(bytes.as_ref());
    let mut archive = tar::Archive::new(gz);

    let tmp_dir = std::env::temp_dir().join("morph-update-extract");
    std::fs::create_dir_all(&tmp_dir)?;
    archive.unpack(&tmp_dir)?;

    let morph_bin = walkdir::WalkDir::new(&tmp_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name() == "morph" || e.file_name() == "morphc")
        .map(|e| e.path().to_path_buf());

    if let Some(bin) = morph_bin {
        let dest = std::env::current_exe()?;
        let backup = dest.with_extension("backup");
        let _ = std::fs::copy(&dest, &backup);
        std::fs::copy(&bin, &dest)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&dest)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&dest, perms)?;
        }

        crate::logger::log_success(&format!(
            "Installed to {}",
            dest.display().to_string().dimmed()
        ));
    } else {
        crate::logger::log_warn("Could not find morph binary in archive");
        crate::logger::log_dim(&tmp_dir.display().to_string());
    }

    Ok(())
}

fn get_latest_runtime_version(runtime_type: &str) -> Result<String> {
    let local = PathBuf::from(format!("versions/runtime/{}.json", runtime_type));
    if local.exists() {
        let vf = VersionFile::from_file(&local)?;
        return Ok(vf.version);
    }
    anyhow::bail!("no version file for runtime {}", runtime_type)
}

fn get_latest_morphc_version() -> Result<String> {
    let local = PathBuf::from("versions/morphc/version.json");
    if local.exists() {
        let vf = VersionFile::from_file(&local)?;
        return Ok(vf.version);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(format!("morphc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client
        .get("https://api.github.com/repos/Levizr/morph/releases/latest")
        .header("Accept", "application/vnd.github.v3+json")
        .send();

    if let Ok(r) = resp {
        if r.status().is_success() {
            if let Ok(json) = r.json::<serde_json::Value>() {
                if let Some(tag) = json.get("tag_name").and_then(|v| v.as_str()) {
                    return Ok(tag.trim_start_matches('v').to_string());
                }
            }
        }
    }

    anyhow::bail!("could not fetch latest morphc version")
}
