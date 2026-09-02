use anyhow::Result;
use colored::Colorize;
use std::path::PathBuf;

pub fn run(
    entry: Option<String>,
    output: Option<String>,
    static_: bool,
    upx: Option<bool>,
    no_upx: bool,
    suppress_banner: bool,
) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry = entry.unwrap_or(config.entry.clone());
    let output_raw = output.unwrap_or(config.output.clone());
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
    crate::logger::log_key("Binary", &clean_name);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );
    if static_ {
        crate::logger::log_key("Static", "enabled");
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
    let mut builder = morph_ir::IRBuilder::new();
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

    // ── Compile (skip when nothing changed, like cargo run) ──
    let compiler_name = morph_build::detect_compiler();
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
    let compiler = morph_build::Compiler::new(None).silent();
    let binary_path = output_dir.join(format!("{}{}", clean_name, morph_build::exe_suffix()));

    // A build is "fresh" (cargo-style) only when every input fingerprint is
    // unchanged AND the binary already exists. On any change we rebuild.
    let config_text = std::fs::read_to_string(&config_path).unwrap_or_default();
    let runtime_hash = morph_cache::hash_tree(&runtime_dir);
    let mut owned_inputs: Vec<(String, String)> = vec![
        ("morph.config.json".to_string(), config_text),
        ("entry".to_string(), source.clone()),
        ("runtime".to_string(), runtime_hash),
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
    if let Err(e) = compiler.compile(&output_dir.join("app.cpp"), &binary_path, &runtime_dir, &defines) {
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
    // UPX compression if requested (stub)
    if upx.unwrap_or(false) && !no_upx {
        crate::logger::log_dim("UPX compression requested (not yet implemented in Rust, skipping)");
    }
    println!();

    Ok(binary_path)
}
