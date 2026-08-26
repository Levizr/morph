use anyhow::Result;
use colored::Colorize;

pub fn run(verbose: bool, _yes: bool) -> Result<()> {
    crate::logger::log_banner("Morph Doctor — System Check");

    crate::logger::log_step("Required tools");
    let checks = morph_build::check_system()?;

    let mut all_ok = true;
    for (name, ok, version) in &checks {
        if !ok && *name == "g++" {
            all_ok = false;
        }
        let status = if *ok {
            "✓".green().bold().to_string()
        } else {
            "✗".red().bold().to_string()
        };
        let ver = if verbose && !version.is_empty() {
            format!("  {}", version.dimmed())
        } else if !version.is_empty() {
            format!(
                "  {}",
                version.lines().next().unwrap_or("").dimmed()
            )
        } else {
            "".to_string()
        };
        println!(
            "      {} {:<14}{}",
            status,
            name.bold(),
            ver
        );
    }

    crate::logger::log_step("Optional libraries");
    for (name, bin) in [
        ("GLFW", "glfw"),
        ("FreeType", "freetype-config"),
        ("HarfBuzz", "harfbuzz"),
    ] {
        let found = which(bin);
        let status = if found {
            "✓".green().bold().to_string()
        } else {
            "○".dimmed().to_string()
        };
        let ver = if found {
            format!("  {}", "found".dimmed())
        } else {
            format!("  {}", "(optional)".dimmed())
        };
        println!(
            "      {} {:<14}{}",
            status,
            name.bold(),
            ver
        );
    }

    println!();
    if all_ok {
        crate::logger::log_success("All checks passed!");
    } else {
        crate::logger::log_warn("Some checks failed. Install missing tools:");
        crate::logger::log_bullet(
            &"sudo apt install build-essential cmake pkg-config libglfw3-dev libfreetype6-dev libharfbuzz-dev"
                .dimmed()
                .to_string(),
        );
    }
    println!();

    Ok(())
}

fn which(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
