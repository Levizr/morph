import os
import subprocess
import platform


def get_devrt_path() -> str:
    base = os.path.join(os.path.dirname(__file__), "..", "bin")
    system = platform.system()
    match system:
        case "Linux":   return os.path.abspath(os.path.join(base, "morph_devrt"))
        case "Darwin":  return os.path.abspath(os.path.join(base, "morph_devrt_macos"))
        case "Windows": return os.path.abspath(os.path.join(base, "morph_devrt.exe"))
        case _:         raise RuntimeError(f"Unsupported platform: {system}")


def launch() -> subprocess.Popen:
    path = get_devrt_path()
    if not os.path.exists(path):
        raise FileNotFoundError(f"morph_devrt not found at {path}")
    return subprocess.Popen([path])
