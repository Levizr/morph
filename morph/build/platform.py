"""Platform helpers for morph's native build (Linux, macOS, Windows)."""

from __future__ import annotations

import shutil
import sys


def current() -> str:
    if sys.platform == "darwin":
        return "macos"
    if sys.platform == "win32":
        return "windows"
    return "linux"


def is_macos() -> bool:
    return current() == "macos"


def is_windows() -> bool:
    return current() == "windows"


def is_linux() -> bool:
    return current() == "linux"


def exe_suffix() -> str:
    return ".exe" if is_windows() else ""


def shared_lib_ext() -> str:
    """Native shared-library extension for the current OS."""
    if is_windows():
        return ".dll"
    if is_macos():
        return ".dylib"
    return ".so"


def shared_lib_flag() -> str:
    """Linker flag that produces a shared library on this OS."""
    return "-dynamiclib" if is_macos() else "-shared"


def pick_cpp() -> str:
    """Pick the best available C++ compiler for this OS."""
    if is_windows():
        for c in ("x86_64-w64-mingw32-g++", "g++", "clang++"):
            p = shutil.which(c)
            if p:
                return p
    elif is_macos():
        for c in ("clang++", "g++"):
            p = shutil.which(c)
            if p:
                return p
    for c in ("g++-14", "g++", "clang++"):
        p = shutil.which(c)
        if p:
            return p
    return "g++"
