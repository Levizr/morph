/// Platform helpers — mirrors Python's `morph/build/platform.py`
/// Works on Linux, macOS, Windows.

pub fn current() -> &'static str {
    if cfg!(target_os = "macos") { "macos" }
    else if cfg!(target_os = "windows") { "windows" }
    else { "linux" }
}

pub fn is_macos() -> bool { current() == "macos" }
pub fn is_windows() -> bool { current() == "windows" }
pub fn is_linux() -> bool { current() == "linux" }

pub fn exe_suffix() -> &'static str {
    if is_windows() { ".exe" } else { "" }
}

pub fn shared_lib_ext() -> &'static str {
    if is_windows() { ".dll" } else if is_macos() { ".dylib" } else { ".so" }
}

pub fn shared_lib_flag() -> &'static str {
    if is_macos() { "-dynamiclib" } else { "-shared" }
}

pub fn pick_cpp() -> String {
    // On Windows, prefer mingw g++, then g++, then clang++
    // On macOS, prefer clang++, then g++
    // On Linux, prefer g++-14, g++, clang++
    let candidates: &[&str] = if is_windows() {
        &["x86_64-w64-mingw32-g++", "g++", "clang++"]
    } else if is_macos() {
        &["clang++", "g++"]
    } else {
        &["g++-14", "g++", "clang++"]
    };
    for c in candidates {
        if which(c) { return c.to_string(); }
    }
    "g++".to_string()
}

fn which(bin: &str) -> bool {
    // Use `which` crate if available, else fallback to `which` command
    // For now, use `which` command via `which` crate's logic — try PATH
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(bin);
            if candidate.exists() { return true; }
            // On Windows, also check .exe
            if is_windows() {
                let exe = dir.join(format!("{}.exe", bin));
                if exe.exists() { return true; }
            }
        }
    }
    false
}

/// IPC socket path — Unix socket on Unix, TCP on Windows
/// Returns (socket_path_for_unix, tcp_addr_for_windows)
pub fn dev_ipc_addr(project_dir: &std::path::Path) -> String {
    if is_windows() {
        // Windows: no Unix socket, use TCP
        "127.0.0.1:3000".to_string()
    } else {
        // Unix: use .morph/dev.sock
        project_dir.join(".morph").join("dev.sock").display().to_string()
    }
}
