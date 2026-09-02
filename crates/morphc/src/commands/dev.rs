use anyhow::Result;
use colored::Colorize;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::Path;
use std::sync::mpsc::channel;
use std::time::Duration;

pub fn run(entry: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry_file = entry.as_deref().unwrap_or(&config.entry).to_string();

    crate::logger::log_banner(&format!("Morph Dev — {}", config.name));

    crate::logger::log_step("Verifying runtime");
    crate::commands::install::ensure_runtime(&cwd)?;
    crate::logger::log_success(&format!(
        "Runtime {} v{}",
        config.runtime.runtime_type.cyan(),
        config.runtime.version.dimmed()
    ));

    crate::logger::log_step("Configuration");
    crate::logger::log_key("Entry", &entry_file);
    crate::logger::log_key("Runtime", &format!("{} v{}", config.runtime.runtime_type, config.runtime.version));
    // Cross-platform IPC
    let ipc_addr = morph_build::platform::dev_ipc_addr(&cwd);
    if morph_build::platform::is_windows() {
        crate::logger::log_key("IPC", &format!("TCP {}", ipc_addr));
    } else {
        crate::logger::log_key("IPC", &format!("Unix {}", ipc_addr));
    }

    // Initial build
    // Morph only supports strict .ts/.tsx/.mx entries — hard error otherwise.
    morph_config::validate_entry_ext(&cwd.join(&entry_file))
        .map_err(|msg| anyhow::anyhow!(msg))?;

    crate::logger::log_step("Initial build");
    if let Err(e) = do_build(&cwd, &entry_file) {
        crate::logger::log_error(&format!("Initial build failed: {}", e));
    } else {
        crate::logger::log_success("Initial build OK");
    }

    // Watch src/ for changes
    let watch_path = cwd.join("src");
    if !watch_path.exists() {
        crate::logger::log_warn("No src/ directory to watch");
        return Ok(());
    }

    crate::logger::log_step(&format!("Watching {} for changes (Ctrl+C to stop)", watch_path.display()));
    // Setup watcher
    let (tx, rx) = channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(ev) = res {
            // Only care about modify/create
            match ev.kind {
                EventKind::Modify(_) | EventKind::Create(_) => { let _ = tx.send(()); }
                _ => {}
            }
        }
    })?;
    watcher.watch(&watch_path, RecursiveMode::Recursive)?;

    // Also watch the entry file's directory
    let entry_path = cwd.join(&entry_file);
    if let Some(parent) = entry_path.parent() {
        if parent != watch_path {
            let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
        }
    }

    // Dev server loop — on change, rebuild
    loop {
        // Wait for event with debounce
        if rx.recv_timeout(Duration::from_millis(500)).is_ok() {
            // Debounce: drain extra events
            while rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(100));
            crate::logger::log_step("Change detected — rebuilding");
            match do_build(&cwd, &entry_file) {
                Ok(_) => crate::logger::log_success("Hot reload OK"),
                Err(e) => crate::logger::log_error(&format!("Build failed: {}", e)),
            }
        }
        // Also check for Ctrl+C via polling? notify will keep running until process killed
    }
}

fn do_build(cwd: &Path, entry: &str) -> Result<()> {
    let entry_path = cwd.join(entry);
    let source = std::fs::read_to_string(&entry_path)?;
    let parsed = morph_parser::parse_mx_str(&source, entry)?;
    let mut css = morph_parser::CssData::default();
    for imp in &parsed.imports {
        if let morph_parser::MxImportKind::CssLocal { path } = &imp.kind {
            let cand = entry_path.parent().map(|p| p.join(path)).unwrap_or_else(|| cwd.join(path));
            if cand.exists() {
                if let Ok(text) = std::fs::read_to_string(&cand) {
                    if let Ok(data) = morph_parser::parse_css(&text) {
                        css.rules.extend(data.rules);
                        for (k, v) in data.keyframes { css.keyframes.entry(k).or_default().extend(v); }
                    }
                }
            }
        }
    }
    let mut builder = morph_ir::IRBuilder::new();
    let windows = builder.build(&parsed, &css.rules, &css.keyframes);
    // For dev, we just verify IR builds; in full dev we'd emit logic and push via IPC
    crate::logger::log_dim(&format!("Parsed {} ({} windows, {} state vars)", entry, windows.len(), windows.iter().map(|w| w.state_vars.len()).sum::<usize>()));
    Ok(())
}
