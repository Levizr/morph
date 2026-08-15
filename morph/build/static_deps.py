"""Build minimal static archives for GLFW / FreeType / HarfBuzz on demand so
`morph build --static` produces the smallest possible self-contained binary.

Each library is compiled with -Os, function/data sections and (when using gcc)
LTO, only the subsets morph actually uses are linked, and the results are
cached under ~/.cache/morph/static (override with MORPH_STATIC_CACHE).
Source tarballs are looked up in MORPH_SRC_DIR first, then downloaded.
"""

from __future__ import annotations

import hashlib
import json
import multiprocessing
import os
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request

from morph.utils.logger import log_info, log_error, log_success


_DEP_SPECS: dict[str, dict] = {
    "glfw": {
        "version": "3.4",
        "archive": "glfw-3.4.tar.gz",
        "urls": ["https://github.com/glfw/glfw/archive/refs/tags/3.4.tar.gz"],
        "lib": "libglfw3.a",
    },
    "freetype": {
        "version": "2.13.3",
        "archive": "freetype-2.13.3.tar.gz",
        "urls": [
            "https://download.savannah.gnu.org/releases/freetype/freetype-2.13.3.tar.gz",
            "https://github.com/freetype/freetype/archive/refs/tags/VER-2-13-3.tar.gz",
        ],
        "lib": "libfreetype.a",
    },
    "harfbuzz": {
        "version": "8.5.0",
        "archive": "harfbuzz-8.5.0.tar.gz",
        "urls": ["https://github.com/harfbuzz/harfbuzz/archive/refs/tags/8.5.0.tar.gz"],
        "lib": "libharfbuzz.a",
    },
}

_FT_MODULE_KEEP = {
    "tt_driver_class",
    "sfnt_module_class",
    "ft_smooth_renderer_class",
    "ft_raster1_renderer_class",
}

_HB_WANTED_OPTS = {
    "freetype": "enabled",
    "glib": "disabled",
    "gobject": "disabled",
    "cairo": "disabled",
    "fontconfig": "disabled",
    "icu": "disabled",
    "graphite": "disabled",
    "graphite2": "disabled",
    "introspection": "disabled",
    "gtk_doc": "disabled",
    "tests": "disabled",
    "tools": "disabled",
    "utilities": "disabled",
    "docs": "disabled",
    "benchmark": "disabled",
    "subset": "disabled",
    "wasm": "disabled",
}


class StaticDeps:
    def __init__(self, gpp: str, wayland: bool = False):
        self.gpp = gpp
        self.wayland = wayland
        from morph.build.platform import current
        self.platform = current()
        self.is_clang = "clang" in gpp
        self.cc, self.cxx = self._compiler_pair(gpp)
        version_match = (not self.is_clang) and (
            self._dumpver(self.cc) == self._dumpver(self.cxx))
        self.lto_tools = self._lto_tools()
        # LTO objects must be indexed by LTO-enabled ar/ranlib; without them
        # (e.g. some MinGW installs), -flto would break archive creation.
        self.lto_ok = version_match and "ar" in self.lto_tools and "ranlib" in self.lto_tools
        env = os.environ.get("MORPH_STATIC_CACHE", "")
        if env:
            self.cache = os.path.abspath(env)
        else:
            base = os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
            self.cache = os.path.join(base, "morph", "static")
        self.src_dir = os.environ.get("MORPH_SRC_DIR", "")
        if not self.src_dir:
            self.src_dir = os.path.join(self.cache, "src")
        self.prefix: dict[str, str] = {}
        self._manifest = self._load_manifest()

    def _compiler_pair(self, gpp: str) -> tuple[str, str]:
        base = os.path.basename(gpp)
        if "clang" in base:
            cc = shutil.which("clang") or "clang"
            cxx = shutil.which(base) or base
            return cc, cxx
        # Handles g++, g++-14, and MinGW x86_64-w64-mingw32-g++.
        if base.endswith("g++"):
            ccname = base[:-3] + "gcc"
            cxxname = base
        else:
            m = re.match(r"g\+\+(-(\d+))?$", base)
            ver = m.group(2) if m else None
            ccname = f"gcc-{ver}" if ver else "gcc"
            cxxname = base
        cc = shutil.which(ccname) or ccname
        cxx = shutil.which(cxxname) or cxxname
        return cc, cxx

    def _dumpver(self, exe: str) -> str | None:
        try:
            return subprocess.check_output(
                [exe, "-dumpversion"], text=True).strip()
        except Exception:
            return None

    def build(self, dep: str) -> dict | None:
        spec = _DEP_SPECS[dep]
        prefix = os.path.join(self.cache, "prefix", dep)
        self.prefix[dep] = prefix
        cur_hash = self._build_hash(dep)
        cached = self._find_archive(prefix, spec["lib"])
        entry = self._manifest.get(dep)
        if entry and entry.get("hash") == cur_hash and cached:
            return self._info(dep, prefix, cached)
        log_success(f"Building {dep} {spec['version']} from source (minimal static)...")
        self._reset_dir(prefix)
        if not self._build_dep(dep, prefix):
            return None
        archive = self._find_archive(prefix, spec["lib"])
        if not archive:
            log_error(f"Build produced no archive ({spec['lib']}) under {prefix}")
            return None
        self._manifest[dep] = {"hash": cur_hash, "version": spec["version"]}
        self._save_manifest()
        log_success(f"{dep} {spec['version']} built at {archive}")
        return self._info(dep, prefix, archive)

    def _info(self, dep: str, prefix: str, archive: str) -> dict:
        info = {"archive": archive, "self_built": True}
        if dep == "freetype":
            inc = self._find_include(prefix, "freetype2")
            if inc:
                info["include_dir"] = inc
        if dep == "harfbuzz":
            inc = self._find_include(prefix, "harfbuzz")
            if inc:
                info["include_dir"] = inc
        return info

    def _find_archive(self, prefix: str, libname: str) -> str | None:
        for root, _dirs, files in os.walk(prefix):
            if libname in files:
                return os.path.join(root, libname)
        return None

    def _find_include(self, prefix: str, name: str) -> str | None:
        direct = os.path.join(prefix, "include", name)
        if os.path.isdir(direct):
            return direct
        for root, dirs, _files in os.walk(prefix):
            if os.path.basename(root) == name and dirs:
                return root
        return None

    def _pkgconfig_dirs(self, prefix: str) -> list[str]:
        dirs = []
        for root, _dirs, _files in os.walk(prefix):
            if os.path.basename(root) == "pkgconfig":
                dirs.append(root)
        return dirs

    def _build_dep(self, dep: str, prefix: str) -> bool:
        if dep == "glfw":
            return self._build_glfw(prefix)
        if dep == "freetype":
            return self._build_freetype(prefix)
        if dep == "harfbuzz":
            return self._build_harfbuzz(prefix)
        return False

    def _build_hash(self, dep: str) -> str:
        h = hashlib.sha256()
        h.update(dep.encode())
        h.update(_DEP_SPECS[dep]["version"].encode())
        h.update(self.gpp.encode())
        h.update(self.cc.encode())
        h.update(self.cxx.encode())
        h.update(self.platform.encode())
        if dep == "glfw":
            h.update(str(self.wayland).encode())
        if dep == "freetype":
            h.update(",".join(sorted(_FT_MODULE_KEEP)).encode())
        if dep == "harfbuzz":
            h.update("|".join(self._freetype_include_args()).encode())
        return h.hexdigest()[:16]

    def _load_manifest(self) -> dict:
        try:
            with open(os.path.join(self.cache, "manifest.json")) as f:
                return json.load(f)
        except (FileNotFoundError, OSError, ValueError):
            return {}

    def _save_manifest(self) -> None:
        os.makedirs(self.cache, exist_ok=True)
        with open(os.path.join(self.cache, "manifest.json"), "w") as f:
            json.dump(self._manifest, f, indent=2)

    def _run(self, cmd: list[str], step: str, timeout: int = 1800,
             env: dict | None = None) -> bool:
        log_info(f"{step} ...")
        try:
            r = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout,
                               env=env)
        except subprocess.TimeoutExpired:
            log_error(f"{step} timed out")
            return False
        if r.returncode != 0:
            lines = (r.stdout + r.stderr).strip().split("\n")
            for line in lines[-60:]:
                print(f"    {line}")
            return False
        return True

    def _nproc(self) -> int:
        return max(1, multiprocessing.cpu_count())

    def _reset_dir(self, d: str) -> None:
        if os.path.exists(d):
            shutil.rmtree(d)
        os.makedirs(d, exist_ok=True)

    def _cflags_extra(self) -> str:
        flags = ["-Os", "-ffunction-sections", "-fdata-sections"]
        if self.lto_ok:
            flags.append("-flto")
        return " ".join(flags)

    def _lto_tools(self) -> dict[str, str]:
        """Find LTO-aware ar/ranlib/nm for the active gcc (gcc-ar etc.)."""
        if self.is_clang:
            return {}
        tools: dict[str, str] = {}
        ccbase = os.path.basename(self.cc)
        for name in ("ar", "ranlib", "nm"):
            p = None
            if "gcc" in ccbase:
                p = shutil.which(ccbase.replace("gcc", f"gcc-{name}", 1))
            if p is None:
                p = shutil.which(f"gcc-{name}")
            if p:
                tools[name] = p
        return tools

    def _cmake_gnu_lto_tools(self, args: list[str]) -> None:
        if not self.lto_ok:
            return
        for name, var in (("ar", "CMAKE_AR"), ("ranlib", "CMAKE_RANLIB"),
                          ("nm", "CMAKE_NM")):
            p = self.lto_tools.get(name)
            if p:
                args.append(f"-D{var}={p}")

    def _source_tarball(self, dep: str) -> str:
        spec = _DEP_SPECS[dep]
        name = spec["archive"]
        path = os.path.join(self.src_dir, name)
        if os.path.isfile(path):
            return path
        os.makedirs(self.src_dir, exist_ok=True)
        last_err = None
        for url in spec["urls"]:
            log_info(f"Downloading {name} ...")
            try:
                req = urllib.request.Request(url, headers={"User-Agent": "morph/static"})
                with urllib.request.urlopen(req, timeout=60) as resp, open(path, "wb") as f:
                    while True:
                        chunk = resp.read(65536)
                        if not chunk:
                            break
                        f.write(chunk)
                return path
            except Exception as e:
                last_err = e
                log_error(f"Download failed ({url}): {e}")
        raise RuntimeError(f"Could not obtain {name}: {last_err}")

    def _extract(self, dep: str, tarball: str) -> str:
        work = os.path.join(self.cache, "build")
        os.makedirs(work, exist_ok=True)
        dst = os.path.join(work, dep)
        if os.path.exists(dst):
            shutil.rmtree(dst)
        os.makedirs(dst)
        with tarfile.open(tarball, "r:*") as tf:
            tf.extractall(dst)
        entries = [e for e in os.listdir(dst) if not e.startswith(".")]
        if len(entries) == 1:
            return os.path.join(dst, entries[0])
        return dst

    def _glfw_platform_flags(self) -> list[str]:
        if self.platform == "macos":
            return ["-DGLFW_BUILD_COCOA=ON",
                    "-DGLFW_BUILD_WAYLAND=OFF", "-DGLFW_BUILD_X11=OFF"]
        if self.platform == "windows":
            return ["-DGLFW_BUILD_WIN32=ON",
                    "-DGLFW_BUILD_WAYLAND=OFF", "-DGLFW_BUILD_X11=OFF"]
        return ["-DGLFW_BUILD_X11=ON",
                "-DGLFW_BUILD_WAYLAND=ON" if self.wayland
                else "-DGLFW_BUILD_WAYLAND=OFF"]

    def _build_glfw(self, prefix: str) -> bool:
        tarball = self._source_tarball("glfw")
        src = self._extract("glfw", tarball)
        build = os.path.join(self.cache, "build", "glfw-build")
        self._reset_dir(build)
        args = [
            "cmake", "-S", src, "-B", build,
            "-DCMAKE_BUILD_TYPE=Release",
            f"-DCMAKE_INSTALL_PREFIX={prefix}",
            f"-DCMAKE_C_COMPILER={self.cc}",
            f"-DCMAKE_CXX_COMPILER={self.cxx}",
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DGLFW_BUILD_EXAMPLES=OFF",
            "-DGLFW_BUILD_TESTS=OFF",
            "-DGLFW_BUILD_DOCS=OFF",
            "-DGLFW_INSTALL=ON",
            *self._glfw_platform_flags(),
            f"-DCMAKE_C_FLAGS={self._cflags_extra()}",
        ]
        self._cmake_gnu_lto_tools(args)
        if not self._run(args, "Configuring glfw"):
            return False
        if not self._run(["cmake", "--build", build, "--parallel"],
                         "Compiling glfw"):
            return False
        return self._run(["cmake", "--install", build], "Installing glfw")

    def _write_trimmed_ftmodule(self, src: str) -> None:
        """Overwrite freetype's ftmodule.h with only the modules morph uses.

        The source tree is a per-build scratch copy (reset on every build), so
        patching it directly is safe and avoids fragile -D...="..." quoting.
        """
        module_h = os.path.join(src, "include", "freetype", "config", "ftmodule.h")
        pat = re.compile(r"FT_USE_MODULE\s*\(\s*[^,]+,\s*(\w+)\s*\)")
        keep = []
        try:
            with open(module_h) as f:
                for line in f:
                    m = pat.search(line)
                    if m and m.group(1) in _FT_MODULE_KEEP:
                        keep.append(line)
        except OSError:
            keep = []
        with open(module_h, "w") as f:
            f.write("/* trimmed by morph --static: only modules morph uses */\n")
            f.writelines(keep)

    def _build_freetype(self, prefix: str) -> bool:
        tarball = self._source_tarball("freetype")
        src = self._extract("freetype", tarball)
        build = os.path.join(self.cache, "build", "freetype-build")
        self._reset_dir(build)
        self._write_trimmed_ftmodule(src)
        cflags = self._cflags_extra()
        args = [
            "cmake", "-S", src, "-B", build,
            "-DCMAKE_BUILD_TYPE=MinSizeRel",
            f"-DCMAKE_INSTALL_PREFIX={prefix}",
            f"-DCMAKE_C_COMPILER={self.cc}",
            f"-DCMAKE_CXX_COMPILER={self.cxx}",
            "-DCMAKE_POSITION_INDEPENDENT_CODE=ON",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DFT_DISABLE_ZLIB=ON",
            "-DFT_DISABLE_BZIP2=ON",
            "-DFT_DISABLE_PNG=ON",
            "-DFT_DISABLE_HARFBUZZ=ON",
            "-DFT_DISABLE_BROTLI=ON",
            f"-DCMAKE_C_FLAGS={cflags}",
        ]
        self._cmake_gnu_lto_tools(args)
        if not self._run(args, "Configuring freetype"):
            return False
        if not self._run(["cmake", "--build", build, "--parallel"],
                         "Compiling freetype"):
            return False
        if not self._run(["cmake", "--install", build], "Installing freetype"):
            return False
        self._ensure_freetype_pc(prefix)
        return True

    def _ensure_freetype_pc(self, prefix: str) -> None:
        pc_dir = os.path.join(prefix, "lib", "pkgconfig")
        pc = os.path.join(pc_dir, "freetype2.pc")
        if os.path.isfile(pc):
            return
        os.makedirs(pc_dir, exist_ok=True)
        with open(pc, "w") as f:
            f.write(f"prefix={prefix}\n")
            f.write(f"includedir={prefix}/include\n")
            f.write(f"libdir={prefix}/lib\n")
            f.write("Name: freetype2\n")
            f.write("Description: FreeType 2 font engine (morph static build)\n")
            f.write("Version: 24.3.18\n")
            f.write("Libs: -L${libdir} -lfreetype\n")
            f.write("Cflags: -I${includedir}/freetype2\n")

    def _freetype_include_args(self) -> list[str]:
        ft_prefix = self.prefix.get("freetype")
        if ft_prefix:
            inc = os.path.join(ft_prefix, "include", "freetype2")
            if os.path.isdir(inc):
                return [f"-I{inc}"]
        try:
            out = subprocess.check_output(
                ["pkg-config", "--cflags", "freetype2"], text=True).strip()
            return out.split()
        except Exception:
            return []

    def _venv_scripts_dir(self) -> str:
        return "Scripts" if os.name == "nt" else "bin"

    def _meson(self) -> str | None:
        if shutil.which("meson") and shutil.which("ninja"):
            return shutil.which("meson")
        venv = os.path.join(self.cache, "venv")
        scripts = self._venv_scripts_dir()
        meson_bin = os.path.join(venv, scripts, "meson" + (".exe" if os.name == "nt" else ""))
        if os.path.isfile(meson_bin):
            return meson_bin
        log_success("Bootstrapping meson+ninja (needed to build harfbuzz)...")
        if not self._run([sys.executable, "-m", "venv", venv], "Creating build venv",
                         timeout=300):
            log_error("Could not create a Python venv. Install meson+ninja or "
                      "python3-venv, or place a harfbuzz static archive in "
                      "MORPH_STATIC_LIBDIRS.")
            return None
        pip = os.path.join(venv, scripts, "pip" + (".exe" if os.name == "nt" else ""))
        if not self._run([pip, "install", "--quiet", "meson", "ninja"],
                         "Installing meson+ninja", timeout=600):
            log_error("pip install meson ninja failed - check network access.")
            return None
        return meson_bin

    def _meson_opts(self, src: str, wanted: dict[str, str]) -> list[str]:
        opts_file = os.path.join(src, "meson_options.txt")
        available: set[str] = set()
        if os.path.isfile(opts_file):
            try:
                with open(opts_file) as f:
                    for m in re.finditer(r"option\s*\(\s*'([a-zA-Z0-9_]+)'", f.read()):
                        available.add(m.group(1))
            except OSError:
                available = set()
        out = []
        for name, value in wanted.items():
            if not available or name in available:
                out.append(f"-D{name}={value}")
        return out

    def _build_harfbuzz(self, prefix: str) -> bool:
        meson = self._meson()
        if not meson:
            return False
        tarball = self._source_tarball("harfbuzz")
        src = self._extract("harfbuzz", tarball)
        build = os.path.join(self.cache, "build", "harfbuzz-build")
        self._reset_dir(build)
        opts = self._meson_opts(src, _HB_WANTED_OPTS)
        extra = ["-DHB_MINI", "-ffunction-sections", "-fdata-sections"]
        c_args = extra + self._freetype_include_args()
        cpp_args = extra + self._freetype_include_args()
        if self.lto_ok:
            opts.append("-Db_lto=true")
        opts.append(f"-Dc_args={' '.join(c_args)}")
        opts.append(f"-Dcpp_args={' '.join(cpp_args)}")
        args = [
            meson, "setup", build, src,
            "--buildtype=minsize",
            "--default-library=static",
            f"--prefix={prefix}",
            *opts,
        ]
        env = os.environ.copy()
        env["CC"] = self.cc
        env["CXX"] = self.cxx
        ft_prefix = self.prefix.get("freetype")
        if ft_prefix:
            pc_dirs = self._pkgconfig_dirs(ft_prefix)
            existing = env.get("PKG_CONFIG_PATH", "")
            if pc_dirs:
                env["PKG_CONFIG_PATH"] = os.pathsep.join(
                    pc_dirs + ([existing] if existing else [])
                )
        if not self._run(args, "Configuring harfbuzz", env=env):
            return False
        if not self._run([meson, "compile", "-C", build], "Compiling harfbuzz", env=env):
            return False
        return self._run([meson, "install", "-C", build], "Installing harfbuzz", env=env)
