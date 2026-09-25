use anyhow::Result;
use colored::Colorize;
use morph_build::doctor as sys;

pub(crate) fn run(verbose: bool, yes: bool) -> Result<()> {
    crate::logger::log_banner("Morph Doctor — System Check");
    println!("      {} {}", "OS:".dimmed(), sys::os_description().bold());

    crate::logger::log_step("Required tools");
    let checks = morph_build::check_system()?;
    let mut missing_keys: Vec<&str> = Vec::new();
    let mut all_ok = true;
    for (name, ok, version) in &checks {
        if matches!(*name, "g++" | "clang++") {
            continue;
        }
        let required = matches!(*name, "cmake" | "pkg-config");
        if !ok && required {
            all_ok = false;
            missing_keys.push("toolchain");
        }
        print_check(name, *ok, version, verbose, required);
    }
    if !which("make") && !which("gmake") {
        all_ok = false;
        missing_keys.push("toolchain");
        print_check("make", false, "", verbose, true);
    } else if verbose {
        print_check("make", true, &get_version("make"), verbose, false);
    }

    crate::logger::log_step("C++ compiler (C++23 — g++-14+ required)");
    let cxx = sys::check_cpp_compiler();
    if !cxx.ok {
        all_ok = false;
        missing_keys.push("toolchain");
    }
    let cxx_label = format!("C++ ({})", cxx.found_via);
    print_check(&cxx_label, cxx.ok, &cxx.version, verbose, true);

    crate::logger::log_step("Required libraries (mandatory)");
    for dep in sys::check_libraries() {
        if !dep.ok {
            all_ok = false;
            match dep.name {
                "GLFW" => missing_keys.push("glfw"),
                "FreeType" => missing_keys.push("freetype"),
                "HarfBuzz" => missing_keys.push("harfbuzz"),
                _ => {}
            }
        }
        let detail = if dep.ok && !dep.version.is_empty() {
            dep.version.clone()
        } else if dep.ok {
            format!("found via {}", dep.found_via)
        } else {
            format!("missing (pkg-config {})", dep.pkg)
        };
        print_check(dep.name, dep.ok, &detail, verbose, true);
    }

    if morph_build::is_linux() {
        crate::logger::log_step("Platform graphics");
        let gl_ok = pkg_ok("gl") || path_exists("/usr/include/GL/gl.h");
        let x11_ok = pkg_ok("x11") || path_exists("/usr/include/X11/Xlib.h");
        if !gl_ok {
            all_ok = false;
            missing_keys.push("gl");
        }
        if !x11_ok {
            all_ok = false;
            missing_keys.push("x11");
        }
        print_check("OpenGL", gl_ok, &pkg_version("gl"), verbose, true);
        print_check("X11", x11_ok, &pkg_version("x11"), verbose, true);
    }

    println!();
    if all_ok {
        crate::logger::log_success("All checks passed!");
        println!();
        return Ok(());
    }

    missing_keys.sort_unstable();
    missing_keys.dedup();
    let manager = sys::detect_package_manager();
    let mut packages: Vec<String> = Vec::new();
    for key in &missing_keys {
        packages.extend(sys::packages_for(manager, key));
    }
    packages.sort();
    packages.dedup();

    if packages.is_empty() {
        crate::logger::log_warn("Some checks failed and no install map exists for this OS.");
        crate::logger::log_bullet("Install a C++ toolchain, GLFW, FreeType and HarfBuzz manually.");
        println!();
        anyhow::bail!("morph doctor found missing mandatory dependencies");
    }

    let command = sys::install_command(manager, &packages);
    crate::logger::log_warn("Missing mandatory dependencies:");
    crate::logger::log_bullet(&format!("Manager: {}", manager.name().cyan()));
    crate::logger::log_bullet(&format!("Install: {}", command.yellow()));
    println!();

    if yes || confirm_install(&command)? {
        crate::logger::log_step("Auto-installing missing packages");
        sys::auto_install(manager, &packages)?;
        crate::logger::log_success("Install finished — re-running checks...");
        println!();
        return run(false, false);
    }

    crate::logger::log_bullet(&format!(
        "Re-run with {} to auto-install.",
        "morph doctor -y".cyan()
    ));
    println!();
    anyhow::bail!("morph doctor found missing mandatory dependencies");
}

fn print_check(name: &str, ok: bool, version: &str, verbose: bool, required: bool) {
    let status = if ok { "✓".green().bold().to_string() } else { "✗".red().bold().to_string() };
    let tag = if !required { "  (optional)".dimmed().to_string() } else { String::new() };
    let ver = if verbose && !version.is_empty() {
        format!("  {}", version.dimmed())
    } else if !version.is_empty() {
        format!("  {}", version.lines().next().unwrap_or("").dimmed())
    } else {
        String::new()
    };
    println!("      {} {:<14}{}{}", status, name.bold(), ver, tag);
}

fn confirm_install(command: &str) -> Result<bool> {
    if !console::user_attended() {
        return Ok(false);
    }
    Ok(dialoguer::Confirm::new()
        .with_prompt(format!("Run `{command}` now?"))
        .default(false)
        .interact()?)
}

fn which(bin: &str) -> bool {
    std::env::var("PATH").is_ok_and(|p| {
        std::env::split_paths(&p).any(|d| {
            d.join(bin).exists()
                || d.join(format!("{bin}.exe")).exists()
                || d.join(format!("{bin}.cmd")).exists()
        })
    })
}

fn get_version(bin: &str) -> String {
    std::process::Command::new(bin)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.lines().next().map(ToString::to_string))
        .unwrap_or_default()
}

fn pkg_ok(pkg: &str) -> bool {
    std::process::Command::new("pkg-config")
        .args(["--exists", pkg])
        .status()
        .is_ok_and(|s| s.success())
}

fn pkg_version(pkg: &str) -> String {
    std::process::Command::new("pkg-config")
        .args(["--modversion", pkg])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}
