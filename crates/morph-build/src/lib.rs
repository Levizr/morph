pub mod platform;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub use platform::{current, is_macos, is_windows, is_linux, exe_suffix, shared_lib_ext, shared_lib_flag, pick_cpp};

pub fn detect_compiler() -> String {
    pick_cpp()
}

fn which(bin: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(bin).exists() { return true; }
            if is_windows() && dir.join(format!("{}.exe", bin)).exists() { return true; }
        }
    }
    false
}

pub struct Compiler {
    pub gpp: String,
    pub silent: bool,
}

impl Compiler {
    pub fn new(cxx: Option<String>) -> Self {
        let default = pick_cpp();
        let gpp = if let Some(c) = cxx {
            if which(&c) { c } else {
                eprintln!("  ⚠ configured compiler '{}' not found — falling back to '{}'", c, default);
                default
            }
        } else { default };
        Self { gpp, silent: false }
    }

    /// Suppress printing of the raw compiler command line.
    pub fn silent(mut self) -> Self {
        self.silent = true;
        self
    }

    /// The runtime .cpp sources that must be compiled and linked alongside the
    /// generated app.cpp (mirrors morph/build/compiler.py runtime_sources).
    fn runtime_sources(&self, runtime_dir: &Path) -> Vec<PathBuf> {
        let node_dir = runtime_dir.join("core/node");
        let mut srcs = vec![
            node_dir.join("node.cpp"),
            node_dir.join("events.cpp"),
            node_dir.join("flatten.cpp"),
            node_dir.join("style.cpp"),
            node_dir.join("animation.cpp"),
            node_dir.join("layout.cpp"),
            node_dir.join("paint_order.cpp"),
            runtime_dir.join("core/window.cpp"),
            runtime_dir.join("render/gl_renderer.cpp"),
            runtime_dir.join("core/compositor.cpp"),
            runtime_dir.join("reactivity/effect.cpp"),
            runtime_dir.join("reactivity/task.cpp"),
            runtime_dir.join("net/net.cpp"),
            runtime_dir.join("renderers/renderer.cpp"),
            runtime_dir.join("renderers/flash/flash.cpp"),
            runtime_dir.join("renderers/forge/forge.cpp"),
            runtime_dir.join("renderers/forge/damage.cpp"),
        ];
        // Only include sources that actually exist (some may be optional)
        srcs.retain(|p| p.exists());
        srcs
    }

    /// Compile `source_path` → `binary_path` (executable)
    pub fn compile(&self, source_path: &Path, binary_path: &Path, runtime_dir: &Path, defines: &[String]) -> Result<()> {
        let mut cmd = vec![self.gpp.clone()];
        cmd.push("-std=c++20".into());
        cmd.push("-O2".into());
        cmd.push("-ffunction-sections".into());
        cmd.push("-fdata-sections".into());
        // Include runtime headers
        cmd.push(format!("-I{}", runtime_dir.display()));
        cmd.push(format!("-I{}", runtime_dir.join("include").display()));
        cmd.push(format!("-I{}", runtime_dir.join("vendor").display()));
        cmd.push(format!("-I{}", runtime_dir.join("renderers").display()));
        // Generated app source
        cmd.push(source_path.display().to_string());
        // Runtime .cpp sources
        for s in self.runtime_sources(runtime_dir) {
            cmd.push(s.display().to_string());
        }
        // Vendor C sources
        let vendor_glad = runtime_dir.join("vendor/glad/glad.c");
        let vendor_stb = runtime_dir.join("vendor/stb_image.c");
        if vendor_glad.exists() { cmd.push(vendor_glad.display().to_string()); }
        if vendor_stb.exists() { cmd.push(vendor_stb.display().to_string()); }
        // Output
        cmd.push("-o".into());
        cmd.push(binary_path.display().to_string());
        // GC dead code eliminated by the renderer backend dispatch
        cmd.push(if is_macos() { "-Wl,-dead_strip".to_string() } else { "-Wl,--gc-sections".to_string() });

        // Platform libs
        if is_macos() {
            cmd.extend(["-framework".into(), "Cocoa".into(), "-framework".into(), "OpenGL".into(), "-framework".into(), "IOKit".into(), "-framework".into(), "CoreVideo".into(), "-lpthread".into()]);
        } else if is_windows() {
            cmd.extend(["-lopengl32".into(), "-lgdi32".into(), "-lshell32".into(), "-luser32".into(), "-lcomdlg32".into(), "-lole32".into(), "-lws2_32".into(), "-lpthread".into(), "-lm".into()]);
        } else {
            cmd.extend(["-lglfw".into(), "-lGL".into(), "-lX11".into(), "-lXrandr".into(), "-lXinerama".into(), "-lXcursor".into(), "-lXi".into(), "-lrt".into(), "-lpthread".into(), "-ldl".into(), "-lm".into()]);
        }

        // Feature defines
        for d in defines {
            cmd.push(format!("-D{}", d));
        }

        // FreeType/HarfBuzz via pkg-config if available
        for dep in &["freetype2", "harfbuzz"] {
            if let Ok(cflags) = std::process::Command::new("pkg-config").args(["--cflags", dep]).output() {
                if cflags.status.success() {
                    if let Ok(s) = String::from_utf8(cflags.stdout) {
                        for flag in s.split_whitespace() { cmd.push(flag.to_string()); }
                    }
                }
            }
            if let Ok(libs) = std::process::Command::new("pkg-config").args(["--libs", dep]).output() {
                if libs.status.success() {
                    if let Ok(s) = String::from_utf8(libs.stdout) {
                        for flag in s.split_whitespace() { cmd.push(flag.to_string()); }
                    }
                }
            }
        }

        if !self.silent {
            println!("  $ {}", cmd.join(" "));
        }

        let status = std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .status()
            .with_context(|| format!("failed to execute compiler: {}", cmd[0]))?;

        if !status.success() {
            anyhow::bail!("compilation failed with status: {}", status);
        }
        Ok(())
    }

    /// Compile shared library for hot-reload (dev mode)
    pub fn compile_shared(&self, source_path: &Path, output_path: &Path, runtime_dir: &Path) -> Result<()> {
        let mut cmd = vec![self.gpp.clone()];
        cmd.push("-std=c++20".into());
        cmd.push("-O2".into());
        cmd.push("-fPIC".into());
        cmd.push(shared_lib_flag().into());
        cmd.push(format!("-I{}", runtime_dir.display()));
        cmd.push(source_path.display().to_string());
        cmd.push("-o".into());
        cmd.push(output_path.display().to_string());
        // Minimal libs for shared
        if is_macos() {
            cmd.extend(["-framework".into(), "Cocoa".into()]);
        } else if !is_windows() {
            cmd.extend(["-lGL".into(), "-lX11".into()]);
        }

        if !self.silent {
            println!("  $ {}", cmd.join(" "));
        }

        let status = std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .status()
            .with_context(|| format!("failed to execute compiler: {}", cmd[0]))?;

        if !status.success() {
            anyhow::bail!("shared compilation failed: {}", status);
        }
        Ok(())
    }
}

pub fn build_project(project_dir: &Path) -> Result<()> {
    println!("  Building project at {}", project_dir.display());
    println!("  Using compiler: {}", detect_compiler());
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
