from __future__ import annotations

import os
import shlex
import subprocess
import shutil
from morph.build.platform import current, is_macos, is_windows, shared_lib_flag
from morph.utils.logger import log_info, log_error, log_warn

# Memoized result of the clang++ probe: False = not probed yet,
# None = no usable clang++, str = path of a working driver.
_clang_cache: "str | None | False" = False


def find_clang() -> "str | None":
    """Locate the newest usable clang++ for dev hot-reload compiles.

    Scans versioned names (clang++-21 … clang++-14) plus the unversioned
    `clang++`, newest first, and probes the first hit with a trivial
    -std=c++20 syntax check (some distro pairings of older clang + newer
    libstdc++ can't parse C++20 headers — those are skipped). The probe
    runs once per process and is memoized.
    """
    global _clang_cache
    if _clang_cache is not False:
        return _clang_cache or None

    candidates = ["clang++"] + [f"clang++-{v}" for v in range(21, 13, -1)]
    for cand in candidates:
        path = shutil.which(cand)
        if not path:
            continue
        try:
            r = subprocess.run(
                [path, "-std=c++20", "-x", "c++", "-fsyntax-only", os.devnull],
                capture_output=True, timeout=30)
            if r.returncode == 0:
                _clang_cache = path
                return path
        except (OSError, subprocess.TimeoutExpired):
            continue
    _clang_cache = None
    return None


class Compiler:
    """Invokes g++ to compile generated C++ into a native binary."""

    def __init__(self, cxx: "str | None" = None):
        """`cxx` overrides the compiler executable (morph.config.json
        build.cxx / build.dev_cxx or MORPH_CXX / MORPH_DEV_CXX). A missing
        override falls back to the platform default with a warning."""
        from morph.build.platform import pick_cpp
        default = pick_cpp()
        if cxx:
            if shutil.which(cxx):
                self.gpp = cxx
            else:
                log_warn(f"configured compiler '{cxx}' not found on PATH — "
                         f"falling back to '{default}'")
                self.gpp = default
        else:
            self.gpp = default
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
        if is_macos():
            dirs.extend(["/opt/homebrew/lib", "/usr/local/lib", "/usr/lib"])
        elif is_windows():
            # MinGW static archives live alongside the compiler install.
            exe = shutil.which(self.gpp) or self.gpp
            base = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(exe))))
            for cand in [os.path.join(base, "lib"),
                         os.path.join(base, "x86_64-w64-mingw32", "lib"),
                         "/mingw64/lib", "/mingw32/lib"]:
                if os.path.isdir(cand):
                    dirs.append(cand)
        else:
            dirs.extend([
                "/usr/lib/x86_64-linux-gnu",
                "/usr/lib/aarch64-linux-gnu",
                "/usr/lib/arm-linux-gnueabihf",
                "/usr/local/lib",
                "/usr/lib",
            ])
        return dirs

    def _system_include_dirs(self) -> list[str]:
        """OS-appropriate fallback include roots when pkg-config is missing."""
        if is_macos():
            return ["/opt/homebrew/include", "/usr/local/include"]
        if is_windows():
            return []
        return ["/usr/include"]

    def _find_static_archive(self, names: tuple[str, ...]) -> str | None:
        for d in self._static_lib_dirs():
            for n in names:
                p = os.path.join(d, n)
                if os.path.isfile(p):
                    return p
        return None

    def _static_archives(self, needs_freetype: bool,
                         needs_harfbuzz: bool,
                         wayland: bool = False,
                         system_freetype: bool = False) -> dict | None:
        """Locate (or build from source) the static archives morph needs.

        Order matters: freetype must be resolved before harfbuzz, because a
        self-built freetype is what harfbuzz links its shape engine against.
        """
        from morph.build.static_deps import StaticDeps
        deps = StaticDeps(self.gpp, wayland=wayland)
        info: dict = {}

        def _resolve(key: str, force_build: bool = False) -> bool:
            names = self._STATIC_ARCHIVE_NAMES[key]
            if not force_build:
                found = self._find_static_archive(names)
                if found is not None:
                    info[key] = {"archive": found, "self_built": False}
                    return True
            if os.environ.get("MORPH_NO_BUILD_DEPS"):
                log_error(
                    f"Static build needs a static archive for {key} "
                    f"({' / '.join(names)}). "
                    f"Install it (e.g. libglfw3-dev, libharfbuzz-dev) or build it "
                    f"and point MORPH_STATIC_LIBDIRS at its directory."
                )
                return False
            built = deps.build(key)
            if built is not None:
                info[key] = built
                return True
            # Fall back to a system archive if the source build failed.
            fallback = self._find_static_archive(names)
            if fallback is not None:
                log_error(
                    f"Could not build {key} from source — falling back to "
                    f"{fallback} (bigger binary, fewer size optimizations)."
                )
                info[key] = {"archive": fallback, "self_built": False}
                return True
            return False

        if not _resolve("glfw"):
            return None
        # Default: build the trimmed freetype ourselves (smallest result).
        # system_freetype: true prefers the system archive instead.
        if needs_freetype:
            if not _resolve("freetype", force_build=not system_freetype):
                return None
        if needs_harfbuzz:
            if not _resolve("harfbuzz"):
                return None
        return info

    def _static_size_flags(self) -> tuple[str, list[str]]:
        override = os.environ.get("MORPH_STATIC_CFLAGS")
        if override:
            return "", shlex.split(override)
        flags = ["-s"]
        if "clang" not in self.gpp:
            flags.insert(0, "-flto")
        return "-Os", flags

    def _native_flags(self, native: "object | None") -> list[str]:
        """Convert NativeConfig into g++ flags (-I, -L, -l, cflags, ldflags)."""
        if native is None:
            return []
        flags: list[str] = []
        for d in getattr(native, "include_dirs", []):
            if d:
                flags += ["-I", d]
        for d in getattr(native, "library_dirs", []):
            if d:
                flags += ["-L", d]
        for lib in getattr(native, "libraries", []):
            if lib:
                flags.append(f"-l{lib}")
        flags.extend(getattr(native, "cflags", []))
        flags.extend(getattr(native, "ldflags", []))
        return flags

    def compile(self, source_path: str, binary_path: str,
                needs_freetype: bool = True,
                needs_harfbuzz: bool = True,
                defines: list[str] | None = None,
                static: bool = False,
                wayland: bool = False,
                system_freetype: bool = False,
                native: "object | None" = None) -> bool:
        if not os.path.exists(source_path):
            log_error(f"Source not found: {source_path}")
            return False

        runtime_dir = os.path.join(os.path.dirname(__file__), "../../runtime/cpp")
        runtime_dir = os.path.abspath(runtime_dir)

        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            log_error("Install a C++23 compiler (run `morph doctor` for per-OS instructions)")
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
            os.path.join(node_dir, "animation.cpp"),
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

        if static:
            opt, size_flags = self._static_size_flags()
            timeout = 300
        else:
            opt, size_flags = "-O2", []
            timeout = 600  # -O2 whole-runtime in one invocation; slow/loaded
                           # machines (and CI) routinely exceed 120s

        cmd = [
            self.gpp,
            "-std=c++23",
        ]
        if opt:
            cmd.append(opt)
        gc_flag = "-Wl,-dead_strip" if is_macos() else "-Wl,--gc-sections"
        cmd += [
            "-ffunction-sections", "-fdata-sections",
            *size_flags,
            source_path,
            *runtime_sources,
            glad_c, stb_c,
            "-o", binary_path,
            "-I", runtime_dir,
            "-I", vendor_dir,
            "-I", os.path.join(runtime_dir, "renderers"),
            gc_flag,
        ]

        archives: dict = {}
        if static:
            archives = self._static_archives(
                needs_freetype, needs_harfbuzz,
                wayland=wayland, system_freetype=system_freetype,
            )
            if archives is None:
                return False
            static_libs: list[str] = []
            if "freetype" in archives:
                static_libs.append(archives["freetype"]["archive"])
                if not archives["freetype"].get("self_built"):
                    static_libs += [
                        "-lbz2", "-lz", "-lpng16", "-lbrotlidec", "-lbrotlicommon",
                    ]
            if "harfbuzz" in archives:
                static_libs.append(archives["harfbuzz"]["archive"])
            static_libs.append(archives["glfw"]["archive"])
            # Statically bundle GLFW/HarfBuzz/FreeType (+ their freetype
            # closure). Everything else (GL, X11/Cocoa/Win32, libc, libstdc++)
            # stays dynamic — those are universal on any desktop OS.
            plat = current()
            if plat == "macos":
                dynamic = ["-framework", "Cocoa", "-framework", "OpenGL",
                           "-framework", "IOKit", "-framework", "CoreVideo",
                           "-lpthread"]
            elif plat == "windows":
                dynamic = ["-lopengl32", "-lgdi32", "-lshell32", "-luser32",
                           "-lcomdlg32", "-lole32", "-lsetupapi", "-lws2_32",
                           "-lpthread", "-lm"]
            else:
                dynamic = ["-lGL", "-lX11", "-lXrandr", "-lXinerama",
                           "-lXcursor", "-lXi", "-lrt", "-lpthread", "-ldl",
                           "-lm"]
                if wayland:
                    # GLFW Wayland backend: EGL + wayland-client/xkbcommon
                    # stay dynamic (they are system libs, like X11/GL).
                    dynamic[1:1] = ["-lwayland-client", "-lwayland-cursor",
                                    "-lwayland-egl", "-lxkbcommon", "-lEGL"]
            if plat == "macos":
                # ld64 has no -Bstatic; pass the archives directly.
                cmd += static_libs + dynamic
            else:
                cmd += ["-Wl,-Bstatic", *static_libs, "-Wl,-Bdynamic", *dynamic]
        else:
            plat = current()
            if plat == "macos":
                cmd += ["-lglfw", "-framework", "Cocoa", "-framework", "OpenGL",
                        "-framework", "IOKit", "-framework", "CoreVideo",
                        "-lpthread"]
            elif plat == "windows":
                cmd += ["-lglfw3", "-lopengl32", "-lgdi32", "-lshell32",
                        "-luser32", "-lcomdlg32", "-lole32", "-lws2_32",
                        "-lpthread"]
            else:
                cmd += ["-lglfw", "-lGL", "-lX11", "-lpthread", "-ldl"]

        # Feature defines (visible to all translation units)
        if defines:
            for d in defines:
                cmd.append(f"-D{d}")

        # User native build options (external includes/libs/cflags)
        cmd.extend(self._native_flags(native))

        if needs_freetype:
            ft_self_built = static and archives.get("freetype", {}).get("self_built")
            if ft_self_built:
                ft_cflags = [f"-I{archives['freetype']['include_dir']}"]
            else:
                try:
                    ft_cflags = subprocess.check_output(
                        ["pkg-config", "--cflags", "freetype2"], text=True
                    ).strip().split()
                except Exception:
                    ft_cflags = [f"-I{d}/freetype2" for d in self._system_include_dirs()]
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
            hb_self_built = static and archives.get("harfbuzz", {}).get("self_built")
            if hb_self_built:
                hb_cflags = [f"-I{archives['harfbuzz']['include_dir']}"]
            else:
                try:
                    hb_cflags = subprocess.check_output(
                        ["pkg-config", "--cflags", "harfbuzz"], text=True
                    ).strip().split()
                except Exception:
                    hb_cflags = [f"-I{d}/harfbuzz" for d in self._system_include_dirs()]
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
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)

        if result.returncode != 0:
            log_error("Compilation failed:")
            for line in result.stderr.split("\n"):
                if line.strip():
                    print(f"  {line.strip()}")
            return False

        return True

    def _pkg_config_cflags(self, cache: dict, pkgs: list[tuple[str, str]]) -> list[str]:
        """pkg-config cflags for the given packages, memoized per instance."""
        out: list[str] = []
        for pkg, sub in pkgs:
            key = f"cflags:{pkg}"
            if key not in cache:
                try:
                    cache[key] = subprocess.check_output(
                        ["pkg-config", "--cflags", pkg], text=True
                    ).strip().split()
                except Exception:
                    dirs = " ".join(f"-I{d}/{sub}" for d in self._system_include_dirs())
                    cache[key] = dirs.split()
            out.extend(cache[key])
        return out

    def _dev_shared_flags(self, defines: list[str] | None,
                          native: "object | None") -> list[str]:
        """Flag set shared by dev logic .so and dev PCH compiles.

        The PCH is only valid when these flags match exactly what the logic
        compile uses (GCC validates and silently falls back to a full parse
        on mismatch), so both paths must build their cmd from this one list.
        """
        runtime_dir = os.path.join(os.path.dirname(__file__), "../../runtime/cpp")
        runtime_dir = os.path.abspath(runtime_dir)
        dev_dir = os.path.join(runtime_dir, "dev")
        vendor_dir = os.path.join(runtime_dir, "vendor")

        cmd = [
            # c++20: the runtime header chain uses no C++23 features, and
            # GCC parses its C++23 library bits ~1s slower per TU.
            "-std=c++20",
            "-O0",  # fast compilation for dev hot-reload
            shared_lib_flag(), "-fPIC",
            "-I", runtime_dir,
            "-I", dev_dir,
            "-I", vendor_dir,
        ]

        # Feature defines for the dev logic.so. MUST match the layout-affecting
        # feature set of morph_devrt (runtime/dev/CMakeLists.txt): MorphNode /
        # MorphStyle are header-only, so any divergence (e.g. a missing
        # MORPH_FEATURE_OPACITY) changes the class layout between the devrt
        # binary and the dynamically loaded logic.so and causes ODR mismatches
        # (garbage std::function -> SIGSEGV on click handlers).
        _DEV_FEATURES = [
            "MORPH_FEATURE_SCROLL", "MORPH_FEATURE_RADIUS",
            "MORPH_FEATURE_TEXT", "MORPH_FEATURE_BOLD",
            "MORPH_FEATURE_POSITION", "MORPH_FEATURE_ZINDEX",
            "MORPH_FEATURE_OPACITY", "MORPH_FEATURE_FLEX",
            "MORPH_FEATURE_CURSOR", "MORPH_FEATURE_BORDER",
            "MORPH_FEATURE_DISPLAY_NONE", "MORPH_FEATURE_INLINE",
            "MORPH_FEATURE_MARGIN_COLLAPSE", "MORPH_FEATURE_MIN_MAX",
            "MORPH_FEATURE_BORDER_BOX", "MORPH_FEATURE_IMAGE",
            "MORPH_FEATURE_DIRTY_RENDERING",
            "MORPH_FEATURE_INPUT",
            "MORPH_FEATURE_TRANSFORM",
            "MORPH_FEATURE_ANIMATION",
            "MORPH_FEATURE_DEV",
        ]
        for d in _DEV_FEATURES:
            cmd.append(f"-D{d}")
        if defines:
            for d in defines:
                cmd.append(f"-D{d}")

        # User native build options (external includes/libs/cflags)
        cmd.extend(self._native_flags(native))

        # FreeType and HarfBuzz headers (needed by node.h → gl_renderer.h)
        pkg_cache: dict = getattr(self, "_pkg_cache", None)
        if pkg_cache is None:
            pkg_cache = self._pkg_cache = {}
        cmd.extend(self._pkg_config_cflags(
            pkg_cache, [("freetype2", "freetype2"), ("harfbuzz", "harfbuzz")]))
        return cmd

    def compile_pch(self, header_path: str, output_path: str,
                    defines: list[str] | None = None,
                    native: "object | None" = None) -> bool:
        """Precompile `header_path` for reuse across dev logic compiles.

        GCC output keeps the `<header>.gch` convention (auto-picked up by
        `-include <header>`); clang gets an explicit `.pch` file used via
        `-include-pch`. Both must be built with the exact same flags as
        `compile_shared` uses — handled by `_dev_shared_flags`.
        """
        if not os.path.exists(header_path):
            log_error(f"PCH header not found: {header_path}")
            return False
        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            return False

        cmd = [self.gpp]
        cmd.extend(self._dev_shared_flags(defines, native))
        if "clang" in os.path.basename(self.gpp):
            cmd.extend(["-x", "c++-header", "-emit-pch",
                        header_path, "-o", output_path])
        else:
            cmd.extend(["-x", "c++-header", header_path, "-o", output_path])

        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        if result.returncode != 0:
            log_error("PCH compilation failed:")
            for line in result.stderr.split("\n"):
                if line.strip():
                    print(f"  {line.strip()}")
            return False
        return True

    def compile_shared(self, source_path: str, output_path: str,
                       defines: list[str] | None = None,
                       native: "object | None" = None,
                       pch_header: str | None = None) -> bool:
        if not os.path.exists(source_path):
            log_error(f"Source not found: {source_path}")
            return False

        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            return False

        cmd = [self.gpp]
        cmd.extend(self._dev_shared_flags(defines, native))
        if pch_header:
            base, _ = os.path.splitext(pch_header)
            if "clang" in os.path.basename(self.gpp):
                pch_file = base + ".pch"
                if os.path.exists(pch_file):
                    cmd.append(f"-include-pch")
                    cmd.append(pch_file)
            elif os.path.exists(base + ".gch"):
                # -include <header> auto-loads <header>.gch when flag-compatible
                cmd.append("-include")
                cmd.append(pch_header)
        cmd.append(source_path)
        cmd.extend(["-o", output_path])

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
