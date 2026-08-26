use anyhow::Result;
use std::path::Path;

pub fn detect_compiler() -> String {
    // Prefer g++, fallback to clang++
    for c in ["g++", "clang++", "c++"] {
        if which(c) {
            return c.to_string();
        }
    }
    "g++".to_string()
}

fn which(bin: &str) -> bool {
    std::process::Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn build_project(project_dir: &Path) -> Result<()> {
    println!("  Building project at {}", project_dir.display());
    println!("  Using compiler: {}", detect_compiler());
    // TODO: invoke g++/clang++ with runtime includes + generated sources
    Ok(())
}

pub fn check_system() -> Result<Vec<(&'static str, bool, String)>> {
    let checks = vec![
        ("g++", which("g++"), get_version("g++", "--version")),
        ("clang++", which("clang++"), get_version("clang++", "--version")),
        ("cmake", which("cmake"), get_version("cmake", "--version")),
        ("pkg-config", which("pkg-config"), get_version("pkg-config", "--version")),
    ];
    Ok(checks)
}

fn get_version(bin: &str, arg: &str) -> String {
    std::process::Command::new(bin)
        .arg(arg)
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(|l| l.to_string()))
        .unwrap_or_default()
}
