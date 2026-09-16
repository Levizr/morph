//! Hot-reload logic compilation (dev mode).
//!
//! Mirrors `morph/build/compiler.py` (`_dev_shared_flags`,
//! `compile_shared`): the dev logic `.so` is compiled with the exact flag
//! set the dev runtime uses, so header-only `MorphNode` / `MorphStyle`
//! layouts match between `morph_devrt` and the dlopen'ed library.

use std::path::Path;

use anyhow::{Context, Result};

use super::{append_pkg_config, system_include_dirs};
use crate::{shared_lib_flag, Compiler, NativeFlags};

/// Layout-affecting feature defines for the dev logic library.
///
/// MUST match the feature set of `morph_devrt`
/// (`runtime/cpp/dev/CMakeLists.txt`): any divergence changes the class
/// layout between the host binary and the logic library (ODR mismatch →
/// garbage `std::function` → SIGSEGV on handlers).
pub const DEV_FEATURES: &[&str] = &[
    "MORPH_FEATURE_SCROLL",
    "MORPH_FEATURE_RADIUS",
    "MORPH_FEATURE_TEXT",
    "MORPH_FEATURE_BOLD",
    "MORPH_FEATURE_POSITION",
    "MORPH_FEATURE_ZINDEX",
    "MORPH_FEATURE_OPACITY",
    "MORPH_FEATURE_FLEX",
    "MORPH_FEATURE_CURSOR",
    "MORPH_FEATURE_BORDER",
    "MORPH_FEATURE_DISPLAY_NONE",
    "MORPH_FEATURE_INLINE",
    "MORPH_FEATURE_MARGIN_COLLAPSE",
    "MORPH_FEATURE_MIN_MAX",
    "MORPH_FEATURE_BORDER_BOX",
    "MORPH_FEATURE_IMAGE",
    "MORPH_FEATURE_DIRTY_RENDERING",
    "MORPH_FEATURE_INPUT",
    "MORPH_FEATURE_TRANSFORM",
    "MORPH_FEATURE_ANIMATION",
    "MORPH_FEATURE_DEV",
];

/// Flag set shared by dev logic compiles.
///
/// C++23, like the app compile: generated and user-native code may use
/// `std::println` (`<print>`), which does not exist in C++20 mode.
pub fn dev_shared_flags(
    runtime_dir: &Path,
    extra_defines: &[String],
    native: &NativeFlags,
) -> Vec<String> {
    let mut cmd = vec![
        "-std=c++23".to_string(),
        "-O0".to_string(),
        shared_lib_flag().to_string(),
        "-fPIC".to_string(),
        "-I".to_string(),
        runtime_dir.display().to_string(),
        "-I".to_string(),
        runtime_dir.join("dev").display().to_string(),
        "-I".to_string(),
        runtime_dir.join("vendor").display().to_string(),
    ];
    for feature in DEV_FEATURES {
        cmd.push(format!("-D{feature}"));
    }
    for define in extra_defines {
        cmd.push(format!("-D{define}"));
    }
    for dir in &native.include_dirs {
        if !dir.is_empty() {
            cmd.push("-I".to_string());
            cmd.push(dir.clone());
        }
    }
    for dir in &native.library_dirs {
        if !dir.is_empty() {
            cmd.push("-L".to_string());
            cmd.push(dir.clone());
        }
    }
    for lib in &native.libraries {
        if !lib.is_empty() {
            cmd.push(format!("-l{lib}"));
        }
    }
    cmd.extend(native.cflags.iter().cloned());
    cmd.extend(native.ldflags.iter().cloned());
    // FreeType/HarfBuzz headers (needed by node.h → gl_renderer.h), with
    // the same system fallbacks as the app compile.
    let have_ft = append_pkg_config(&mut cmd, "freetype2", "--cflags");
    if !have_ft {
        cmd.extend(system_include_dirs().iter().map(|d| format!("-I{d}/freetype2")));
    }
    let have_hb = append_pkg_config(&mut cmd, "harfbuzz", "--cflags");
    if !have_hb {
        cmd.extend(system_include_dirs().iter().map(|d| format!("-I{d}/harfbuzz")));
    }
    cmd
}

impl Compiler {
    /// Compile `source_path` → shared library for dev hot-reload.
    ///
    /// Captures compiler output; on failure the stderr lines are printed
    /// and an error is returned (mirrors Python `compile_shared`).
    pub fn compile_shared(
        &self,
        source_path: &Path,
        output_path: &Path,
        runtime_dir: &Path,
        defines: &[String],
        native: &NativeFlags,
    ) -> Result<()> {
        if !source_path.exists() {
            anyhow::bail!("logic source not found: {}", source_path.display());
        }
        let mut cmd = vec![self.gpp.clone()];
        cmd.extend(dev_shared_flags(runtime_dir, defines, native));
        cmd.push(source_path.display().to_string());
        cmd.push("-o".to_string());
        cmd.push(output_path.display().to_string());

        if !self.silent {
            println!("  $ {}", cmd.join(" "));
        }
        let output = std::process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .output()
            .with_context(|| format!("failed to execute compiler: {}", cmd[0]))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            for line in stderr.lines().filter(|l| !l.trim().is_empty()) {
                eprintln!("  {}", line.trim());
            }
            anyhow::bail!("shared library compilation failed: {}", output.status);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_flags_carry_feature_set_and_includes() {
        let runtime = Path::new("/rt");
        let flags = dev_shared_flags(runtime, &[], &NativeFlags::default());
        assert!(flags.contains(&"-std=c++23".to_string()));
        assert!(flags.contains(&"-O0".to_string()));
        assert!(flags.contains(&"-DMORPH_FEATURE_DEV".to_string()));
        assert!(flags.contains(&"-DMORPH_FEATURE_TRANSFORM".to_string()));
        assert!(flags.contains(&"-DMORPH_FEATURE_ANIMATION".to_string()));
        assert!(flags
            .windows(2)
            .any(|w| { w[0] == "-I" && w[1] == runtime.join("dev").display().to_string() }));
    }

    #[test]
    fn dev_flags_include_extra_defines_and_native() {
        let native = NativeFlags {
            include_dirs: vec!["/inc".to_string()],
            cflags: vec!["-march=native".to_string()],
            ..NativeFlags::default()
        };
        let flags = dev_shared_flags(Path::new("/rt"), &["MORPH_EXTRA=1".to_string()], &native);
        assert!(flags.contains(&"-DMORPH_EXTRA=1".to_string()));
        assert!(flags.contains(&"-march=native".to_string()));
        assert!(flags.windows(2).any(|w| w[0] == "-I" && w[1] == "/inc"));
    }
}
