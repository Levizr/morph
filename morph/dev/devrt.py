import os
import subprocess
import platform


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


def build() -> bool:
    """Build morph_devrt via CMake. Returns True on success."""
    dev_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../runtime/dev"))
    build_dir = os.path.join(dev_dir, "build")

    # Configure
    if subprocess.run(["cmake", "-S", dev_dir, "-B", build_dir],
                      capture_output=True).returncode != 0:
        return False

    # Build
    if subprocess.run(["cmake", "--build", build_dir],
                      capture_output=True).returncode != 0:
        return False

    return os.path.exists(get_devrt_path())


def ensure_built() -> bool:
    path = get_devrt_path()
    if os.path.exists(path):
        return True
    return build()


def launch() -> subprocess.Popen:
    path = get_devrt_path()
    if not os.path.exists(path):
        if not build():
            raise FileNotFoundError(f"morph_devrt not found at {path} and build failed")
    return subprocess.Popen([path])
