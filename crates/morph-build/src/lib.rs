pub mod css_fetch;
pub mod platform;
pub mod static_deps;
pub mod upx;

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub use platform::{
    current, exe_suffix, is_linux, is_macos, is_windows, pick_cpp, shared_lib_ext, shared_lib_flag,
};

pub fn detect_compiler() -> String {
    pick_cpp()
}

fn which(bin: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(bin).exists() {
                return true;
            }
            if is_windows() && dir.join(format!("{}.exe", bin)).exists() {
                return true;
            }
        }
    }
    false
}

pub struct Compiler {
    pub gpp: String,
    pub silent: bool,
}

/// External native build options from `morph.config.json` (`native` key).
/// Mirrors Python `NativeConfig` → `_native_flags`.
#[derive(Debug, Clone, Default)]
pub struct NativeFlags {
    pub include_dirs: Vec<String>,
    pub library_dirs: Vec<String>,
    pub libraries: Vec<String>,
    pub cflags: Vec<String>,
    pub ldflags: Vec<String>,
}

/// Link/build options for [`Compiler::compile_with_options`].
/// Mirrors the `static` / `wayland` / `system_freetype` / `native` arguments
/// of Python `Compiler.compile`.
#[derive(Debug, Clone, Default)]
pub struct BuildOptions {
    /// Bundle GLFW/FreeType/HarfBuzz as static archives (`--static`).
    pub static_mode: bool,
    /// Build GLFW with the Wayland backend plus X11 (Linux only).
    pub wayland: bool,
    /// Prefer the system FreeType archive instead of building the trimmed one.
    pub system_freetype: bool,
    /// User include dirs / libraries / extra flags.
    pub native: NativeFlags,
}

impl Compiler {
    pub fn new(cxx: Option<String>) -> Self {
        let default = pick_cpp();
        let gpp = if let Some(c) = cxx {
            if which(&c) {
                c
            } else {
                eprintln!(
                    "  ⚠ configured compiler '{}' not found — falling back to '{}'",
                    c, default
                );
                default
            }
        } else {
            default
        };
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
    pub fn compile(
        &self,
        source_path: &Path,
        binary_path: &Path,
        runtime_dir: &Path,
        defines: &[String],
        extra_sources: &[PathBuf],
    ) -> Result<()> {
        self.compile_with_options(
            source_path,
            binary_path,
            runtime_dir,
            defines,
            extra_sources,
            &BuildOptions::default(),
        )
    }

    /// Compile `source_path` → `binary_path` with static/wayland/native options.
    pub fn compile_with_options(
        &self,
        source_path: &Path,
        binary_path: &Path,
        runtime_dir: &Path,
        defines: &[String],
        extra_sources: &[PathBuf],
        opts: &BuildOptions,
    ) -> Result<()> {
        let mut cmd = vec![self.gpp.clone()];
        // C++23: generated code uses std::println (<print>), which does not
        // exist in C++20 mode. pick_cpp already prefers g++-14 for this.
        cmd.push("-std=c++23".into());
        if opts.static_mode {
            let (opt, size_flags) = static_size_flags(&self.gpp);
            if !opt.is_empty() {
                cmd.push(opt);
            }
            cmd.extend(size_flags);
        } else {
            cmd.push("-O2".into());
        }
        cmd.push("-ffunction-sections".into());
        cmd.push("-fdata-sections".into());
        // Include runtime headers
        cmd.push(format!("-I{}", runtime_dir.display()));
        cmd.push(format!("-I{}", runtime_dir.join("include").display()));
        cmd.push(format!("-I{}", runtime_dir.join("vendor").display()));
        cmd.push(format!("-I{}", runtime_dir.join("renderers").display()));
        // Generated app source
        cmd.push(source_path.display().to_string());
        // Translated TypeScript fragments linked into the app
        for extra in extra_sources {
            cmd.push(extra.display().to_string());
        }
        // Runtime .cpp sources
        for s in self.runtime_sources(runtime_dir) {
            cmd.push(s.display().to_string());
        }
        // Vendor C sources
        let vendor_glad = runtime_dir.join("vendor/glad/glad.c");
        let vendor_stb = runtime_dir.join("vendor/stb_image.c");
        if vendor_glad.exists() {
            cmd.push(vendor_glad.display().to_string());
        }
        if vendor_stb.exists() {
            cmd.push(vendor_stb.display().to_string());
        }
        // Output
        cmd.push("-o".into());
        cmd.push(binary_path.display().to_string());
        // GC dead code eliminated by the renderer backend dispatch
        cmd.push(if is_macos() {
            "-Wl,-dead_strip".to_string()
        } else {
            "-Wl,--gc-sections".to_string()
        });

        // Platform libs: static archives are bundled into the binary so the
        // app runs on a stock machine; only the universal base (GL,
        // X11/Cocoa/Win32, libc, libstdc++) stays dynamic.
        let mut archives: std::collections::HashMap<String, static_deps::DepInfo> =
            std::collections::HashMap::new();
        if opts.static_mode {
            archives = resolve_static_archives(&self.gpp, opts, self.silent)?;
            let ft = archives.get("freetype");
            let hb = archives.get("harfbuzz");
            let glfw = archives
                .get("glfw")
                .with_context(|| "static build is missing the GLFW archive")?;
            let mut static_libs = vec![glfw.archive.display().to_string()];
            if let Some(ft) = ft {
                static_libs.push(ft.archive.display().to_string());
                if !ft.self_built {
                    // Transitive static closure for the freetype static
                    // archive on Debian/Ubuntu.
                    static_libs.extend(
                        ["-lbz2", "-lz", "-lpng16", "-lbrotlidec", "-lbrotlicommon"]
                            .iter()
                            .map(|s| s.to_string()),
                    );
                }
            }
            if let Some(hb) = hb {
                static_libs.push(hb.archive.display().to_string());
            }
            let dynamic: Vec<String> = if is_macos() {
                vec![
                    "-framework", "Cocoa", "-framework", "OpenGL", "-framework", "IOKit",
                    "-framework", "CoreVideo", "-lpthread",
                ]
            } else if is_windows() {
                vec![
                    "-lopengl32", "-lgdi32", "-lshell32", "-luser32", "-lcomdlg32",
                    "-lole32", "-lsetupapi", "-lws2_32", "-lpthread", "-lm",
                ]
            } else {
                let mut d = vec![
                    "-lGL", "-lX11", "-lXrandr", "-lXinerama", "-lXcursor", "-lXi",
                    "-lrt", "-lpthread", "-ldl", "-lm",
                ];
                if opts.wayland {
                    // GLFW Wayland backend: EGL + wayland-client/xkbcommon
                    // stay dynamic (they are system libs, like X11/GL).
                    d.splice(
                        1..1,
                        [
                            "-lwayland-client",
                            "-lwayland-cursor",
                            "-lwayland-egl",
                            "-lxkbcommon",
                            "-lEGL",
                        ],
                    );
                }
                d
            }
            .into_iter()
            .map(str::to_string)
            .collect();
            if is_macos() {
                // ld64 has no -Bstatic; pass the archives directly.
                cmd.extend(static_libs);
                cmd.extend(dynamic);
            } else {
                cmd.push("-Wl,-Bstatic".into());
                cmd.extend(static_libs);
                cmd.push("-Wl,-Bdynamic".into());
                cmd.extend(dynamic);
            }
        } else if is_macos() {
            cmd.extend([
                "-framework".into(),
                "Cocoa".into(),
                "-framework".into(),
                "OpenGL".into(),
                "-framework".into(),
                "IOKit".into(),
                "-framework".into(),
                "CoreVideo".into(),
                "-lpthread".into(),
            ]);
        } else if is_windows() {
            cmd.extend([
                "-lopengl32".into(),
                "-lgdi32".into(),
                "-lshell32".into(),
                "-luser32".into(),
                "-lcomdlg32".into(),
                "-lole32".into(),
                "-lws2_32".into(),
                "-lpthread".into(),
                "-lm".into(),
            ]);
        } else {
            cmd.extend([
                "-lglfw".into(),
                "-lGL".into(),
                "-lX11".into(),
                "-lXrandr".into(),
                "-lXinerama".into(),
                "-lXcursor".into(),
                "-lXi".into(),
                "-lrt".into(),
                "-lpthread".into(),
                "-ldl".into(),
                "-lm".into(),
            ]);
        }

        // Feature defines
        for d in defines {
            cmd.push(format!("-D{}", d));
        }

        // FreeType/HarfBuzz: self-built static prefixes contribute only their
        // include dir (the archive itself is already on the link line);
        // otherwise pkg-config as usual, with the same fallbacks as Python
        // (`-lfreetype` / `-lharfbuzz` + system include roots).
        let ft_self = archives.get("freetype").map(|d| d.self_built).unwrap_or(false);
        let hb_self = archives.get("harfbuzz").map(|d| d.self_built).unwrap_or(false);
        if ft_self {
            if let Some(dir) = archives.get("freetype").and_then(|d| d.include_dir.clone()) {
                cmd.push(format!("-I{}", dir.display()));
            }
        } else {
            let have_cflags = append_pkg_config(&mut cmd, "freetype2", "--cflags");
            if !opts.static_mode {
                if !append_pkg_config(&mut cmd, "freetype2", "--libs") {
                    cmd.push("-lfreetype".into());
                }
                if !have_cflags {
                    cmd.extend(system_include_dirs().iter().map(|d| format!("-I{}/freetype2", d)));
                }
            } else if !have_cflags {
                cmd.extend(system_include_dirs().iter().map(|d| format!("-I{}/freetype2", d)));
            }
        }
        if hb_self {
            if let Some(dir) = archives.get("harfbuzz").and_then(|d| d.include_dir.clone()) {
                cmd.push(format!("-I{}", dir.display()));
            }
        } else {
            let have_cflags = append_pkg_config(&mut cmd, "harfbuzz", "--cflags");
            if !opts.static_mode {
                if !append_pkg_config(&mut cmd, "harfbuzz", "--libs") {
                    cmd.push("-lharfbuzz".into());
                }
                if !have_cflags {
                    cmd.extend(system_include_dirs().iter().map(|d| format!("-I{}/harfbuzz", d)));
                }
            } else if !have_cflags {
                // Static system archive: the archive is already on the link
                // line, only the headers still need a fallback path.
                cmd.extend(system_include_dirs().iter().map(|d| format!("-I{}/harfbuzz", d)));
            }
        }

        // User native build options (external includes/libs/cflags).
        for d in &opts.native.include_dirs {
            if !d.is_empty() {
                cmd.push("-I".to_string());
                cmd.push(d.clone());
            }
        }
        for d in &opts.native.library_dirs {
            if !d.is_empty() {
                cmd.push("-L".to_string());
                cmd.push(d.clone());
            }
        }
        for lib in &opts.native.libraries {
            if !lib.is_empty() {
                cmd.push(format!("-l{}", lib));
            }
        }
        cmd.extend(opts.native.cflags.iter().cloned());
        cmd.extend(opts.native.ldflags.iter().cloned());

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
    pub fn compile_shared(
        &self,
        source_path: &Path,
        output_path: &Path,
        runtime_dir: &Path,
    ) -> Result<()> {
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

/// `pkg-config --cflags/--libs` for one package; appends on success.
/// Returns true when pkg-config knew the package.
fn append_pkg_config(cmd: &mut Vec<String>, pkg: &str, flag: &str) -> bool {
    let out = std::process::Command::new("pkg-config").args([flag, pkg]).output();
    match out {
        Ok(o) if o.status.success() => {
            if let Ok(s) = String::from_utf8(o.stdout) {
                for f in s.split_whitespace() {
                    cmd.push(f.to_string());
                }
            }
            true
        }
        _ => false,
    }
}

/// OS-appropriate fallback include roots when pkg-config is missing.
fn system_include_dirs() -> Vec<&'static str> {
    if is_macos() {
        vec!["/opt/homebrew/include", "/usr/local/include"]
    } else if is_windows() {
        vec![]
    } else {
        vec!["/usr/include"]
    }
}

/// Size flags for `--static` builds: `-Os -s` (+ `-flto` unless clang),
/// overridable wholesale via `MORPH_STATIC_CFLAGS`. Returns (opt, extras).
fn static_size_flags(gpp: &str) -> (String, Vec<String>) {
    if let Ok(override_flags) = std::env::var("MORPH_STATIC_CFLAGS") {
        let extras: Vec<String> =
            override_flags.split_whitespace().map(str::to_string).collect();
        return (String::new(), extras);
    }
    let mut extras = vec!["-s".to_string()];
    if !gpp.contains("clang") {
        extras.insert(0, "-flto".to_string());
    }
    ("-Os".to_string(), extras)
}

/// Locate (or build from source) the static archives for a `--static` build.
/// Order matters: freetype resolves before harfbuzz, because a self-built
/// freetype is what harfbuzz links its shape engine against.
fn resolve_static_archives(
    gpp: &str,
    opts: &BuildOptions,
    silent: bool,
) -> Result<std::collections::HashMap<String, static_deps::DepInfo>> {
    let mut deps = static_deps::StaticDeps::new(gpp, opts.wayland);
    if silent {
        deps = deps.silent();
    }
    let mut info = std::collections::HashMap::new();
    resolve_one(&mut info, &mut deps, "glfw", false)?;
    // Default: build the trimmed freetype ourselves (smallest result).
    // system_freetype prefers the system archive instead.
    resolve_one(&mut info, &mut deps, "freetype", !opts.system_freetype)?;
    resolve_one(&mut info, &mut deps, "harfbuzz", false)?;
    Ok(info)
}

fn resolve_one(
    info: &mut std::collections::HashMap<String, static_deps::DepInfo>,
    deps: &mut static_deps::StaticDeps,
    key: &str,
    force_build: bool,
) -> Result<()> {
    let names: &[&str] = match key {
        "glfw" => &["libglfw3.a", "libglfw.a"],
        "harfbuzz" => &["libharfbuzz.a"],
        _ => &["libfreetype.a"],
    };
    if !force_build {
        if let Some(found) = find_static_archive(names, &deps_gpp(deps)) {
            info.insert(
                key.to_string(),
                static_deps::DepInfo { archive: found, self_built: false, include_dir: None },
            );
            return Ok(());
        }
    }
    if std::env::var("MORPH_NO_BUILD_DEPS").is_ok() {
        anyhow::bail!(
            "Static build needs a static archive for {} ({}). Install it \
             (e.g. libglfw3-dev, libharfbuzz-dev) or build it and point \
             MORPH_STATIC_LIBDIRS at its directory.",
            key,
            names.join(" / ")
        );
    }
    match deps.build(key) {
        Ok(built) => {
            info.insert(key.to_string(), built);
            return Ok(());
        }
        Err(e) => {
            // Fall back to a system archive if the source build failed.
            if let Some(fallback) = find_static_archive(names, &deps_gpp(deps)) {
                eprintln!(
                    "  ⚠ Could not build {} from source ({}) — falling back to {} \
                     (bigger binary, fewer size optimizations).",
                    key,
                    e,
                    fallback.display()
                );
                info.insert(
                    key.to_string(),
                    static_deps::DepInfo {
                        archive: fallback,
                        self_built: false,
                        include_dir: None,
                    },
                );
                return Ok(());
            }
            return Err(e);
        }
    }
}

fn static_lib_dirs(gpp: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("MORPH_STATIC_LIBDIRS") {
        dirs.extend(std::env::split_paths(&env));
    }
    if is_macos() {
        dirs.extend(
            ["/opt/homebrew/lib", "/usr/local/lib", "/usr/lib"].iter().map(PathBuf::from),
        );
    } else if is_windows() {
        // MinGW static archives live alongside the compiler install.
        if let Some(exe) = path_which(gpp) {
            if let Some(base) = exe
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
            {
                dirs.push(base.join("lib"));
                dirs.push(base.join("x86_64-w64-mingw32").join("lib"));
            }
        }
        for cand in ["/mingw64/lib", "/mingw32/lib"] {
            let p = PathBuf::from(cand);
            if p.is_dir() {
                dirs.push(p);
            }
        }
    } else {
        dirs.extend(
            [
                "/usr/lib/x86_64-linux-gnu",
                "/usr/lib/aarch64-linux-gnu",
                "/usr/lib/arm-linux-gnueabihf",
                "/usr/local/lib",
                "/usr/lib",
            ]
            .iter()
            .map(PathBuf::from),
        );
    }
    dirs
}

fn deps_gpp(deps: &static_deps::StaticDeps) -> String {
    deps.compiler().to_string()
}

fn find_static_archive(names: &[&str], gpp: &str) -> Option<PathBuf> {
    for d in static_lib_dirs(gpp) {
        for n in names {
            let p = d.join(n);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

fn path_which(bin: &str) -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join(bin);
            if cand.is_file() {
                return Some(cand);
            }
            if is_windows() {
                let exe = dir.join(format!("{}.exe", bin));
                if exe.is_file() {
                    return Some(exe);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_flags_default_to_os_and_strip() {
        let (opt, extras) = static_size_flags("g++");
        assert_eq!(opt, "-Os");
        assert!(extras.contains(&"-s".to_string()));
        assert!(extras.contains(&"-flto".to_string()));
    }

    #[test]
    fn static_flags_skip_lto_under_clang() {
        let (opt, extras) = static_size_flags("clang++");
        assert_eq!(opt, "-Os");
        assert!(!extras.contains(&"-flto".to_string()));
    }

    #[test]
    fn static_flags_env_override_replaces_all() {
        std::env::set_var("MORPH_STATIC_CFLAGS", "-Oz -g0");
        let (opt, extras) = static_size_flags("g++");
        std::env::remove_var("MORPH_STATIC_CFLAGS");
        assert!(opt.is_empty());
        assert_eq!(extras, vec!["-Oz".to_string(), "-g0".to_string()]);
    }

    #[test]
    fn build_options_default_to_dynamic() {
        let opts = BuildOptions::default();
        assert!(!opts.static_mode);
        assert!(!opts.wayland);
        assert!(!opts.system_freetype);
    }
}
