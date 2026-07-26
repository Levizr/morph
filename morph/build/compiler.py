from __future__ import annotations

import os
import subprocess
import shutil
from morph.utils.logger import log_info, log_error


class Compiler:
    """Invokes g++ to compile generated C++ into a native binary."""

    def __init__(self):
        self.gpp = (shutil.which("g++-14") or shutil.which("g++") or
                    shutil.which("clang++") or "g++")
        self.silent = False

    def compile(self, source_path: str, binary_path: str,
                needs_freetype: bool = True,
                needs_harfbuzz: bool = True,
                defines: list[str] | None = None) -> bool:
        if not os.path.exists(source_path):
            log_error(f"Source not found: {source_path}")
            return False

        runtime_dir = os.path.join(os.path.dirname(__file__), "../runtime")
        runtime_dir = os.path.abspath(runtime_dir)

        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            log_error("Install g++-14: sudo apt install g++-14")
            return False

        vendor_dir = os.path.join(runtime_dir, "vendor")
        glad_c   = os.path.join(vendor_dir, "glad", "glad.c")
        stb_c    = os.path.join(vendor_dir, "stb_image.c")

        # Runtime source files (compiled separately for incremental builds)
        node_dir = os.path.join(runtime_dir, "core", "node")
        runtime_sources = [
            os.path.join(node_dir, "node.cpp"),
            os.path.join(node_dir, "events.cpp"),
            os.path.join(node_dir, "flatten.cpp"),
            os.path.join(node_dir, "style.cpp"),
            os.path.join(node_dir, "layout.cpp"),
            os.path.join(runtime_dir, "core", "window.cpp"),
            os.path.join(runtime_dir, "render", "gl_renderer.cpp"),
            os.path.join(runtime_dir, "core", "compositor.cpp"),
            os.path.join(runtime_dir, "reactivity", "effect.cpp"),
            os.path.join(runtime_dir, "reactivity", "task.cpp"),
        ]

        cmd = [
            self.gpp,
            "-std=c++23",
            "-O2",
            "-ffunction-sections", "-fdata-sections",
            source_path,
            *runtime_sources,
            glad_c, stb_c,
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

        if needs_harfbuzz:
            try:
                hb_cflags = subprocess.check_output(
                    ["pkg-config", "--cflags", "harfbuzz"], text=True
                ).strip().split()
            except Exception:
                hb_cflags = []
            try:
                hb_libs = subprocess.check_output(
                    ["pkg-config", "--libs", "harfbuzz"], text=True
                ).strip().split()
            except Exception:
                hb_libs = ["-lharfbuzz"]
            cmd.extend([*hb_cflags, *hb_libs])

        if not self.silent:
            log_info(f"Compiling: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)

        if result.returncode != 0:
            log_error("Compilation failed:")
            for line in result.stderr.split("\n"):
                if line.strip():
                    print(f"  {line.strip()}")
            return False

        return True

    def compile_shared(self, source_path: str, output_path: str,
                       defines: list[str] | None = None) -> bool:
        if not os.path.exists(source_path):
            log_error(f"Source not found: {source_path}")
            return False

        runtime_dir = os.path.join(os.path.dirname(__file__), "../runtime")
        runtime_dir = os.path.abspath(runtime_dir)
        dev_dir = os.path.join(runtime_dir, "dev")
        vendor_dir = os.path.join(runtime_dir, "vendor")

        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            return False

        cmd = [
            self.gpp,
            "-std=c++23",
            "-O0", "-g",  # fast compilation, debug symbols for dev
            "-shared", "-fPIC",
            source_path,
            "-o", output_path,
            "-I", runtime_dir,
            "-I", dev_dir,
            "-I", vendor_dir,
        ]

        # Feature defines (same as build for compat)
        _DEV_FEATURES = [
            "MORPH_FEATURE_SCROLL", "MORPH_FEATURE_RADIUS",
            "MORPH_FEATURE_TEXT", "MORPH_FEATURE_BOLD",
            "MORPH_FEATURE_POSITION", "MORPH_FEATURE_FLEX",
            "MORPH_FEATURE_CURSOR", "MORPH_FEATURE_BORDER",
            "MORPH_FEATURE_DISPLAY_NONE", "MORPH_FEATURE_INLINE",
            "MORPH_FEATURE_MARGIN_COLLAPSE", "MORPH_FEATURE_MIN_MAX",
            "MORPH_FEATURE_BORDER_BOX", "MORPH_FEATURE_IMAGE",
            "MORPH_FEATURE_DIRTY_RENDERING",
        ]
        for d in _DEV_FEATURES:
            cmd.append(f"-D{d}")
        if defines:
            for d in defines:
                cmd.append(f"-D{d}")

        # FreeType and HarfBuzz headers (needed by node.h → gl_renderer.h)
        try:
            ft_cflags = subprocess.check_output(
                ["pkg-config", "--cflags", "freetype2"], text=True
            ).strip().split()
            hb_cflags = subprocess.check_output(
                ["pkg-config", "--cflags", "harfbuzz"], text=True
            ).strip().split()
            cmd.extend([*ft_cflags, *hb_cflags])
        except Exception:
            pass

        if not self.silent:
            log_info(f"Compiling shared library: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)

        if result.returncode != 0:
            log_error("Shared library compilation failed:")
            for line in result.stderr.split("\n"):
                if line.strip():
                    print(f"  {line.strip()}")
            return False

        return True
