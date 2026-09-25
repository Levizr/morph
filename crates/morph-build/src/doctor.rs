//! System dependency checks for `morph doctor`.
//!
//! GLFW, FreeType and HarfBuzz are **mandatory**: the C++ runtime links them
//! on every platform (dynamic build) or builds them from source (static
//! build), and text never renders without FreeType/HarfBuzz. Detection uses
//! `pkg-config` first with header fallbacks — never `which`, because these
//! are libraries, not binaries.

use anyhow::Result;
use std::path::PathBuf;

/// One checked dependency.
#[derive(Debug, Clone)]
pub struct DependencyCheck {
    /// Display name (`GLFW`).
    pub name: &'static str,
    /// `pkg-config` package name (`glfw3`, `freetype2`, `harfbuzz`).
    pub pkg: &'static str,
    /// Present on this machine.
    pub ok: bool,
    /// Version string (empty when unknown).
    pub version: String,
    /// How it was found: `pkg-config`, `header`, `framework`, `bundled`.
    pub found_via: String,
}

/// Package manager detected on this machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageManager {
    Apt,
    Dnf,
    Pacman,
    Zypper,
    Apk,
    Brew,
    Winget,
    Choco,
    Unknown,
}

impl PackageManager {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Apt => "apt",
            Self::Dnf => "dnf",
            Self::Pacman => "pacman",
            Self::Zypper => "zypper",
            Self::Apk => "apk",
            Self::Brew => "brew",
            Self::Winget => "winget",
            Self::Choco => "choco",
            Self::Unknown => "unknown",
        }
    }

    pub const fn needs_sudo(self) -> bool {
        !matches!(self, Self::Brew | Self::Winget | Self::Choco | Self::Unknown)
    }
}

/// Detect the native package manager by OS + well-known binaries.
pub fn detect_package_manager() -> PackageManager {
    if cfg!(target_os = "macos") {
        return if path_has("brew") { PackageManager::Brew } else { PackageManager::Unknown };
    }
    if cfg!(target_os = "windows") {
        if path_has("winget") {
            return PackageManager::Winget;
        }
        if path_has("choco") {
            return PackageManager::Choco;
        }
        return PackageManager::Unknown;
    }
    for (bin, manager) in [
        ("apt-get", PackageManager::Apt),
        ("apt", PackageManager::Apt),
        ("dnf", PackageManager::Dnf),
        ("pacman", PackageManager::Pacman),
        ("zypper", PackageManager::Zypper),
        ("apk", PackageManager::Apk),
        ("brew", PackageManager::Brew),
    ] {
        if path_has(bin) || PathBuf::from(format!("/usr/bin/{bin}")).exists() {
            return manager;
        }
    }
    if std::fs::read_to_string("/etc/os-release").is_ok_and(|s| {
        let id_line = s.lines().find(|l| l.starts_with("ID=")).unwrap_or("").to_string();
        let like_line = s.lines().find(|l| l.starts_with("ID_LIKE=")).unwrap_or("").to_string();
        let combined = format!("{id_line} {like_line}");
        combined.contains("debian") || combined.contains("ubuntu")
    }) {
        return PackageManager::Apt;
    }
    PackageManager::Unknown
}

/// OS description for `morph doctor` output (`linux x86_64`, ...).
pub fn os_description() -> String {
    format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
}

/// Minimum C++ compiler versions: generated code needs C++23
/// (`std::println`), which GCC only supports from 14 and Clang from 17.
pub const MIN_GCC_MAJOR: u32 = 14;
pub const MIN_CLANG_MAJOR: u32 = 17;

/// Check the C++ compiler morph will actually use ([`crate::platform::pick_cpp`]).
/// GCC older than 14 (or Clang older than 17) is a hard failure, not a warning.
pub fn check_cpp_compiler() -> DependencyCheck {
    let cxx = crate::platform::pick_cpp();
    let is_clang = cxx.contains("clang");
    let version = compiler_version_line(&cxx);
    let major = compiler_major(&cxx);
    let (ok, need) = match (is_clang, major) {
        (false, Some(m)) => (m >= MIN_GCC_MAJOR, format!("g++-{MIN_GCC_MAJOR}+")),
        (true, Some(m)) => (m >= MIN_CLANG_MAJOR, format!("clang++ {MIN_CLANG_MAJOR}+")),
        (false, None) => (false, format!("g++-{MIN_GCC_MAJOR}+")),
        (true, None) => (false, format!("clang++ {MIN_CLANG_MAJOR}+")),
    };
    let detail = if ok {
        version
    } else if version.is_empty() {
        format!("not found — install {need} (C++23 required)")
    } else {
        format!("{version} — requires {need} (C++23)")
    };
    DependencyCheck { name: "C++", pkg: "c++", ok, version: detail, found_via: cxx }
}

/// Major version of `bin` via `-dumpversion` (`14`, `13.3.0` → `13`),
/// falling back to the first `version X` number in `--version`.
pub fn compiler_major(bin: &str) -> Option<u32> {
    let dump = std::process::Command::new(bin)
        .arg("-dumpversion")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();
    if let Some(major) = major_of_dump(dump.trim()) {
        return Some(major);
    }
    let first = compiler_version_line(bin);
    first
        .split("version")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(major_of_dump)
}

fn major_of_dump(s: &str) -> Option<u32> {
    s.split(['.', '-', ' ']).next()?.trim().parse::<u32>().ok()
}

fn compiler_version_line(bin: &str) -> String {
    std::process::Command::new(bin)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(ToString::to_string))
        .unwrap_or_default()
}

/// Check the three mandatory graphics/text libraries.
pub fn check_libraries() -> Vec<DependencyCheck> {
    vec![
        check_one("GLFW", "glfw3", &["GLFW/glfw3.h", "glfw3.h"]),
        check_one("FreeType", "freetype2", &["freetype2/ft2build.h", "ft2build.h"]),
        check_one("HarfBuzz", "harfbuzz", &["harfbuzz/hb.h", "hb.h"]),
    ]
}

fn check_one(name: &'static str, pkg: &'static str, headers: &[&str]) -> DependencyCheck {
    if let Some(version) = pkg_config_version(pkg) {
        return DependencyCheck {
            name,
            pkg,
            ok: true,
            version,
            found_via: "pkg-config".to_string(),
        };
    }
    if cfg!(target_os = "macos") {
        for prefix in ["/opt/homebrew", "/usr/local"] {
            for header in headers {
                if PathBuf::from(format!("{prefix}/include/{header}")).exists() {
                    return DependencyCheck {
                        name,
                        pkg,
                        ok: true,
                        version: String::new(),
                        found_via: "header".to_string(),
                    };
                }
            }
        }
    } else if cfg!(target_os = "windows") {
        return DependencyCheck {
            name,
            pkg,
            ok: true,
            version: String::new(),
            found_via: "bundled".to_string(),
        };
    } else {
        for header in headers {
            if PathBuf::from(format!("/usr/include/{header}")).exists() {
                return DependencyCheck {
                    name,
                    pkg,
                    ok: true,
                    version: String::new(),
                    found_via: "header".to_string(),
                };
            }
        }
        let arch_dirs = [
            "/usr/include/x86_64-linux-gnu",
            "/usr/include/aarch64-linux-gnu",
            "/usr/include/arm-linux-gnueabihf",
        ];
        for dir in arch_dirs {
            for header in headers {
                if PathBuf::from(format!("{dir}/{header}")).exists() {
                    return DependencyCheck {
                        name,
                        pkg,
                        ok: true,
                        version: String::new(),
                        found_via: "header".to_string(),
                    };
                }
            }
        }
    }
    DependencyCheck { name, pkg, ok: false, version: String::new(), found_via: String::new() }
}

fn pkg_config_version(pkg: &str) -> Option<String> {
    std::process::Command::new("pkg-config")
        .args(["--modversion", pkg])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Per-manager system packages for one `morph doctor` dependency key.
///
/// Keys: `toolchain`, `glfw`, `gl`, `x11`, `freetype`, `harfbuzz`.
#[allow(clippy::match_same_arms)]
pub fn packages_for(manager: PackageManager, key: &str) -> Vec<String> {
    let pkgs: &[&str] = match (manager, key) {
        (PackageManager::Apt, "toolchain") => &["g++-14", "cmake", "make", "pkg-config"],
        (PackageManager::Apt, "glfw") => &["libglfw3-dev"],
        (PackageManager::Apt, "gl") => &["libgl1-mesa-dev"],
        (PackageManager::Apt, "x11") => &["libx11-dev"],
        (PackageManager::Apt, "freetype") => &["libfreetype-dev"],
        (PackageManager::Apt, "harfbuzz") => &["libharfbuzz-dev"],
        (PackageManager::Dnf, "toolchain") => &["gcc-c++", "cmake", "make", "pkgconf-pkg-config"],
        (PackageManager::Dnf, "glfw") => &["glfw-devel"],
        (PackageManager::Dnf, "gl") => &["mesa-libGL-devel"],
        (PackageManager::Dnf, "x11") => &["libX11-devel"],
        (PackageManager::Dnf, "freetype") => &["freetype-devel"],
        (PackageManager::Dnf, "harfbuzz") => &["harfbuzz-devel"],
        (PackageManager::Pacman, "toolchain") => &["base-devel", "cmake", "pkgconf"],
        (PackageManager::Pacman, "glfw") => &["glfw"],
        (PackageManager::Pacman, "gl") => &["mesa"],
        (PackageManager::Pacman, "x11") => &["libx11"],
        (PackageManager::Pacman, "freetype") => &["freetype2"],
        (PackageManager::Pacman, "harfbuzz") => &["harfbuzz"],
        (PackageManager::Zypper, "toolchain") => &["gcc-c++", "cmake", "make", "pkg-config"],
        (PackageManager::Zypper, "glfw") => &["glfw-devel"],
        (PackageManager::Zypper, "gl") => &["Mesa-libGL-devel"],
        (PackageManager::Zypper, "x11") => &["libX11-devel"],
        (PackageManager::Zypper, "freetype") => &["freetype2-devel"],
        (PackageManager::Zypper, "harfbuzz") => &["harfbuzz-devel"],
        (PackageManager::Apk, "toolchain") => &["build-base", "cmake", "make", "pkgconf"],
        (PackageManager::Apk, "glfw") => &["glfw-dev"],
        (PackageManager::Apk, "gl") => &["mesa-dev"],
        (PackageManager::Apk, "x11") => &["libx11-dev"],
        (PackageManager::Apk, "freetype") => &["freetype-dev"],
        (PackageManager::Apk, "harfbuzz") => &["harfbuzz-dev"],
        (PackageManager::Brew, "toolchain") => &["cmake", "pkg-config"],
        (PackageManager::Brew, "glfw") => &["glfw"],
        (PackageManager::Brew, "gl") => &[],
        (PackageManager::Brew, "x11") => &[],
        (PackageManager::Brew, "freetype") => &["freetype"],
        (PackageManager::Brew, "harfbuzz") => &["harfbuzz"],
        (PackageManager::Winget, "toolchain") => &["Kitware.CMake"],
        (PackageManager::Winget, "glfw") => &["GLFW.GLFW3"],
        (PackageManager::Winget, "gl") => &[],
        (PackageManager::Winget, "x11") => &[],
        (PackageManager::Winget, "freetype") => &["FreeType.FreeType"],
        (PackageManager::Winget, "harfbuzz") => &["HarfBuzz.HarfBuzz"],
        (PackageManager::Choco, "toolchain") => &["cmake", "pkgconfiglite"],
        (PackageManager::Choco, "glfw") => &["glfw3"],
        (PackageManager::Choco, "gl") => &[],
        (PackageManager::Choco, "x11") => &[],
        (PackageManager::Choco, "freetype") => &["freetype"],
        (PackageManager::Choco, "harfbuzz") => &["harfbuzz"],
        _ => &[],
    };
    pkgs.iter().map(ToString::to_string).collect()
}

/// Full install command line for `manager` + `packages`.
pub fn install_command(manager: PackageManager, packages: &[String]) -> String {
    let list = packages.join(" ");
    match manager {
        PackageManager::Apt => format!("sudo apt update && sudo apt install -y {list}"),
        PackageManager::Dnf => format!("sudo dnf install -y {list}"),
        PackageManager::Pacman => format!("sudo pacman -S --needed {list}"),
        PackageManager::Zypper => format!("sudo zypper install -y {list}"),
        PackageManager::Apk => format!("sudo apk add {list}"),
        PackageManager::Brew => format!("brew install {list}"),
        PackageManager::Winget => format!("winget install {list}"),
        PackageManager::Choco => format!("choco install -y {list}"),
        PackageManager::Unknown => format!("install manually: {list}"),
    }
}

/// Run the install command for `manager`. Returns an error with the manual
/// command when the manager is unknown or the install fails.
pub fn auto_install(manager: PackageManager, packages: &[String]) -> Result<()> {
    if packages.is_empty() || manager == PackageManager::Unknown {
        anyhow::bail!("no known package manager — install manually: {}", packages.join(" "));
    }
    let (program, args): (&str, Vec<&str>) = match manager {
        PackageManager::Apt => {
            let status = std::process::Command::new("sudo")
                .args(["apt", "update"])
                .status()
                .map_err(|e| anyhow::anyhow!("failed to run apt update: {e}"))?;
            if !status.success() {
                anyhow::bail!("`sudo apt update` failed ({status})");
            }
            ("sudo", vec!["apt", "install", "-y"])
        }
        PackageManager::Dnf => ("sudo", vec!["dnf", "install", "-y"]),
        PackageManager::Pacman => ("sudo", vec!["pacman", "-S", "--needed", "--noconfirm"]),
        PackageManager::Zypper => ("sudo", vec!["zypper", "install", "-y"]),
        PackageManager::Apk => ("sudo", vec!["apk", "add"]),
        PackageManager::Brew => ("brew", vec!["install"]),
        PackageManager::Winget => ("winget", vec!["install"]),
        PackageManager::Choco => ("choco", vec!["install", "-y"]),
        PackageManager::Unknown => anyhow::bail!("unknown package manager"),
    };
    let mut cmd = std::process::Command::new(program);
    cmd.args(&args);
    for package in packages {
        cmd.arg(package);
    }
    let status = cmd
        .status()
        .map_err(|e| anyhow::anyhow!("failed to run {program}: {e} — is it on PATH?"))?;
    if !status.success() {
        anyhow::bail!(
            "install failed ({status}); try manually: {}",
            install_command(manager, packages)
        );
    }
    Ok(())
}

/// Download `dest` from the first mirror in `urls` that succeeds.
///
/// Retries each mirror once, validates the body is non-empty and not an
/// HTML error page, and writes atomically (temp file + rename) so a failed
/// download never leaves a corrupt tarball behind.
pub fn download_to_file(urls: &[&str], dest: &std::path::Path, label: &str) -> Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .user_agent(format!("morphc/{}", env!("CARGO_PKG_VERSION")))
        .build()?;
    let mut last_err = String::from("no mirrors listed");
    for url in urls {
        for attempt in 1..=2 {
            eprintln!("  … Downloading {label} (attempt {attempt}/2) ...");
            match fetch_one(&client, url) {
                Ok(bytes) => {
                    let tmp = dest.with_extension("part");
                    std::fs::write(&tmp, &bytes)?;
                    std::fs::rename(&tmp, dest)?;
                    return Ok(());
                }
                Err(e) => {
                    last_err = format!("{url}: {e}");
                    eprintln!("  ⚠ Download failed ({url}): {e}");
                }
            }
        }
    }
    anyhow::bail!("Could not download {label}: {last_err}")
}

fn fetch_one(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>> {
    let resp = client.get(url).send().map_err(|e| anyhow::anyhow!("request failed: {e}"))?;
    if !resp.status().is_success() {
        anyhow::bail!("HTTP {} for {url}", resp.status());
    }
    let bytes = resp.bytes().map_err(|e| anyhow::anyhow!("reading body failed: {e}"))?.to_vec();
    if bytes.is_empty() {
        anyhow::bail!("empty response");
    }
    if bytes.starts_with(b"<!DOCTYPE") || bytes.starts_with(b"<html") {
        anyhow::bail!("server returned an HTML error page");
    }
    Ok(bytes)
}

fn path_has(bin: &str) -> bool {
    std::env::var("PATH").is_ok_and(|p| {
        std::env::split_paths(&p).any(|d| {
            d.join(bin).exists()
                || d.join(format!("{bin}.exe")).exists()
                || d.join(format!("{bin}.cmd")).exists()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_manager_knows_toolchain_and_text_libs() {
        for manager in [
            PackageManager::Apt,
            PackageManager::Dnf,
            PackageManager::Pacman,
            PackageManager::Zypper,
            PackageManager::Apk,
            PackageManager::Brew,
        ] {
            assert!(!packages_for(manager, "toolchain").is_empty(), "{manager:?}");
            assert!(!packages_for(manager, "freetype").is_empty(), "{manager:?}");
            assert!(!packages_for(manager, "harfbuzz").is_empty(), "{manager:?}");
            assert!(!packages_for(manager, "glfw").is_empty(), "{manager:?}");
        }
    }

    #[test]
    fn install_command_mentions_packages() {
        let cmd = install_command(PackageManager::Apt, &["libglfw3-dev".to_string()]);
        assert!(cmd.contains("apt"));
        assert!(cmd.contains("libglfw3-dev"));
    }

    #[test]
    fn library_checks_cover_mandatory_trio() {
        let names: Vec<&str> = check_libraries().iter().map(|c| c.name).collect();
        assert!(names.contains(&"GLFW"));
        assert!(names.contains(&"FreeType"));
        assert!(names.contains(&"HarfBuzz"));
    }

    #[test]
    fn parses_dumpversion_majors() {
        assert_eq!(major_of_dump("14"), Some(14));
        assert_eq!(major_of_dump("13.3.0"), Some(13));
        assert_eq!(major_of_dump("17.0.0"), Some(17));
        assert_eq!(major_of_dump(""), None);
        assert_eq!(major_of_dump("unknown"), None);
    }

    #[test]
    fn picked_compiler_meets_cxx23_floor() {
        let cxx = check_cpp_compiler();
        assert!(!(cxx.found_via.is_empty()));
        if cxx.found_via.contains("clang") {
            assert_eq!(
                cxx.ok,
                compiler_major(&cxx.found_via).is_some_and(|m| m >= MIN_CLANG_MAJOR)
            );
        } else {
            assert_eq!(cxx.ok, compiler_major(&cxx.found_via).is_some_and(|m| m >= MIN_GCC_MAJOR));
        }
    }
}
