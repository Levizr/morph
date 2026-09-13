use anyhow::{Context, Result};
use colored::Colorize;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

/// Watched source extensions (mirrors `morph/dev/watcher.py`).
const WATCH_EXTS: &[&str] = &["mx", "html", "css", "js"];
/// Trailing-edge debounce: fire once the editor goes quiet this long.
const DEBOUNCE: Duration = Duration::from_millis(300);
/// Settle wait: file content hash stable across polls (mirrors `_wait_settle`).
const SETTLE_TIMEOUT: Duration = Duration::from_secs(2);
const SETTLE_POLL: Duration = Duration::from_millis(50);
/// How long to wait for the runtime's socket announcement / connection.
const IPC_TIMEOUT: Duration = Duration::from_secs(30);

pub fn run(entry: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let config_path = cwd.join("morph.config.json");

    if !config_path.exists() {
        anyhow::bail!("morph.config.json not found. Run `morph new` first.");
    }

    let config = morph_config::MorphConfig::from_file(&config_path)?;
    let entry_file = entry.unwrap_or_else(|| config.entry.clone());

    crate::logger::log_banner(&format!("Morph Dev — {}", config.name));

    crate::logger::log_step("Verifying runtime");
    crate::commands::install::ensure_runtime(&cwd)?;
    crate::logger::log_success(&format!(
        "Runtime {} v{}",
        config.runtime.runtime_type.cyan(),
        config.runtime.version.dimmed()
    ));

    // Morph only supports strict .ts/.tsx/.mx entries — hard error otherwise.
    morph_config::validate_entry_ext(&cwd.join(&entry_file)).map_err(|msg| anyhow::anyhow!(msg))?;

    crate::logger::log_step("Configuration");
    crate::logger::log_key("Entry", &entry_file);
    crate::logger::log_key(
        "Runtime",
        &format!("{} v{}", config.runtime.runtime_type, config.runtime.version),
    );
    crate::logger::log_key("IPC", &format!("TCP {}", morph_build::platform::dev_ipc_addr()));

    let type_mode: morpher::TypeMode =
        config.type_mode.parse().map_err(|e: String| anyhow::anyhow!(e))?;
    let runtime_dir = morph_build::find_runtime_dir(&cwd);
    let dev_dir = runtime_dir.join("dev");
    let binary = morph_build::devrt::binary_path(&dev_dir);

    crate::logger::log_step("Building dev runtime");
    let cmake_bin = morph_build::devrt::resolve_cmake(&config.build.cmake);
    let started = Instant::now();
    match morph_build::devrt::ensure_built(&cmake_bin, &dev_dir, &runtime_dir, &binary) {
        Ok(path) => crate::logger::log_success(&format!(
            "morph_devrt ready in {:.1}s ({})",
            started.elapsed().as_secs_f32(),
            path.display()
        )),
        Err(e) => {
            crate::logger::log_error(&format!("Dev runtime build failed: {e:#}"));
            crate::logger::log_info("Ensure cmake and a C++20 compiler are installed");
            return Err(e);
        }
    }

    let mut cmd = Session {
        cwd: cwd.clone(),
        entry: entry_file.clone(),
        type_mode,
        runtime_dir,
        cache_dir: cwd.join(".morph").join("cache"),
        compiler: dev_compiler(&config),
        native: morph_build::NativeFlags {
            include_dirs: config.native.include_dirs.clone(),
            library_dirs: config.native.library_dirs.clone(),
            libraries: config.native.libraries.clone(),
            cflags: config.native.cflags.clone(),
            ldflags: config.native.ldflags.clone(),
        },
        logic: morph_build::logic::LogicSession::default(),
        file_hashes: HashMap::new(),
    };

    crate::logger::log_step("Launching dev runtime");
    let (mut child, mut client) = launch_and_connect(&binary, &cwd)?;

    reload(&mut cmd, &mut client, Path::new(""));

    crate::logger::log_dim("Ready — watching for changes (Ctrl+C to stop)");
    let watch_dir = cwd.join(&entry_file).parent().map_or_else(|| cwd.clone(), Path::to_path_buf);
    crate::logger::log_key("Watching", &watch_dir.display().to_string());
    let (watch_tx, watch_rx) = channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, _>| {
        if let Ok(event) = res {
            if !is_relevant(&event) {
                return;
            }
            for path in event.paths {
                let _ = watch_tx.send(path);
            }
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::Recursive)?;

    watch_loop(&mut child, &mut cmd, &mut client, &watch_rx)?;
    kill_child(&mut child);
    Ok(())
}

/// Spawn the dev runtime, wait for its socket announcement and connect.
fn launch_and_connect(
    binary: &Path,
    cwd: &Path,
) -> Result<(std::process::Child, morph_build::ipc::IpcClient)> {
    let mut child = morph_build::devrt::launch(binary, cwd)?;
    let (port_tx, port_rx) = channel::<u16>();
    pump_stream(child.stdout.take(), false, port_tx.clone(), "devrt");
    pump_stream(child.stderr.take(), true, port_tx, "devrt");

    crate::logger::log_step("Connecting to IPC");
    let port = wait_for_port(&port_rx, &mut child)?;
    crate::logger::log_key("IPC", &format!("TCP 127.0.0.1:{port}"));
    match morph_build::ipc::IpcClient::connect(port, IPC_TIMEOUT) {
        Ok(client) => {
            crate::logger::log_success("IPC connected");
            Ok((child, client))
        }
        Err(e) => {
            crate::logger::log_error(&format!("IPC connect failed: {e:#}"));
            kill_child(&mut child);
            Err(e)
        }
    }
}

/// Watch loop: debounce change events into reloads until the window closes.
fn watch_loop(
    child: &mut std::process::Child,
    cmd: &mut Session,
    client: &mut morph_build::ipc::IpcClient,
    watch_rx: &Receiver<PathBuf>,
) -> Result<()> {
    loop {
        if child_exited(child)? {
            crate::logger::log_success("Window closed — dev mode stopping");
            break;
        }
        let changed = match watch_rx.recv_timeout(Duration::from_millis(300)) {
            Ok(path) => drain_debounced(watch_rx, child, path)?,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };
        let Some(changed) = changed else { continue };
        if child_exited(child)? {
            crate::logger::log_success("Window closed — dev mode stopping");
            break;
        }
        reload(cmd, client, &changed);
    }
    Ok(())
}

/// Per-session reload state.
struct Session {
    cwd: PathBuf,
    entry: String,
    type_mode: morpher::TypeMode,
    runtime_dir: PathBuf,
    cache_dir: PathBuf,
    compiler: morph_build::Compiler,
    native: morph_build::NativeFlags,
    logic: morph_build::logic::LogicSession,
    file_hashes: HashMap<PathBuf, String>,
}

/// Hot-reload logic compiler: `build.dev_cxx` → `MORPH_DEV_CXX` → default.
fn dev_compiler(config: &morph_config::MorphConfig) -> morph_build::Compiler {
    let configured = if config.build.dev_cxx.is_empty() {
        std::env::var("MORPH_DEV_CXX").ok().filter(|v| !v.is_empty())
    } else {
        Some(config.build.dev_cxx.clone())
    };
    let name = configured.clone().unwrap_or_else(morph_build::detect_compiler);
    crate::logger::log_key("Logic compiler", &name);
    morph_build::Compiler::new(configured).silent()
}

/// Drain follow-up events and wait out the trailing-edge debounce window.
/// Returns the last changed path, or `None` when the child exited meanwhile.
fn drain_debounced(
    rx: &Receiver<PathBuf>,
    child: &mut std::process::Child,
    first: PathBuf,
) -> Result<Option<PathBuf>> {
    let mut last = first;
    loop {
        if child_exited(child)? {
            return Ok(None);
        }
        match rx.recv_timeout(DEBOUNCE) {
            Ok(path) => last = path,
            Err(_) => return Ok(Some(last)),
        }
    }
}

fn is_relevant(event: &Event) -> bool {
    if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
        return false;
    }
    event
        .paths
        .iter()
        .any(|p| p.extension().and_then(|e| e.to_str()).is_some_and(|e| WATCH_EXTS.contains(&e)))
}

/// Stream a child pipe to the console, forwarding the socket announcement.
fn pump_stream(
    pipe: Option<impl std::io::Read + Send + 'static>,
    is_stderr: bool,
    port_tx: Sender<u16>,
    _tag: &'static str,
) {
    let Some(pipe) = pipe else { return };
    std::thread::spawn(move || {
        let reader = BufReader::new(pipe);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(port) = morph_build::ipc::parse_socket_announce(&line) {
                let _ = port_tx.send(port);
            }
            if is_stderr {
                eprintln!("{line}");
            } else {
                println!("{line}");
            }
        }
    });
}

/// Wait for the runtime's socket announcement, bailing if it exits first.
fn wait_for_port(port_rx: &Receiver<u16>, child: &mut std::process::Child) -> Result<u16> {
    let deadline = Instant::now() + IPC_TIMEOUT;
    loop {
        if child_exited(child)? {
            anyhow::bail!("dev runtime exited before announcing its socket");
        }
        let remaining = deadline.checked_duration_since(Instant::now()).unwrap_or(Duration::ZERO);
        if remaining.is_zero() {
            anyhow::bail!("timed out waiting for the dev runtime socket announcement");
        }
        match port_rx.recv_timeout(remaining.min(Duration::from_millis(300))) {
            Ok(port) => return Ok(port),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("lost dev runtime output before socket announcement")
            }
        }
    }
}

fn child_exited(child: &mut std::process::Child) -> Result<bool> {
    Ok(child.try_wait()?.is_some())
}

fn kill_child(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Wait until a file's content stops changing (editors write in stages).
fn wait_settle(path: &Path) {
    let hash_of = || morph_cache::sha256_file(path).ok();
    let prev = hash_of();
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    while Instant::now() < deadline {
        std::thread::sleep(SETTLE_POLL);
        let cur = hash_of();
        if cur.is_some() && cur == prev {
            return;
        }
    }
}

/// Full reload: parse → CSS → IR → serialize → push. Errors are reported
/// to the terminal and the in-app DevTools panel; watching continues.
fn reload(cmd: &mut Session, client: &mut morph_build::ipc::IpcClient, changed: &Path) {
    if !changed.as_os_str().is_empty() {
        let rel = changed.strip_prefix(&cmd.cwd).unwrap_or(changed).display().to_string();
        wait_settle(changed);
        if let Ok(cur) = morph_cache::sha256_file(changed) {
            if cmd.file_hashes.get(changed).map(String::as_str) == Some(cur.as_str()) {
                return;
            }
            cmd.file_hashes.insert(changed.to_path_buf(), cur);
        }
        let _ = client.send_log("info", &format!("File changed: {rel}"));
        crate::logger::log_step(&format!("Change detected — recompiling {rel}"));
    }
    match build_windows(cmd) {
        Ok(windows) => match push_windows(cmd, client, &windows) {
            Ok(timing) => {
                if changed.as_os_str().is_empty() {
                    crate::logger::log_success(&format!("Rebuilt & reloaded in {timing}"));
                } else {
                    crate::logger::log_success(&format!("Hot reloaded in {timing}"));
                }
                let _ = client.send_log("ok", &format!("Hot reloaded in {timing}"));
            }
            Err(e) => {
                let message = format!("{e:#}");
                crate::logger::log_error(&message);
                let _ = client.send_error(&message);
            }
        },
        Err(e) => {
            let message = format!("{e:#}");
            crate::logger::log_error(&message);
            let _ = client.send_error(&message);
        }
    }
}

/// Emit + compile the logic library and push the IR document.
fn push_windows(
    cmd: &mut Session,
    client: &mut morph_build::ipc::IpcClient,
    windows: &[morph_ir::IRWindow],
) -> Result<String> {
    let started = Instant::now();
    let logic = morph_codegen::logic_emitter::emit_logic(windows);
    if let Some(header) = logic.state_header.as_deref() {
        std::fs::create_dir_all(&cmd.cache_dir)
            .with_context(|| format!("creating logic cache {}", cmd.cache_dir.display()))?;
        std::fs::write(cmd.cache_dir.join(morph_codegen::logic_emitter::STATE_HEADER_NAME), header)
            .with_context(|| "writing native state header")?;
    }
    let mut user_sources = Vec::new();
    for w in windows {
        for import in &w.cpp_imports {
            if let Some(path) = import.get("path") {
                let candidate = PathBuf::from(path);
                let resolved =
                    if candidate.is_absolute() { candidate } else { cmd.cwd.join(&candidate) };
                if resolved.exists() {
                    user_sources.push(resolved);
                }
            }
        }
    }
    let runtime_hash =
        morph_build::devrt::compute_source_hash(&cmd.runtime_dir.join("dev"), &cmd.runtime_dir)?;
    // User `.cpp` imports are entry-relative (`./native.cpp`); resolve them
    // for the logic TU via the entry dir (quoted includes search `-I` dirs).
    let mut native = cmd.native.clone();
    if let Some(dir) = cmd.cwd.join(&cmd.entry).parent() {
        native.include_dirs.push(dir.display().to_string());
    }
    let so_path = morph_build::logic::compile_logic(
        &mut cmd.logic,
        &cmd.compiler,
        &cmd.runtime_dir,
        &cmd.cache_dir,
        &logic.source,
        &user_sources,
        &runtime_hash,
        &native,
    )?;
    let json = morph_ir::IRSerializer::to_json(windows, Some(&so_path.display().to_string()))
        .context("serializing dev IR")?;
    client.send_ir(&json)?;
    Ok(format!("{:.1}s", started.elapsed().as_secs_f32()))
}

/// Parse the entry, resolve CSS and build the IR windows.
fn build_windows(cmd: &Session) -> Result<Vec<morph_ir::IRWindow>> {
    let entry_path = cmd.cwd.join(&cmd.entry);
    let source = std::fs::read_to_string(&entry_path)
        .with_context(|| format!("reading {}", entry_path.display()))?;
    let parsed = morph_parser::parse_mx_str(&source, &cmd.entry)?;
    let (css_rules, css_keyframes) = collect_css(&cmd.cwd, &entry_path, &parsed.imports);
    let builder = morph_ir::IRBuilder::new().with_type_mode(cmd.type_mode);
    let windows = builder.build(&parsed, &css_rules, &css_keyframes);
    if windows.is_empty() {
        anyhow::bail!("Build failed — no windows in {}", cmd.entry);
    }
    Ok(windows)
}

/// Collected stylesheets: ordered rules plus keyframes by animation name.
type CssBundle =
    (Vec<(String, morph_parser::CssRule)>, HashMap<String, Vec<morph_parser::CssKeyframe>>);

/// Collect local + remote stylesheets (mirrors the build command).
fn collect_css(cwd: &Path, entry_path: &Path, imports: &[morph_parser::MxImport]) -> CssBundle {
    let mut css_rules = Vec::new();
    let mut css_keyframes: HashMap<String, Vec<morph_parser::CssKeyframe>> = HashMap::new();
    let css_fetcher =
        morph_build::css_fetch::CssFetcher::new(cwd.join(".morph").join("css-cache")).silent();
    for imp in imports {
        match &imp.kind {
            morph_parser::MxImportKind::CssUrl { url } => match css_fetcher.fetch_with_fonts(url) {
                Some(text) if !text.is_empty() => {
                    if let Ok(data) = morph_parser::parse_css(&text) {
                        css_rules.extend(data.rules);
                        for (k, v) in data.keyframes {
                            css_keyframes.entry(k).or_default().extend(v);
                        }
                    }
                }
                _ => crate::logger::log_dim(&format!("Remote CSS unavailable, skipping: {url}")),
            },
            morph_parser::MxImportKind::CssLocal { path } => {
                let candidates = [
                    entry_path.parent().map_or_else(|| cwd.join(path), |p| p.join(path)),
                    cwd.join(path),
                ];
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
            _ => {}
        }
    }
    (css_rules, css_keyframes)
}
