import hashlib
import os
import platform
import subprocess

from morph.utils.logger import log_info, log_error, log_success, log_warn


def _get_devrt_dir() -> str:
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "bin"))


def get_devrt_path() -> str:
    base = _get_devrt_dir()
    system = platform.system()
    match system:
        case "Linux":   return os.path.join(base, "morph_devrt")
        case "Darwin":  return os.path.join(base, "morph_devrt_macos")
        case "Windows": return os.path.join(base, "morph_devrt.exe")
        case _:         raise RuntimeError(f"Unsupported platform: {system}")


def _get_dev_src_dir() -> str:
    return os.path.abspath(os.path.join(os.path.dirname(__file__), "../runtime/dev"))


def _hash_file(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def _compute_source_hash() -> str:
    dev_dir = _get_dev_src_dir()
    runtime_dir = os.path.abspath(os.path.join(dev_dir, ".."))
    paths = []

    # Scan dev, core, render, ui, style, vendor directories for source changes
    for sub in ["dev", "core", "render", "ui", "style", "vendor"]:
        scan = os.path.join(runtime_dir, sub)
        if not os.path.isdir(scan):
            continue
        for root, _dirs, files in os.walk(scan):
            for f in files:
                if f.endswith((".cpp", ".h", ".hpp")) or f == "CMakeLists.txt":
                    paths.append(os.path.join(root, f))

    paths.sort()
    h = hashlib.sha256()
    for p in paths:
        rel = os.path.relpath(p, dev_dir)
        h.update(rel.encode())
        h.update(_hash_file(p).encode())
    return h.hexdigest()


def _hash_stored_path() -> str:
    return os.path.join(_get_devrt_dir(), ".devrt_source_hash")


def _stored_hash() -> str | None:
    p = _hash_stored_path()
    try:
        with open(p) as f:
            return f.read().strip()
    except (FileNotFoundError, OSError):
        return None


def _save_hash(h: str) -> None:
    p = _hash_stored_path()
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w") as f:
        f.write(h + "\n")


def _source_changed() -> bool:
    cur = _compute_source_hash()
    prev = _stored_hash()
    return cur != prev


def ensure_built() -> bool:
    path = get_devrt_path()
    binary_exists = os.path.exists(path)
    changed = _source_changed()

    if binary_exists and not changed:
        return True

    if binary_exists and changed:
        log_info("morph_devrt source changed — rebuilding...")

    if not binary_exists:
        log_info("morph_devrt not found — building from source...")

    dev_dir = _get_dev_src_dir()
    build_dir = os.path.join(dev_dir, "build")

    log_info("Configuring CMake...")
    subprocess.run(["rm", "-rf", build_dir], check=True)
    r = subprocess.run(["cmake", "-S", dev_dir, "-B", build_dir])
    if r.returncode != 0:
        log_error("CMake configuration failed")
        log_info("Ensure cmake is installed: sudo apt install cmake")
        return False

    log_info("Compiling morph_devrt (this may take a minute)...")
    r = subprocess.run(["cmake", "--build", build_dir])
    if r.returncode != 0:
        log_error("Compilation failed — check output above for errors")
        return False

    if os.path.exists(path):
        _save_hash(_compute_source_hash())
        log_success(f"morph_devrt built at {path}")
        return True

    log_error(f"Build completed but binary not found at {path}")
    return False


def launch() -> subprocess.Popen:
    path = get_devrt_path()

    if not ensure_built():
        log_error("Failed to build morph_devrt. Make sure cmake and g++ are installed:")
        log_info("  sudo apt install cmake g++ libglfw3-dev")
        raise FileNotFoundError(
            f"morph_devrt binary missing at {path} and auto-build failed. "
            "Run `morph doctor` for diagnostics."
        )

    return subprocess.Popen([path], stderr=subprocess.PIPE, text=True)
