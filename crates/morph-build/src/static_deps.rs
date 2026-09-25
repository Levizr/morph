//! Build minimal static archives for GLFW / FreeType / HarfBuzz on demand so
//! `morph build --static` produces the smallest possible self-contained binary.
//! Mirrors `morph/build/static_deps.py`.
//!
//! Each library is compiled with -Os, function/data sections and (when using
//! gcc) LTO; results are cached under `~/.cache/morph/static` (override with
//! `MORPH_STATIC_CACHE`). Source tarballs are looked up in `MORPH_SRC_DIR`
//! first, then downloaded.

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct DepSpec {
    version: &'static str,
    archive: &'static str,
    urls: &'static [&'static str],
    lib: &'static str,
}

const GLFW: DepSpec = DepSpec {
    version: "3.4",
    archive: "glfw-3.4.tar.gz",
    urls: &["https://github.com/glfw/glfw/archive/refs/tags/3.4.tar.gz"],
    lib: "libglfw3.a",
};
const FREETYPE: DepSpec = DepSpec {
    version: "2.13.3",
    archive: "freetype-2.13.3.tar.gz",
    urls: &[
        "https://download.savannah.gnu.org/releases/freetype/freetype-2.13.3.tar.gz",
        "https://github.com/freetype/freetype/archive/refs/tags/VER-2-13-3.tar.gz",
    ],
    lib: "libfreetype.a",
};
const HARFBUZZ: DepSpec = DepSpec {
    version: "8.5.0",
    archive: "harfbuzz-8.5.0.tar.gz",
    urls: &["https://github.com/harfbuzz/harfbuzz/archive/refs/tags/8.5.0.tar.gz"],
    lib: "libharfbuzz.a",
};

fn spec(dep: &str) -> &'static DepSpec {
    match dep {
        "glfw" => &GLFW,
        "freetype" => &FREETYPE,
        _ => &HARFBUZZ,
    }
}

/// FreeType modules morph actually uses; everything else is trimmed from
/// ftmodule.h before configuring (mirrors `_FT_MODULE_KEEP`).
const FT_MODULE_KEEP: &[&str] = &[
    "tt_driver_class",
    "sfnt_module_class",
    "ft_smooth_renderer_class",
    "ft_raster1_renderer_class",
];

/// HarfBuzz meson options (mirrors `_HB_WANTED_OPTS`).
const HB_WANTED_OPTS: &[(&str, &str)] = &[
    ("freetype", "enabled"),
    ("glib", "disabled"),
    ("gobject", "disabled"),
    ("cairo", "disabled"),
    ("fontconfig", "disabled"),
    ("icu", "disabled"),
    ("graphite", "disabled"),
    ("graphite2", "disabled"),
    ("introspection", "disabled"),
    ("gtk_doc", "disabled"),
    ("tests", "disabled"),
    ("tools", "disabled"),
    ("utilities", "disabled"),
    ("docs", "disabled"),
    ("benchmark", "disabled"),
    ("subset", "disabled"),
    ("wasm", "disabled"),
];

/// A resolved static dependency: archive path plus include dir when self-built.
#[derive(Debug, Clone)]
pub struct DepInfo {
    pub archive: PathBuf,
    pub self_built: bool,
    pub include_dir: Option<PathBuf>,
}

pub struct StaticDeps {
    gpp: String,
    wayland: bool,
    platform: String,
    cc: String,
    cxx: String,
    lto_tools: HashMap<String, String>,
    lto_ok: bool,
    cache: PathBuf,
    src_dir: PathBuf,
    freetype_prefix: Option<PathBuf>,
    manifest: HashMap<String, (String, String)>,
    silent: bool,
}

impl StaticDeps {
    pub fn new(gpp: &str, wayland: bool) -> Self {
        let is_clang = gpp.contains("clang");
        let (cc, cxx) = compiler_pair(gpp);
        // LTO objects must be indexed by LTO-enabled ar/ranlib; without them
        // (e.g. some MinGW installs), -flto would break archive creation.
        let lto_tools = lto_tools(&cc, is_clang);
        let lto_ok = !is_clang
            && dumpver(&cc) == dumpver(&cxx)
            && lto_tools.contains_key("ar")
            && lto_tools.contains_key("ranlib");
        let cache = std::env::var("MORPH_STATIC_CACHE")
            .map_or_else(|_| cache_base().join("morph").join("static"), PathBuf::from);
        let src_dir =
            std::env::var("MORPH_SRC_DIR").map_or_else(|_| cache.join("src"), PathBuf::from);
        let manifest = load_manifest(&cache);
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        }
        .to_string();
        Self {
            gpp: gpp.to_string(),
            wayland,
            platform,
            cc,
            cxx,
            lto_tools,
            lto_ok,
            cache,
            src_dir,
            freetype_prefix: None,
            manifest,
            silent: false,
        }
    }

    pub const fn silent(mut self) -> Self {
        self.silent = true;
        self
    }

    /// The compiler executable this builder was configured with.
    pub fn compiler(&self) -> &str {
        &self.gpp
    }

    /// Build (or reuse the cached) static archive for `dep`.
    pub fn build(&mut self, dep: &str) -> Result<DepInfo> {
        let s = spec(dep);
        let prefix = self.cache.join("prefix").join(dep);
        let cur_hash = self.build_hash(dep);
        if let Some(cached) = find_archive(&prefix, s.lib) {
            if let Some((hash, _)) = self.manifest.get(dep) {
                if hash == &cur_hash {
                    return Ok(self.info(dep, &prefix, cached));
                }
            }
        }
        if !self.silent {
            eprintln!("  ✓ Building {} {} from source (minimal static)...", dep, s.version);
        }
        reset_dir(&prefix)?;
        self.build_dep(dep, &prefix)?;
        let archive = find_archive(&prefix, s.lib).with_context(|| {
            format!("Build produced no archive ({}) under {}", s.lib, prefix.display())
        })?;
        self.manifest.insert(dep.to_string(), (cur_hash, s.version.to_string()));
        save_manifest(&self.cache, &self.manifest);
        if !self.silent {
            eprintln!("  ✓ {} {} built at {}", dep, s.version, archive.display());
        }
        Ok(self.info(dep, &prefix, archive))
    }

    fn info(&self, dep: &str, prefix: &Path, archive: PathBuf) -> DepInfo {
        let mut info = DepInfo { archive, self_built: true, include_dir: None };
        let want = match dep {
            "freetype" => Some("freetype2"),
            "harfbuzz" => Some("harfbuzz"),
            _ => None,
        };
        if let Some(name) = want {
            info.include_dir = find_include(prefix, name);
        }
        info
    }

    fn build_hash(&self, dep: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        dep.hash(&mut h);
        spec(dep).version.hash(&mut h);
        self.gpp.hash(&mut h);
        self.cc.hash(&mut h);
        self.cxx.hash(&mut h);
        self.platform.hash(&mut h);
        if dep == "glfw" {
            self.wayland.hash(&mut h);
            // Gamepad DB trim (see trim_glfw_gamepad): part of the build
            // identity so old untrimmed archives never get reused.
            "nogamepad-v1".hash(&mut h);
        }
        // Dependency compile flags are build identity too (unwind tables
        // etc. change object bytes without changing versions).
        self.cflags_extra().hash(&mut h);
        Self::hb_extra_flags().hash(&mut h);
        if dep == "freetype" {
            let mut keep = FT_MODULE_KEEP.to_vec();
            keep.sort_unstable();
            keep.join(",").hash(&mut h);
        }
        if dep == "harfbuzz" {
            self.freetype_include_args().join("|").hash(&mut h);
        }
        format!("{:016x}", h.finish())
    }

    fn cflags_extra(&self) -> String {
        // Lean static archives: size-first flags mirror the app link
        // (-Oz rule lives in the app crate; deps use -Os which LTO
        // reshapes anyway). Unwind tables are pure dead weight in
        // dependency C code — no consumer ever unwinds through them.
        let mut flags = vec![
            "-Os",
            "-ffunction-sections",
            "-fdata-sections",
            "-fno-asynchronous-unwind-tables",
            "-fno-unwind-tables",
        ];
        if self.lto_ok {
            flags.push("-flto");
        }
        flags.join(" ")
    }

    fn run(&self, cmd: &[String], step: &str, env_extra: &[(&str, &str)]) -> Result<()> {
        if !self.silent {
            eprintln!("  … {step} ...");
        }
        let mut c = std::process::Command::new(&cmd[0]);
        c.args(&cmd[1..]);
        for (k, v) in env_extra {
            c.env(k, v);
        }
        let out = c.output().with_context(|| format!("failed to run {step}"))?;
        if !out.status.success() {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            for line in combined.lines().rev().take(60).collect::<Vec<_>>().into_iter().rev() {
                eprintln!("    {line}");
            }
            anyhow::bail!("{step} failed");
        }
        Ok(())
    }

    fn source_tarball(&self, dep: &str) -> Result<PathBuf> {
        let s = spec(dep);
        let path = self.src_dir.join(s.archive);
        if path.is_file() {
            return Ok(path);
        }
        std::fs::create_dir_all(&self.src_dir)?;
        crate::doctor::download_to_file(s.urls, &path, s.archive)
            .with_context(|| format!("Could not obtain {}", s.archive))?;
        Ok(path)
    }

    fn extract(&self, dep: &str, tarball: &Path) -> Result<PathBuf> {
        let work = self.cache.join("build");
        std::fs::create_dir_all(&work)?;
        let dst = work.join(dep);
        if dst.exists() {
            std::fs::remove_dir_all(&dst)?;
        }
        std::fs::create_dir_all(&dst)?;
        let status = std::process::Command::new("tar")
            .args(["-xf"])
            .arg(tarball)
            .args(["-C"])
            .arg(&dst)
            .status()
            .with_context(|| "extracting sources needs `tar` on PATH")?;
        if !status.success() {
            anyhow::bail!("Could not extract {}", tarball.display());
        }
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&dst)?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| !p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with('.')))
            .collect();
        entries.sort();
        if entries.len() == 1 {
            return Ok(entries.remove(0));
        }
        Ok(dst)
    }

    fn cmake_lto_tools(&self, args: &mut Vec<String>) {
        if !self.lto_ok {
            return;
        }
        for (name, var) in [("ar", "CMAKE_AR"), ("ranlib", "CMAKE_RANLIB"), ("nm", "CMAKE_NM")] {
            if let Some(p) = self.lto_tools.get(name) {
                args.push(format!("-D{var}={p}"));
            }
        }
    }

    fn build_dep(&mut self, dep: &str, prefix: &Path) -> Result<()> {
        match dep {
            "glfw" => self.build_glfw(prefix),
            "freetype" => self.build_freetype(prefix),
            _ => self.build_harfbuzz(prefix),
        }
    }

    fn glfw_platform_flags(&self) -> Vec<String> {
        match self.platform.as_str() {
            "macos" => {
                vec!["-DGLFW_BUILD_COCOA=ON", "-DGLFW_BUILD_WAYLAND=OFF", "-DGLFW_BUILD_X11=OFF"]
            }
            "windows" => {
                vec!["-DGLFW_BUILD_WIN32=ON", "-DGLFW_BUILD_WAYLAND=OFF", "-DGLFW_BUILD_X11=OFF"]
            }
            _ => vec![
                "-DGLFW_BUILD_X11=ON",
                if self.wayland { "-DGLFW_BUILD_WAYLAND=ON" } else { "-DGLFW_BUILD_WAYLAND=OFF" },
            ],
        }
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    fn build_glfw(&self, prefix: &Path) -> Result<()> {
        let tarball = self.source_tarball("glfw")?;
        let src = self.extract("glfw", &tarball)?;
        let build = self.cache.join("build").join("glfw-build");
        reset_dir(&build)?;
        Self::trim_glfw_gamepad(&src)?;
        let mut args = vec![
            "cmake".to_string(),
            "-S".to_string(),
            src.display().to_string(),
            "-B".to_string(),
            build.display().to_string(),
            "-DCMAKE_BUILD_TYPE=Release".to_string(),
            format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()),
            format!("-DCMAKE_C_COMPILER={}", self.cc),
            format!("-DCMAKE_CXX_COMPILER={}", self.cxx),
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON".to_string(),
            "-DBUILD_SHARED_LIBS=OFF".to_string(),
            "-DGLFW_BUILD_EXAMPLES=OFF".to_string(),
            "-DGLFW_BUILD_TESTS=OFF".to_string(),
            "-DGLFW_BUILD_DOCS=OFF".to_string(),
            "-DGLFW_INSTALL=ON".to_string(),
        ];
        args.extend(self.glfw_platform_flags());
        args.push(format!("-DCMAKE_C_FLAGS={}", self.cflags_extra()));
        self.cmake_lto_tools(&mut args);
        self.run(&args, "Configuring glfw", &[])?;
        self.run(
            &["cmake".into(), "--build".into(), build.display().to_string(), "--parallel".into()],
            "Compiling glfw",
            &[],
        )?;
        self.run(
            &["cmake".into(), "--install".into(), build.display().to_string()],
            "Installing glfw",
            &[],
        )
    }

    /// Drop GLFW's built-in gamepad mapping database (257KB source).
    /// Morph exposes no gamepad API, so the DB is 100% dead weight that
    /// `glfwInit` would otherwise link unconditionally. The stub keeps
    /// every gamepad function callable (they just find no mappings).
    /// The source tree is a per-build scratch copy, like ftmodule.
    fn trim_glfw_gamepad(src: &Path) -> Result<()> {
        let input_c = src.join("glfw-3.4").join("src").join("input.c");
        // Tarball extracts to glfw-3.4/ — fall back to a flat src/ layout.
        let input_c = if input_c.exists() { input_c } else { src.join("src").join("input.c") };
        let text = std::fs::read_to_string(&input_c)
            .with_context(|| format!("reading {}", input_c.display()))?;
        let marker = "void _glfwInitGamepadMappings(void)";
        let start = text.find(marker).with_context(|| "gamepad init not found")?;
        let brace = text[start..].find('{').with_context(|| "gamepad body not found")? + start;
        let mut depth = 0;
        let mut end = None;
        for (i, ch) in text[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(brace + i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end.with_context(|| "gamepad body end not found")?;
        let mut out = text[..brace].to_string();
        out.push_str("{\n    (void)0; /* trimmed by morph --static: no gamepad API surface */\n}");
        out.push_str(&text[end..]);
        std::fs::write(&input_c, out)?;
        Ok(())
    }

    fn write_trimmed_ftmodule(&self, src: &Path) -> Result<()> {
        // Overwrite freetype's ftmodule.h with only the modules morph uses.
        // The source tree is a per-build scratch copy (reset on every build),
        // so patching it directly is safe and avoids fragile quoting.
        let module_h = src.join("include").join("freetype").join("config").join("ftmodule.h");
        let mut keep = Vec::new();
        if let Ok(text) = std::fs::read_to_string(&module_h) {
            for line in text.lines() {
                if let Some(start) = line.find("FT_USE_MODULE") {
                    let rest = &line[start..];
                    if let (Some(lp), Some(rp)) = (rest.find('('), rest.find(')')) {
                        let args = &rest[lp + 1..rp];
                        if let Some(name) = args.split(',').nth(1).map(str::trim) {
                            if FT_MODULE_KEEP.contains(&name) {
                                keep.push(line.to_string());
                            }
                        }
                    }
                }
            }
        }
        let mut out = String::from("/* trimmed by morph --static: only modules morph uses */\n");
        for line in keep {
            out.push_str(&line);
            out.push('\n');
        }
        std::fs::write(&module_h, out)?;
        Ok(())
    }

    fn build_freetype(&mut self, prefix: &Path) -> Result<()> {
        let tarball = self.source_tarball("freetype")?;
        let src = self.extract("freetype", &tarball)?;
        let build = self.cache.join("build").join("freetype-build");
        reset_dir(&build)?;
        self.write_trimmed_ftmodule(&src)?;
        let mut args = vec![
            "cmake".to_string(),
            "-S".to_string(),
            src.display().to_string(),
            "-B".to_string(),
            build.display().to_string(),
            "-DCMAKE_BUILD_TYPE=MinSizeRel".to_string(),
            format!("-DCMAKE_INSTALL_PREFIX={}", prefix.display()),
            format!("-DCMAKE_C_COMPILER={}", self.cc),
            format!("-DCMAKE_CXX_COMPILER={}", self.cxx),
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON".to_string(),
            "-DBUILD_SHARED_LIBS=OFF".to_string(),
            "-DFT_DISABLE_ZLIB=ON".to_string(),
            "-DFT_DISABLE_BZIP2=ON".to_string(),
            "-DFT_DISABLE_PNG=ON".to_string(),
            "-DFT_DISABLE_HARFBUZZ=ON".to_string(),
            "-DFT_DISABLE_BROTLI=ON".to_string(),
            format!("-DCMAKE_C_FLAGS={}", self.cflags_extra()),
        ];
        self.cmake_lto_tools(&mut args);
        self.run(&args, "Configuring freetype", &[])?;
        self.run(
            &["cmake".into(), "--build".into(), build.display().to_string(), "--parallel".into()],
            "Compiling freetype",
            &[],
        )?;
        self.run(
            &["cmake".into(), "--install".into(), build.display().to_string()],
            "Installing freetype",
            &[],
        )?;
        ensure_freetype_pc(prefix)?;
        self.freetype_prefix = Some(prefix.to_path_buf());
        Ok(())
    }

    fn freetype_include_args(&self) -> Vec<String> {
        if let Some(prefix) = &self.freetype_prefix {
            let inc = prefix.join("include").join("freetype2");
            if inc.is_dir() {
                return vec![format!("-I{}", inc.display())];
            }
        }
        pkg_config("freetype2", "--cflags")
    }

    fn meson(&self) -> Result<String> {
        if path_has("meson") && path_has("ninja") {
            return Ok("meson".to_string());
        }
        let venv = self.cache.join("venv");
        let scripts = if cfg!(target_os = "windows") { "Scripts" } else { "bin" };
        let exe = if cfg!(target_os = "windows") { "meson.exe" } else { "meson" };
        let meson_bin = venv.join(scripts).join(exe);
        if meson_bin.is_file() {
            return Ok(meson_bin.display().to_string());
        }
        if !self.silent {
            eprintln!("  ✓ Bootstrapping meson+ninja (needed to build harfbuzz)...");
        }
        let pip_exe = if cfg!(target_os = "windows") { "pip.exe" } else { "pip" };
        let pip = venv.join(scripts).join(pip_exe);
        let py = if path_has("python3") { "python3" } else { "python" };
        self.run(
            &[py.into(), "-m".into(), "venv".into(), venv.display().to_string()],
            "Creating build venv",
            &[],
        )
        .with_context(|| {
            "Could not create a Python venv. Install meson+ninja or python3-venv, \
             or place a harfbuzz static archive in MORPH_STATIC_LIBDIRS."
        })?;
        self.run(
            &[
                pip.display().to_string(),
                "install".into(),
                "--quiet".into(),
                "meson".into(),
                "ninja".into(),
            ],
            "Installing meson+ninja",
            &[],
        )
        .with_context(|| "pip install meson ninja failed - check network access.")?;
        Ok(meson_bin.display().to_string())
    }

    fn meson_opts(&self, src: &Path) -> Vec<String> {
        // Only pass options the harfbuzz release actually declares (mirrors
        // `_meson_opts`, which reads meson_options.txt).
        let available: std::collections::HashSet<String> =
            std::fs::read_to_string(src.join("meson_options.txt"))
                .map(|text| {
                    text.match_indices("option")
                        .filter_map(|(i, _)| {
                            let rest = &text[i..];
                            let q1 = rest.find('\'')? + 1;
                            let q2 = rest[q1..].find('\'')? + q1;
                            Some(rest[q1..q2].to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
        let opts_file_missing = available.is_empty();
        HB_WANTED_OPTS
            .iter()
            .filter(|(name, _)| opts_file_missing || available.contains(*name))
            .map(|(name, value)| format!("-D{name}={value}"))
            .collect()
    }

    /// Extra C/C++ flags for the HarfBuzz meson build (single source so
    /// the cache hash below stays in sync with what actually compiles).
    const fn hb_extra_flags() -> [&'static str; 5] {
        [
            "-DHB_MINI",
            "-ffunction-sections",
            "-fdata-sections",
            "-fno-asynchronous-unwind-tables",
            "-fno-unwind-tables",
        ]
    }

    fn build_harfbuzz(&self, prefix: &Path) -> Result<()> {
        let meson = self.meson()?;
        let tarball = self.source_tarball("harfbuzz")?;
        let src = self.extract("harfbuzz", &tarball)?;
        let build = self.cache.join("build").join("harfbuzz-build");
        reset_dir(&build)?;
        let mut opts = self.meson_opts(&src);
        let extra = Self::hb_extra_flags();
        let ft_args = self.freetype_include_args();
        let c_args: Vec<String> = extra.iter().map(ToString::to_string).chain(ft_args).collect();
        let cpp_args = c_args.clone();
        if self.lto_ok {
            opts.push("-Db_lto=true".to_string());
        }
        opts.push(format!("-Dc_args={}", c_args.join(" ")));
        opts.push(format!("-Dcpp_args={}", cpp_args.join(" ")));
        let mut args = vec![
            meson,
            "setup".to_string(),
            build.display().to_string(),
            src.display().to_string(),
            "--buildtype=minsize".to_string(),
            "--default-library=static".to_string(),
            format!("--prefix={}", prefix.display()),
        ];
        args.extend(opts);
        let pkg_path = pkgconfig_dirs(prefix);
        let existing = std::env::var("PKG_CONFIG_PATH").unwrap_or_default();
        let mut env_extra: Vec<(String, String)> =
            vec![("CC".to_string(), self.cc.clone()), ("CXX".to_string(), self.cxx.clone())];
        if !pkg_path.is_empty() {
            let joined = if existing.is_empty() {
                pkg_path.join(":")
            } else {
                format!("{}:{}", pkg_path.join(":"), existing)
            };
            env_extra.push(("PKG_CONFIG_PATH".to_string(), joined));
        }
        let env_refs: Vec<(&str, &str)> =
            env_extra.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        let meson_bin = args[0].clone();
        self.run(&args, "Configuring harfbuzz", &env_refs)?;
        self.run(
            &[meson_bin.clone(), "compile".into(), "-C".into(), build.display().to_string()],
            "Compiling harfbuzz",
            &env_refs,
        )?;
        self.run(
            &[meson_bin, "install".into(), "-C".into(), build.display().to_string()],
            "Installing harfbuzz",
            &env_refs,
        )
    }
}

fn cache_base() -> PathBuf {
    std::env::var("XDG_CACHE_HOME").map_or_else(
        |_| {
            std::env::var("HOME")
                .map_or_else(|_| PathBuf::from("~/.cache"), |h| PathBuf::from(h).join(".cache"))
        },
        PathBuf::from,
    )
}

pub fn path_has(bin: &str) -> bool {
    std::env::var("PATH").is_ok_and(|p| std::env::split_paths(&p).any(|d| d.join(bin).exists()))
}

fn compiler_pair(gpp: &str) -> (String, String) {
    let base = Path::new(gpp).file_name().and_then(|n| n.to_str()).unwrap_or(gpp);
    if base.contains("clang") {
        let cc = if path_has("clang") { "clang".to_string() } else { "clang".to_string() };
        return (cc, base.to_string());
    }
    // Handles g++, g++-14, and MinGW x86_64-w64-mingw32-g++.
    if let Some(prefix) = base.strip_suffix("g++") {
        let ccname = format!("{prefix}gcc");
        return (ccname, base.to_string());
    }
    (String::from("gcc"), base.to_string())
}

fn dumpver(exe: &str) -> Option<String> {
    std::process::Command::new(exe)
        .arg("-dumpversion")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// LTO-aware ar/ranlib/nm for the active gcc (gcc-ar etc.).
fn lto_tools(cc: &str, is_clang: bool) -> HashMap<String, String> {
    let mut tools = HashMap::new();
    if is_clang {
        return tools;
    }
    let ccbase = Path::new(cc).file_name().and_then(|n| n.to_str()).unwrap_or(cc);
    for name in ["ar", "ranlib", "nm"] {
        let mut found = None;
        if ccbase.contains("gcc") {
            let cand = ccbase.replacen("gcc", &format!("gcc-{name}"), 1);
            if path_has(&cand) {
                found = Some(cand);
            }
        }
        if found.is_none() {
            let cand = format!("gcc-{name}");
            if path_has(&cand) {
                found = Some(cand);
            }
        }
        if let Some(p) = found {
            tools.insert(name.to_string(), p);
        }
    }
    tools
}

fn find_archive(prefix: &Path, libname: &str) -> Option<PathBuf> {
    let mut stack = vec![prefix.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|n| n.to_str()) == Some(libname) {
                return Some(path);
            }
        }
    }
    None
}

fn find_include(prefix: &Path, name: &str) -> Option<PathBuf> {
    let direct = prefix.join("include").join(name);
    if direct.is_dir() {
        return Some(direct);
    }
    let mut stack = vec![prefix.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some(name)
                    && std::fs::read_dir(&path).ok()?.next().is_some()
                {
                    return Some(path);
                }
                stack.push(path);
            }
        }
    }
    None
}

fn pkgconfig_dirs(prefix: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![prefix.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.filter_map(std::result::Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some("pkgconfig") {
                    out.push(path.display().to_string());
                } else {
                    stack.push(path);
                }
            }
        }
    }
    out
}

fn reset_dir(d: &Path) -> Result<()> {
    if d.exists() {
        std::fs::remove_dir_all(d)?;
    }
    std::fs::create_dir_all(d)?;
    Ok(())
}

fn load_manifest(cache: &Path) -> HashMap<String, (String, String)> {
    let text = std::fs::read_to_string(cache.join("manifest.json")).unwrap_or_default();
    let parsed: HashMap<String, HashMap<String, String>> =
        serde_json::from_str(&text).unwrap_or_default();
    parsed
        .into_iter()
        .filter_map(|(k, v)| Some((k, (v.get("hash")?.clone(), v.get("version")?.clone()))))
        .collect()
}

fn save_manifest(cache: &Path, manifest: &HashMap<String, (String, String)>) {
    let _ = std::fs::create_dir_all(cache);
    let obj: HashMap<String, HashMap<String, String>> = manifest
        .iter()
        .map(|(k, (h, v))| {
            let mut m = HashMap::new();
            m.insert("hash".to_string(), h.clone());
            m.insert("version".to_string(), v.clone());
            (k.clone(), m)
        })
        .collect();
    if let Ok(text) = serde_json::to_string_pretty(&obj) {
        let _ = std::fs::write(cache.join("manifest.json"), text);
    }
}

fn ensure_freetype_pc(prefix: &Path) -> Result<()> {
    let pc = prefix.join("lib").join("pkgconfig").join("freetype2.pc");
    if pc.is_file() {
        return Ok(());
    }
    if let Some(dir) = pc.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        &pc,
        format!(
            "prefix={0}\n\
             includedir={0}/include\n\
             libdir={0}/lib\n\
             Name: freetype2\n\
             Description: FreeType 2 font engine (morph static build)\n\
             Version: 24.3.18\n\
             Libs: -L${{libdir}} -lfreetype\n\
             Cflags: -I${{includedir}}/freetype2\n",
            prefix.display()
        ),
    )?;
    Ok(())
}

fn pkg_config(pkg: &str, flag: &str) -> Vec<String> {
    std::process::Command::new("pkg-config")
        .args([flag, pkg])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout).split_whitespace().map(str::to_string).collect()
        })
        .unwrap_or_default()
}
