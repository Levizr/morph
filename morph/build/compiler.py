from __future__ import annotations

import os
import subprocess
import shutil
from morph.utils.logger import log_info, log_error


class Compiler:
    """Invokes g++ to compile generated C++ into a native binary."""

    def __init__(self):
        self.gpp = shutil.which("g++") or shutil.which("clang++") or "g++"

    def compile(self, source_path: str, binary_path: str,
                needs_freetype: bool = True,
                defines: list[str] | None = None) -> bool:
        if not os.path.exists(source_path):
            log_error(f"Source not found: {source_path}")
            return False

        runtime_dir = os.path.join(os.path.dirname(__file__), "../../runtime")
        runtime_dir = os.path.abspath(runtime_dir)

        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            log_error("Install g++: sudo apt install g++")
            return False

        vendor_dir = os.path.join(runtime_dir, "vendor")
        glad_c = os.path.join(runtime_dir, "core", "glad.c")

        # Runtime source files (compiled separately for incremental builds)
        runtime_sources = [
            os.path.join(runtime_dir, "core", "node.cpp"),
            os.path.join(runtime_dir, "core", "window.cpp"),
            os.path.join(runtime_dir, "render", "gl_renderer.cpp"),
        ]

        cmd = [
            self.gpp,
            "-std=c++17",
            "-O2",
            "-ffunction-sections", "-fdata-sections",
            source_path,
            *runtime_sources,
            glad_c,
            "-o", binary_path,
            "-I", runtime_dir,
            "-I", vendor_dir,
            "-Wl,--gc-sections",
            "-lglfw", "-lGL", "-lX11", "-lpthread", "-ldl",
        ]

        # Feature defines (visible to all translation units)
        if defines:
            for d in defines:
                cmd.append(f"-D{d}")

        if needs_freetype:
            try:
                ft_cflags = subprocess.check_output(
                    ["pkg-config", "--cflags", "freetype2"], text=True
                ).strip().split()
            except Exception:
                ft_cflags = ["-I/usr/include/freetype2"]
            try:
                ft_libs = subprocess.check_output(
                    ["pkg-config", "--libs", "freetype2"], text=True
                ).strip().split()
            except Exception:
                ft_libs = ["-lfreetype"]
            cmd.extend([*ft_cflags, *ft_libs])

        log_info(f"Compiling: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)

        if result.returncode != 0:
            log_error("Compilation failed:")
            for line in result.stderr.split("\n"):
                if line.strip():
                    print(f"  {line.strip()}")
            return False

        return True
