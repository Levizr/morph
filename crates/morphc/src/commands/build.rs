use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};

// Long build pipeline: parse → IR → codegen → compile.
// Function kept whole to preserve step ordering.
#[allow(clippy::too_many_lines)]
pub(crate) fn run(
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
    let entry = entry.unwrap_or_else(|| config.entry.clone());
    let output_raw = output.unwrap_or_else(|| config.output.clone());
    let type_mode = type_mode.unwrap_or_else(|| config.type_mode.clone());
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
    let _parsed = morph_parser::parse_mx_str(&source, &entry)?;
    // Transitive .mx component graph: missing imports are hard errors.
    let graph = morph_parser::resolve_graph(&entry_path, &cwd).inspect_err(|_e| {
        pb.finish_and_clear();
    })?;
    let entry_mod = graph.entry_module().ok_or_else(|| {
        pb.finish_and_clear();
        anyhow::anyhow!("component graph has no entry module")
    })?;
    // Use the graph's entry source so transitive parsing stays consistent.
    let parsed = &entry_mod.source;
    pb.finish_and_clear();
    crate::logger::log_success(&format!("Parsed {} ({} module(s))", entry, graph.len()));

    // ── Lint gate: unresolved identifiers are hard errors ──
    // `morph check` reports them; `morph build` refuses to compile them.
    // Every module in the graph is checked (imports count as declared).
    {
        let mut lint_errors = 0;
        for mod_path in graph.all_paths() {
            let content = match std::fs::read_to_string(mod_path) {
                Ok(content) => content,
                Err(e) => anyhow::bail!("reading {}: {e}", mod_path.display()),
            };
            for err in morph_parser::linter::check(&content, &mod_path.display().to_string()) {
                if err.severity == "error" {
                    lint_errors += 1;
                    crate::logger::log_error(&format!(
                        "{}: {}: {} ({}:{})",
                        err.code,
                        err.message,
                        mod_path.display(),
                        err.line,
                        err.col
                    ));
                    if let Some(hint) = &err.suggestion {
                        crate::logger::log_dim(&format!("  hint: {hint}"));
                    }
                    if err.code.starts_with("mx-") {
                        println!(
                            "  {}  {}",
                            "Learn more:".dimmed(),
                            morph_parser::docs_url(&err.code)
                        );
                    }
                }
            }
        }
        if lint_errors > 0 {
            anyhow::bail!(
                "build blocked by {lint_errors} lint error(s) — run `morph check` for details"
            );
        }
    }

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
    // Collect CSS rules from every module in the component graph.
    let mut css_rules: Vec<(String, morph_parser::CssRule)> = Vec::new();
    let mut css_keyframes: std::collections::HashMap<String, Vec<morph_parser::CssKeyframe>> =
        std::collections::HashMap::new();
    let mut remote_css: Vec<(String, String)> = Vec::new();
    let css_fetcher =
        morph_build::css_fetch::CssFetcher::new(cwd.join(".morph").join("css-cache")).silent();
    for mod_path in graph.all_paths() {
        let Some(resolved) = graph.get(mod_path) else {
            continue;
        };
        for imp in &resolved.source.imports {
            if let morph_parser::MxImportKind::CssUrl { url } = &imp.kind {
                // Remote stylesheets are fetched once into .morph/css-cache
                // (fonts alongside them, URLs rewritten local); a failed fetch
                // warns and continues without it, like a browser offline.
                match css_fetcher.fetch_with_fonts(url) {
                    Some(text) if !text.is_empty() => {
                        if let Ok(data) = morph_parser::parse_css(&text) {
                            css_rules.extend(data.rules);
                            for (k, v) in data.keyframes {
                                css_keyframes.entry(k).or_default().extend(v);
                            }
                            remote_css.push((url.clone(), text));
                        }
                    }
                    _ => {
                        crate::logger::log_dim(&format!("Remote CSS unavailable, skipping: {url}"))
                    }
                }
            }
            if let morph_parser::MxImportKind::CssLocal { path } = &imp.kind {
                let candidates = [resolved.dir.join(path), cwd.join(path)];
                for cand in &candidates {
                    if cand.exists() {
                        if let Ok(text) = std::fs::read_to_string(cand) {
                            if let Ok(data) = morph_parser::parse_css(&text) {
                                css_rules.extend(data.rules);
                                for (k, v) in data.keyframes {
                                    css_keyframes.entry(k).or_default().extend(v);
                                }
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    let builder = morph_ir::IRBuilder::new().with_type_mode(type_mode);
    let windows =
        builder.build_with_graph(&graph, &css_rules, &css_keyframes).inspect_err(|_e| {
            pb.finish_and_clear();
        })?;
    pb.finish_and_clear();
    crate::logger::log_success(&format!("IR built — {} window(s)", windows.len()));

    // ── Generate C++ ──
    let output_dir = cwd.join(&output_raw);
    // Ensure output is treated as directory (clean name handles file case)
    let output_dir = if output_raw.ends_with('/') || Path::new(&output_raw).extension().is_none() {
        output_dir
    } else {
        output_dir.parent().map(Path::to_path_buf).unwrap_or(output_dir)
    };
    let pb = crate::logger::spinner("Generating C++...");
    // Route manifest: index every route.mx under the entry's source root
    // (folder path → RID). Independent of the import graph — routes are
    // resolved by id, never imported.
    let src_root =
        entry_path.parent().map(std::path::Path::to_path_buf).unwrap_or_else(|| cwd.clone());
    let routes = morph_parser::routes::scan_routes(&src_root);
    if !routes.is_empty() {
        crate::logger::log_key("Routes", &routes.len().to_string());
    }
    // Route files aren't in the import graph — lint them explicitly.
    {
        let mut lint_errors = 0;
        for route in &routes {
            let content = match std::fs::read_to_string(&route.file) {
                Ok(content) => content,
                Err(e) => anyhow::bail!("reading {}: {e}", route.file.display()),
            };
            for err in morph_parser::linter::check(&content, &route.file.display().to_string()) {
                if err.severity == "error" {
                    lint_errors += 1;
                    crate::logger::log_error(&format!(
                        "{}: {}: {} ({}:{})",
                        err.code,
                        err.message,
                        route.file.display(),
                        err.line,
                        err.col
                    ));
                    if let Some(hint) = &err.suggestion {
                        crate::logger::log_dim(&format!("  hint: {hint}"));
                    }
                    if err.code.starts_with("mx-") {
                        println!(
                            "  {}  {}",
                            "Learn more:".dimmed(),
                            morph_parser::docs_url(&err.code)
                        );
                    }
                }
            }
        }
        if lint_errors > 0 {
            anyhow::bail!(
                "build blocked by {lint_errors} lint error(s) — run `morph check` for details"
            );
        }
    }
    // Per-route IR for mount emission: one module graph rooted at each
    // route.mx (shared components compile into every mount that uses
    // them — same rule as the entry build, no cross-route sharing).
    let mut routes_ir: Vec<(morph_parser::routes::RouteEntry, morph_ir::IRWindow)> = Vec::new();
    for route in &routes {
        let route_graph = morph_parser::resolve_graph(&route.file, &cwd).inspect_err(|_e| {
            pb.finish_and_clear();
        })?;
        let route_win = builder
            .build_route(&route_graph, route, &css_rules, &css_keyframes)
            .inspect_err(|_e| {
                pb.finish_and_clear();
            })?;
        routes_ir.push((route.clone(), route_win));
    }
    let emitter = morph_codegen::CppEmitter::new(&windows)
        .with_routes(&routes)
        .with_routes_ir(&routes_ir)
        .with_app_window(config.window.title.clone(), config.window.width, config.window.height)
        .with_page_cache(
            config
                .navigation
                .cache
                .capacity()
                .map_err(|e| anyhow::anyhow!("morph.config.json: {e}"))?,
        )
        .with_window_ownership(
            config.window.parent.clone(),
            config.window.modal,
            morph_config::WindowRole::parse(&config.window.role)
                .map_err(|e| anyhow::anyhow!("morph.config.json [window]: {e}"))?
                .as_int() as i64,
        );
    emitter.emit(&output_dir)?;
    write_routes_dts(&cwd, &routes);
    pb.finish_and_clear();
    crate::logger::log_success(&format!("C++ generated → {}", output_dir.display()));

    // ── Translate companion TypeScript files into linkable fragments ──
    // Graph-referenced .ts plus src/**/*.ts, each translated in the
    // configured type mode and compiled+linked below. Failures here are
    // hard errors: silently dropping app logic would miscompile the app.
    let fragment_inputs = collect_typescript_sources(&cwd, &graph)?;
    let mut extra_sources: Vec<PathBuf> = Vec::new();
    if !fragment_inputs.is_empty() {
        let pb = crate::logger::spinner("Translating TypeScript...");
        let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (ts_path, ts_source) in &fragment_inputs {
            let filename = ts_path.file_name().and_then(|n| n.to_str()).unwrap_or("file.ts");
            let mut options = morpher::TranslateOptions { type_mode, ..Default::default() };
            let code = morpher::translate_fragment(ts_source, filename, options)
                .map_err(|e| anyhow::anyhow!("translating {}: {}", ts_path.display(), e))?;
            let stem = ts_path.file_stem().and_then(|s| s.to_str()).unwrap_or("fragment");
            let mut out_name = format!("{stem}.ts.cpp");
            let mut counter = 2;
            while !used_names.insert(out_name.clone()) {
                out_name = format!("{stem}_{counter}.ts.cpp");
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
    // Compiler override: config build.cxx, then MORPH_CXX, then platform default.
    let cxx_override = if config.build.cxx.is_empty() {
        std::env::var("MORPH_CXX").ok().filter(|v| !v.is_empty())
    } else {
        Some(config.build.cxx.clone())
    };
    // Find runtime dir (for headers)
    let runtime_dir = morph_build::find_runtime_dir(&cwd);
    let compiler = morph_build::Compiler::new(cxx_override.clone()).silent();
    let compiler_name = cxx_override.unwrap_or_else(morph_build::detect_compiler);
    let binary_path = output_dir.join(format!("{}{}", clean_name, morph_build::exe_suffix()));

    // A build is "fresh" (cargo-style) only when every input fingerprint is
    // unchanged AND the binary already exists. On any change we rebuild.
    // ── Fingerprinting: include all transitive modules + CSS + TS ──
    let config_text = std::fs::read_to_string(&config_path).unwrap_or_default();
    let runtime_hash = morph_cache::hash_tree(&runtime_dir);
    let mut owned_inputs: Vec<(String, String)> = vec![
        ("morph.config.json".to_string(), config_text),
        ("entry".to_string(), source),
        ("runtime".to_string(), runtime_hash),
        ("static".to_string(), static_.to_string()),
    ];
    // All transitive modules in the component graph.
    for module_path in graph.all_paths() {
        let text = std::fs::read_to_string(module_path).unwrap_or_default();
        owned_inputs.push((module_path.display().to_string(), text));
    }
    // Local CSS / C++ imports referenced from any graph module.
    for mod_path in graph.all_paths() {
        let Some(resolved) = graph.get(mod_path) else {
            continue;
        };
        for imp in &resolved.source.imports {
            // Module sources are already fingerprinted above; skip to avoid dupes.
            if imp.kind.is_module_source() {
                continue;
            }
            let path = match &imp.kind {
                morph_parser::MxImportKind::CssLocal { path }
                | morph_parser::MxImportKind::Component { path, .. }
                | morph_parser::MxImportKind::CppLocal { path, .. } => path,
                morph_parser::MxImportKind::CssUrl { .. } => continue,
            };
            let candidates = [resolved.dir.join(path), cwd.join(path)];
            let text = candidates
                .iter()
                .find(|c| c.exists())
                .map_or_else(String::new, |cand| std::fs::read_to_string(cand).unwrap_or_default());
            owned_inputs.push((path.clone(), text));
        }
    }
    for (ts_path, ts_source) in &fragment_inputs {
        owned_inputs.push((format!("ts:{}", ts_path.display()), ts_source.clone()));
    }
    for (url, text) in &remote_css {
        owned_inputs.push((format!("css:{url}"), text.clone()));
    }
    let fingerprint_inputs: Vec<(&str, &str)> =
        owned_inputs.iter().map(|(p, c)| (p.as_str(), c.as_str())).collect();
    let fingerprint = morph_cache::fingerprint_inputs(&fingerprint_inputs);
    let stored = morph_cache::read_stored_fingerprint(&cwd, &clean_name);
    if stored.as_deref() == Some(fingerprint.as_str()) && binary_path.exists() {
        crate::logger::log_success(&format!(
            "Up to date — nothing to compile ({})",
            binary_path.display()
        ));
        println!();
        return Ok(binary_path);
    }

    let pb = crate::logger::spinner(&format!("Compiling with {compiler_name}..."));
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
        crate::logger::log_error(&format!("Compile failed: {e}"));
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
                crate::logger::log_success(&format!("UPX compressed → {}", binary_path.display()));
            }
        } else {
            crate::logger::log_dim("UPX not available, skipping compression");
        }
    }
    println!();

    Ok(binary_path)
}

/// Typed routes for editors: union of every manifest route id, written to
/// the project root (picked up by any `*.d.ts`-aware tooling). Explicit
/// window ids join `MorphWindowId` once `useWindow` ids exist; until then
/// it stays `string`. Regenerated every build — never hand-edit.
fn write_routes_dts(cwd: &Path, routes: &[morph_parser::routes::RouteEntry]) {
    let mut out = String::from("// Generated by Morph — route manifest. Do not edit.\n");
    if routes.is_empty() {
        out.push_str("export type MorphRoute = never;\n");
    } else {
        out.push_str("export type MorphRoute =\n");
        for (i, route) in routes.iter().enumerate() {
            let sep = if i + 1 == routes.len() { ";" } else { " |" };
            out.push_str(&format!("    | \"{}\"{sep}\n", route.id));
        }
    }
    out.push_str("export type MorphWindowId = string;\n");
    if let Err(e) = std::fs::write(cwd.join("morph-routes.d.ts"), out) {
        crate::logger::log_dim(&format!("Could not write morph-routes.d.ts: {e}"));
    }
}

/// Collect companion TypeScript sources: `.ts` Component imports from every
/// module in the graph plus every `*.ts`/`*.tsx` under the entry directory
/// (excluding the entry itself, ambient `*.d.ts` declarations, generated
/// output, and tooling dirs). Returns canonical paths with contents in
/// stable order. Anything unreadable or unlisted-but-required is a hard
/// error: silently dropping app logic would miscompile the app.
fn collect_typescript_sources(
    cwd: &Path,
    graph: &morph_parser::ModuleGraph,
) -> Result<Vec<(PathBuf, String)>> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    let mut push_candidate = |path: PathBuf| {
        let canonical = path.canonicalize().unwrap_or(path);
        if seen.insert(canonical.clone()) {
            found.push(canonical);
        }
    };
    let entry_parent = graph.entry.parent().map_or_else(|| cwd.to_path_buf(), Path::to_path_buf);
    for mod_path in graph.all_paths() {
        let Some(resolved) = graph.get(mod_path) else {
            continue;
        };
        for imp in &resolved.source.imports {
            if !(imp.kind.is_ts() || imp.kind.is_tsx()) {
                continue;
            }
            let Some(path) = imp.kind.path() else {
                continue;
            };
            let mut located = false;
            for base in [&resolved.dir, cwd] {
                let candidate = base.join(path);
                if candidate.is_file() {
                    push_candidate(candidate);
                    located = true;
                    break;
                }
            }
            if !located {
                anyhow::bail!("TypeScript import not found: {path}");
            }
        }
    }
    let src_dir = cwd.join("src");
    for search_root in [entry_parent.as_path(), src_dir.as_path()] {
        if !search_root.is_dir() {
            continue;
        }
        for entry in
            walkdir::WalkDir::new(search_root).into_iter().filter_map(std::result::Result::ok)
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let ext_is = |want: &str| {
                Path::new(name).extension().is_some_and(|ext| ext.eq_ignore_ascii_case(want))
            };
            let is_typescript = (ext_is("ts") && !ext_is("tsx")) || ext_is("tsx");
            if !is_typescript || name.to_ascii_lowercase().ends_with(".d.ts") {
                continue;
            }
            if name.to_ascii_lowercase().ends_with(".ts.cpp") {
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
    // Graph members are owned by the builder (namespaced definitions in
    // app.cpp + header declarations, imports resolved): compiling them
    // again as standalone fragments would duplicate broken code (fragments
    // never resolved imports). Only unimported files keep fragments.
    let graph_members: std::collections::HashSet<&PathBuf> = graph.all_paths().collect();
    let mut sources = Vec::new();
    for path in found {
        if path == graph.entry || graph_members.contains(&path) {
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

    fn test_graph(root: &Path, entry: &Path) -> morph_parser::ModuleGraph {
        morph_parser::resolve_graph(entry, root).unwrap()
    }

    #[test]
    fn collects_companion_typescript_sources() {
        let root = scratch_project("collect");
        std::fs::write(root.join("src/App.mx"), "<body/>").unwrap();
        std::fs::write(root.join("src/util.ts"), "export const x = 1;\n").unwrap();
        std::fs::write(root.join("src/widget.tsx"), "export const y = 2;\n").unwrap();
        std::fs::write(root.join("src/env.d.ts"), "declare const y: number;\n").unwrap();
        let entry = root.join("src/App.mx");
        let graph = test_graph(&root, &entry);
        let found = collect_typescript_sources(&root, &graph).unwrap();
        let names: Vec<String> = found
            .iter()
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["util.ts".to_string(), "widget.tsx".to_string()]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_typescript_import_is_hard_error() {
        let root = scratch_project("missing");
        let entry = root.join("src/App.mx");
        std::fs::write(
            &entry,
            "import Missing from './missing.ts'\nexport default function App() { return <div/> }",
        )
        .unwrap();
        let err = morph_parser::resolve_graph(&entry, &root).unwrap_err();
        assert!(err.to_string().contains("missing.ts"));
        let _ = std::fs::remove_dir_all(&root);
    }
}
