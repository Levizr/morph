/// Platform helpers — mirrors Python's `morph/build/platform.py`
/// Works on Linux, macOS, Windows.
pub const fn current() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

pub fn is_macos() -> bool {
    current() == "macos"
}
pub fn is_windows() -> bool {
    current() == "windows"
}
pub fn is_linux() -> bool {
    current() == "linux"
}

pub fn exe_suffix() -> &'static str {
    if is_windows() {
        ".exe"
    } else {
        ""
    }
}

pub fn shared_lib_ext() -> &'static str {
    if is_windows() {
        ".dll"
    } else if is_macos() {
        ".dylib"
    } else {
        ".so"
    }
}

pub fn shared_lib_flag() -> &'static str {
    if is_macos() {
        "-dynamiclib"
    } else {
        "-shared"
    }
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
        if which(c) {
            return c.to_string();
        }
    }
    "g++".to_string()
}

fn which(bin: &str) -> bool {
    // Use `which` crate if available, else fallback to `which` command
    // For now, use `which` command via `which` crate's logic — try PATH
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(bin);
            if candidate.exists() {
                return true;
            }
            // On Windows, also check .exe
            if is_windows() {
                let exe = dir.join(format!("{bin}.exe"));
                if exe.exists() {
                    return true;
                }
            }
        }
    }
    false
}

/// Dev IPC address for display before the runtime announces its port.
///
/// The dev runtime binds the preferred loopback port and falls back to an
/// OS-assigned ephemeral port on collision, announcing the bound port on
/// stdout (`[morph] dev socket on 127.0.0.1:PORT`); the driver always
/// connects to the announced port.
pub fn dev_ipc_addr() -> String {
    format!("127.0.0.1:{}", crate::ipc::DEV_PORT_DEFAULT)
}
