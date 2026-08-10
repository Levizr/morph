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

    # Static link support: GLFW / FreeType / HarfBuzz are bundled into the
    # binary so the app runs on a stock machine without system shared libs.
    # Only the universal base (libc, libstdc++, X11, GL) stays dynamic.
    _STATIC_ARCHIVE_NAMES = {
        "glfw":     ("libglfw3.a", "libglfw.a"),
        "harfbuzz": ("libharfbuzz.a",),
        "freetype": ("libfreetype.a",),
    }

    # Transitive static closure for the freetype static archive on Debian/Ubuntu.
    _STATIC_FREETYPE_CLOSURE = [
        "-lbz2", "-lz", "-lpng16", "-lbrotlidec", "-lbrotlicommon",
    ]

    def _static_lib_dirs(self) -> list[str]:
        dirs: list[str] = []
        env = os.environ.get("MORPH_STATIC_LIBDIRS", "")
        if env:
            dirs.extend(p for p in env.split(os.pathsep) if p)
        dirs.extend([
            "/usr/lib/x86_64-linux-gnu",
            "/usr/local/lib",
            "/usr/lib",
        ])
        return dirs

    def _find_static_archive(self, names: tuple[str, ...]) -> str | None:
        for d in self._static_lib_dirs():
            for n in names:
                p = os.path.join(d, n)
                if os.path.isfile(p):
                    return p
        return None

    def _static_archives(self) -> tuple[str, str, str] | None:
        """Locate glfw/harfbuzz/freetype static archives; None + log on failure."""
        found: dict[str, str] = {}
        for key, names in self._STATIC_ARCHIVE_NAMES.items():
            p = self._find_static_archive(names)
            if p is None:
                log_error(
                    f"Static build needs a static archive for {key} "
                    f"({' / '.join(names)}). "
                    f"Install it (e.g. libglfw3-dev, libharfbuzz-dev) or build it "
                    f"and point MORPH_STATIC_LIBDIRS at its directory."
                )
                return None
            found[key] = p
        return found["glfw"], found["harfbuzz"], found["freetype"]

    def compile(self, source_path: str, binary_path: str,
                needs_freetype: bool = True,
                needs_harfbuzz: bool = True,
                defines: list[str] | None = None,
                static: bool = False) -> bool:
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
            os.path.join(node_dir, "paint_order.cpp"),
            os.path.join(runtime_dir, "core", "window.cpp"),
            os.path.join(runtime_dir, "render", "gl_renderer.cpp"),
            os.path.join(runtime_dir, "core", "compositor.cpp"),
            os.path.join(runtime_dir, "reactivity", "effect.cpp"),
            os.path.join(runtime_dir, "reactivity", "task.cpp"),
            os.path.join(runtime_dir, "net", "net.cpp"),
            # Renderer seam + both backends (compile-time dispatch via
            # MORPH_RENDERER_FORGE picks which one is live; the other is
            # eliminated by -Wl,--gc-sections).
            os.path.join(runtime_dir, "renderers", "renderer.cpp"),
            os.path.join(runtime_dir, "renderers", "flash", "flash.cpp"),
            os.path.join(runtime_dir, "renderers", "forge", "forge.cpp"),
            os.path.join(runtime_dir, "renderers", "forge", "damage.cpp"),
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
            "-I", os.path.join(runtime_dir, "renderers"),
            "-Wl,--gc-sections",
        ]

        if static:
            archives = self._static_archives()
            if archives is None:
                return False
            glfw_a, harfbuzz_a, freetype_a = archives
            # Statically bundle GLFW/HarfBuzz/FreeType (+ their freetype
            # closure). Everything else (GL, X11, libc, libstdc++) stays
            # dynamic — those are universal on any Linux.
            cmd += [
                "-Wl,-Bstatic",
                freetype_a, "-lbz2", "-lz", "-lpng16", "-lm", "-lbrotlidec", "-lbrotlicommon",
                harfbuzz_a,
                glfw_a,
                "-Wl,-Bdynamic",
                "-lGL", "-lX11", "-lrt", "-lpthread", "-ldl",
                "-Wl,-Bstatic", "-lz",
                "-Wl,-Bdynamic", "-lm",
            ]
        else:
            cmd += ["-lglfw", "-lGL", "-lX11", "-lpthread", "-ldl"]

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
            cmd.extend(ft_cflags)
            if not static:
                try:
                    ft_libs = subprocess.check_output(
                        ["pkg-config", "--libs", "freetype2"], text=True
                    ).strip().split()
                except Exception:
                    ft_libs = ["-lfreetype"]
                cmd.extend(ft_libs)

        if needs_harfbuzz:
            try:
                hb_cflags = subprocess.check_output(
                    ["pkg-config", "--cflags", "harfbuzz"], text=True
                ).strip().split()
            except Exception:
                hb_cflags = []
            cmd.extend(hb_cflags)
            if not static:
                try:
                    hb_libs = subprocess.check_output(
                        ["pkg-config", "--libs", "harfbuzz"], text=True
                    ).strip().split()
                except Exception:
                    hb_libs = ["-lharfbuzz"]
                cmd.extend(hb_libs)

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
            "MORPH_FEATURE_POSITION", "MORPH_FEATURE_ZINDEX",
            "MORPH_FEATURE_FLEX",
            "MORPH_FEATURE_CURSOR", "MORPH_FEATURE_BORDER",
            "MORPH_FEATURE_DISPLAY_NONE", "MORPH_FEATURE_INLINE",
            "MORPH_FEATURE_MARGIN_COLLAPSE", "MORPH_FEATURE_MIN_MAX",
            "MORPH_FEATURE_BORDER_BOX", "MORPH_FEATURE_IMAGE",
            "MORPH_FEATURE_DIRTY_RENDERING",
            "MORPH_FEATURE_DEV",
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
