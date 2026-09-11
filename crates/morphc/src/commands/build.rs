use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

pub fn run(
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
    upx: Option<bool>,
    no_upx: bool,
    suppress_banner: bool,
    type_mode: Option<String>,
    upx_version: Option<String>,
) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry = entry.unwrap_or(config.entry.clone());
    let output_raw = output.unwrap_or(config.output.clone());
    let type_mode = type_mode.unwrap_or(config.type_mode.clone());
    let type_mode: morpher::TypeMode = type_mode.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    // Clean app name for binary (spaces/special → _)
    let clean_name = morph_config::clean_app_name(&config.name);

    if !suppress_banner {
        crate::logger::log_banner(&format!("Morph Build — {}", config.name));
    }

    crate::logger::log_step("Verifying runtime");
    crate::commands::install::ensure_runtime(&cwd)?;
    crate::logger::log_success(&format!(
        "Runtime {} v{}",
        config.runtime.runtime_type.cyan(),
        config.runtime.version.dimmed()
    ));

    crate::logger::log_step("Configuration");
    crate::logger::log_key("Entry", &entry);
    crate::logger::log_key("Output", &output_raw);
    crate::logger::log_key("Types", &type_mode.to_string());
    crate::logger::log_key("Binary", &clean_name);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );
    if static_ {
        crate::logger::log_key("Static", "enabled");
        crate::logger::log_key(
            "GLFW backend",
            if config.build.wayland { "Wayland + X11" } else { "X11 only (Wayland off)" },
        );
        if config.build.system_freetype {
            crate::logger::log_key("FreeType", "system archive (config.build.system_freetype)");
        }
    }
    if let Some(u) = upx {
        crate::logger::log_key("UPX", &u.to_string());
    }
    if no_upx {
        crate::logger::log_key("UPX", "disabled");
    }

    // ── Parse source ──
    crate::logger::log_step("Parsing");

    let pb = crate::logger::spinner("Parsing source...");
    let entry_path = cwd.join(&entry);
    if !entry_path.exists() {
        pb.finish_and_clear();
        anyhow::bail!("Entry file not found: {}", entry_path.display());
    }
    // Morph only supports strict .ts/.tsx/.mx entries — hard error otherwise.
    if let Err(msg) = morph_config::validate_entry_ext(&entry_path) {
        pb.finish_and_clear();
        anyhow::bail!(msg);
    }

    let source = std::fs::read_to_string(&entry_path)?;
    let parsed = morph_parser::parse_mx_str(&source, &entry)?;
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Parsed {}", entry));

    // Report what we found
    if let Some(ref wc) = parsed.window_config {
        crate::logger::log_key("Window", &format!("\"{}\" {}x{}", wc.title, wc.width, wc.height));
    }
    let total_state: usize = parsed.components.iter().map(|c| c.state_vars.len()).sum();
    crate::logger::log_key("Components", &parsed.components.len().to_string());
    crate::logger::log_key("State vars", &total_state.to_string());
    crate::logger::log_key("Imports", &parsed.imports.len().to_string());

    // ── Build IR (Phase 3: morph-ir builder) ──
    let pb = crate::logger::spinner("Building IR...");
    // Collect CSS rules from imports
    let mut css_rules: Vec<(String, morph_parser::CssRule)> = Vec::new();
    let mut css_keyframes: std::collections::HashMap<String, Vec<morph_parser::CssKeyframe>> = std::collections::HashMap::new();
    for imp in &parsed.imports {
        if let morph_parser::MxImportKind::CssLocal { path } = &imp.kind {
            let candidates = [
                entry_path.parent().map(|p| p.join(path)).unwrap_or_else(|| cwd.join(path)),
                cwd.join(path),
            ];
            for cand in &candidates {
                if cand.exists() {
                    if let Ok(text) = std::fs::read_to_string(cand) {
                        if let Ok(data) = morph_parser::parse_css(&text) {
                            css_rules.extend(data.rules);                            for (k, v) in data.keyframes { css_keyframes.entry(k).or_default().extend(v); }
                        }
                    }
                    break;
                }
            }
        }
    }
    let builder = morph_ir::IRBuilder::new();
    let windows = builder.build(&parsed, &css_rules, &css_keyframes);
    pb.finish_and_clear();
    crate::logger::log_success(&format!("IR built — {} window(s)", windows.len()));

    // ── Generate C++ ──
    let output_dir = cwd.join(&output_raw);
    // Ensure output is treated as directory (clean name handles file case)
    let output_dir = if output_raw.ends_with('/') || std::path::Path::new(&output_raw).extension().is_none() {
        output_dir
    } else {
        output_dir.parent().map(|p| p.to_path_buf()).unwrap_or(output_dir)
    };
    let pb = crate::logger::spinner("Generating C++...");
    let emitter = morph_codegen::CppEmitter::new(&windows);
    emitter.emit(&output_dir)?;
    pb.finish_and_clear();
    crate::logger::log_success(&format!("C++ generated → {}", output_dir.display()));

    // ── Translate companion TypeScript files into linkable fragments ──
    // Entry-referenced .ts plus src/**/*.ts, each translated in the
    // configured type mode and compiled+linked below. Failures here are
    // hard errors: silently dropping app logic would miscompile the app.
    let fragment_inputs = collect_typescript_sources(&cwd, &entry_path, &parsed.imports)?;
    let mut extra_sources: Vec<PathBuf> = Vec::new();
    if !fragment_inputs.is_empty() {
        let pb = crate::logger::spinner("Translating TypeScript...");
        let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (ts_path, ts_source) in &fragment_inputs {
            let filename = ts_path.file_name().and_then(|n| n.to_str()).unwrap_or("file.ts");
            let mut options = morpher::TranslateOptions::default();
            options.type_mode = type_mode;
            let code = morpher::translate_fragment(ts_source, filename, options)
                .map_err(|e| anyhow::anyhow!("translating {}: {}", ts_path.display(), e))?;
            let stem = ts_path.file_stem().and_then(|s| s.to_str()).unwrap_or("fragment");
            let mut out_name = format!("{}.ts.cpp", stem);
            let mut counter = 2;
            while !used_names.insert(out_name.clone()) {
                out_name = format!("{}_{}.ts.cpp", stem, counter);
                counter += 1;
            }
            let out_path = output_dir.join(&out_name);
            std::fs::write(&out_path, &code)?;
            extra_sources.push(out_path);
        }
        pb.finish_and_clear();
        crate::logger::log_success(&format!(
            "Translated {} TypeScript file(s)",
            fragment_inputs.len()
        ));
    }

    // ── Compile (skip when nothing changed, like cargo run) ──
    let compiler_name = morph_build::detect_compiler();
    // Compiler override: config build.cxx, then MORPH_CXX, then platform default.
    let cxx_override = if !config.build.cxx.is_empty() {
        Some(config.build.cxx.clone())
    } else {
        std::env::var("MORPH_CXX").ok().filter(|v| !v.is_empty())
    };
    // Find runtime dir (for headers)
    let runtime_dir = {
        let mut candidates = vec![
            cwd.join("runtime").join("cpp"),
            cwd.join("../runtime").join("cpp"),
            cwd.join("../../runtime").join("cpp"),
            cwd.join("../../../runtime").join("cpp"),
            cwd.join("../../../../runtime").join("cpp"),
            std::path::PathBuf::from("runtime/cpp"),
        ];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("../runtime/cpp"));
                candidates.push(dir.join("../../runtime/cpp"));
                candidates.push(dir.join("../../../runtime/cpp"));
            }
        }
        candidates.into_iter().find(|p| p.join("core/window.h").exists() || p.join("include").exists() || p.exists()).unwrap_or_else(|| cwd.join("runtime/cpp"))
    };
    let compiler = morph_build::Compiler::new(cxx_override.clone()).silent();
    let compiler_name = cxx_override.unwrap_or_else(morph_build::detect_compiler);
    let binary_path = output_dir.join(format!("{}{}", clean_name, morph_build::exe_suffix()));

    // A build is "fresh" (cargo-style) only when every input fingerprint is
    // unchanged AND the binary already exists. On any change we rebuild.
    let config_text = std::fs::read_to_string(&config_path).unwrap_or_default();
    let runtime_hash = morph_cache::hash_tree(&runtime_dir);
    let mut owned_inputs: Vec<(String, String)> = vec![
        ("morph.config.json".to_string(), config_text),
        ("entry".to_string(), source.clone()),
        ("runtime".to_string(), runtime_hash),
        ("static".to_string(), static_.to_string()),
    ];
    let entry_parent = entry_path.parent().unwrap_or(cwd.as_path());
    for imp in &parsed.imports {
        let path = match &imp.kind {
            morph_parser::MxImportKind::CssLocal { path } => path,
            morph_parser::MxImportKind::Component { path, .. } => path,
            morph_parser::MxImportKind::CppLocal { path, .. } => path,
            morph_parser::MxImportKind::CssUrl { .. } => continue,
        };
        let candidates = [
            entry_parent.join(path),
            cwd.join(path),
        ];
        let text = if let Some(cand) = candidates.iter().find(|c| c.exists()) {
            std::fs::read_to_string(cand).unwrap_or_default()
        } else {
            String::new()
        };
        owned_inputs.push((path.clone(), text));
    }
    for (ts_path, ts_source) in &fragment_inputs {
        owned_inputs.push((format!("ts:{}", ts_path.display()), ts_source.clone()));
    }
    let fingerprint_inputs: Vec<(&str, &str)> = owned_inputs
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_str()))
        .collect();
    let fingerprint = morph_cache::fingerprint_inputs(&fingerprint_inputs);
    let stored = morph_cache::read_stored_fingerprint(&cwd, &clean_name);
    if stored.as_deref() == Some(fingerprint.as_str()) && binary_path.exists() {
        crate::logger::log_success(&format!("Up to date — nothing to compile ({})", binary_path.display()));
        println!();
        return Ok(binary_path);
    }

    let pb = crate::logger::spinner(&format!("Compiling with {}...", compiler_name));
    // Feature defines for flex, etc.
    let mut fs = morph_codegen::feature_set::FeatureSet::new();
    fs.scan(&windows);
    let defines = fs.required_defines();
    // Ensure output dir exists (already created by emitter)
    let build_opts = morph_build::BuildOptions {
        static_mode: static_,
        wayland: config.build.wayland,
        system_freetype: config.build.system_freetype,
        native: morph_build::NativeFlags {
            include_dirs: config.native.include_dirs.clone(),
            library_dirs: config.native.library_dirs.clone(),
            libraries: config.native.libraries.clone(),
            cflags: config.native.cflags.clone(),
            ldflags: config.native.ldflags.clone(),
        },
    };
    if let Err(e) = compiler.compile_with_options(
        &output_dir.join("app.cpp"),
        &binary_path,
        &runtime_dir,
        &defines,
        &extra_sources,
        &build_opts,
    ) {
        pb.finish_and_clear();
        // On failure we STOP and do not run any stale binary.
        crate::logger::log_error(&format!("Compile failed: {}", e));
        crate::logger::log_error("Fix the error above, then re-run `morph run`/`morph build`.");
        anyhow::bail!("build failed");
    }
    pb.finish_and_clear();
    // Only record the fingerprint after a successful compile.
    let _ = morph_cache::write_stored_fingerprint(&cwd, &clean_name, &fingerprint);
    crate::logger::log_success(&format!("Compiled → {}", binary_path.display()));
    // UPX compression: config default is on; --upx / --no-upx override.
    let upx_enabled = if no_upx { false } else { upx.unwrap_or(config.build.upx) };
    let upx_version = upx_version.filter(|v| !v.is_empty()).or_else(|| {
        if config.build.upx_version.is_empty() {
            None
        } else {
            Some(config.build.upx_version.clone())
        }
    });
    if upx_enabled {
        if let Some(upx_bin) = morph_build::upx::ensure_upx(upx_version.as_deref(), true) {
            crate::logger::log_step("Compressing binary with UPX ...");
            if morph_build::upx::compress(&binary_path, &upx_bin) {
                crate::logger::log_success(&format!(
                    "UPX compressed → {}",
                    binary_path.display()
                ));
            }
        } else {
            crate::logger::log_dim("UPX not available, skipping compression");
        }
    }
    println!();

    Ok(binary_path)
}

/// Collect companion TypeScript sources: `.ts` Component imports plus
/// every `*.ts` under the entry directory (excluding the entry itself,
/// ambient `*.d.ts` declarations, generated output, and tooling dirs).
/// Returns canonical paths with contents in stable order. Anything
/// unreadable or unlisted-but-required is a hard error: silently dropping
/// app logic would miscompile the app.
fn collect_typescript_sources(
    cwd: &Path,
    entry_path: &Path,
    imports: &[morph_parser::MxImport],
) -> anyhow::Result<Vec<(std::path::PathBuf, String)>> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::HashSet<std::path::PathBuf> = std::collections::HashSet::new();
    let mut push_candidate = |path: std::path::PathBuf| {
        let canonical = path.canonicalize().unwrap_or(path);
        if seen.insert(canonical.clone()) {
            found.push(canonical);
        }
    };
    let entry_parent = entry_path.parent().unwrap_or(cwd);
    for imp in imports {
        if let morph_parser::MxImportKind::Component { path, .. } = &imp.kind {
            if path.ends_with(".ts") && !path.ends_with(".tsx") {
                let mut located = false;
                for base in [entry_parent, cwd] {
                    let candidate = base.join(path);
                    if candidate.is_file() {
                        push_candidate(candidate);
                        located = true;
                        break;
                    }
                }
                if !located {
                    anyhow::bail!("TypeScript import not found: {}", path);
                }
            }
        }
    }
    let src_dir = cwd.join("src");
    for search_root in [entry_parent, src_dir.as_path()] {
        if !search_root.is_dir() {
            continue;
        }
        for entry in walkdir::WalkDir::new(search_root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !name.ends_with(".ts") || name.ends_with(".tsx") || name.ends_with(".d.ts") {
                continue;
            }
            if name.ends_with(".ts.cpp") {
                continue;
            }
            let under_tooling = path.components().any(|component| {
                matches!(
                    component.as_os_str().to_str(),
                    Some(".morph" | "dist" | "target" | "node_modules")
                )
            });
            if under_tooling {
                continue;
            }
            push_candidate(path.to_path_buf());
        }
    }
    let mut sources = Vec::new();
    for path in found {
        if path == entry_path {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("cannot read TypeScript source {}: {}", path.display(), e)
        })?;
        sources.push((path, text));
    }
    sources.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_project(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("morph_build_test_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).unwrap();
        root
    }

    #[test]
    fn collects_companion_ts_sources() {
        let root = scratch_project("collect");
        std::fs::write(root.join("src/App.mx"), "<body/>").unwrap();
        std::fs::write(root.join("src/util.ts"), "export const x = 1;\n").unwrap();
        std::fs::write(root.join("src/env.d.ts"), "declare const y: number;\n").unwrap();
        let entry = root.join("src/App.mx");
        let found = collect_typescript_sources(&root, &entry, &[]).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["util.ts".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_component_import_is_hard_error() {
        let root = scratch_project("missing");
        std::fs::write(root.join("src/App.mx"), "<body/>").unwrap();
        let entry = root.join("src/App.mx");
        let imports = vec![morph_parser::MxImport {
            kind: morph_parser::MxImportKind::Component {
                path: "missing.ts".to_string(),
                specifiers: Vec::new(),
            },
            style: "import".to_string(),
        }];
        let err = collect_typescript_sources(&root, &entry, &imports).unwrap_err();
        assert!(err.to_string().contains("missing.ts"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
