import platform
import sys


def system() -> str:
    return platform.system()   # "Linux" | "Darwin" | "Windows"


def is_linux()   -> bool: return system() == "Linux"
def is_macos()   -> bool: return system() == "Darwin"
def is_windows() -> bool: return system() == "Windows"


def python_version() -> str:
    return f"{sys.version_info.major}.{sys.version_info.minor}"
