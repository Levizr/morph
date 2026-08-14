import shutil
import subprocess
import platform
import os
import re
import sys

from morph.utils.logger import (
    log_info, log_success, log_warn, log_error, log_dim, log_step,
    log_header, log_banner, log_bullet, log_key, log_muted,
    _BOLD, _GREEN, _RED, _YELLOW, _CYAN, _DIM, _RESET,
)


# ── Platform package maps ─────────────────────────────────────────────
# Package names per detected package manager (key = doctor check id).
_PKGS: dict[str, dict[str, str]] = {
    "g++": {
        "apt-get": "g++-14",
        "dnf": "gcc-c++",
        "pacman": "gcc",
        "zypper": "gcc-c++",
        "apk": "g++",
        "brew": "gcc",
        "choco": "mingw",
    },
    "cmake": {
        "apt-get": "cmake", "dnf": "cmake", "pacman": "cmake",
        "zypper": "cmake", "apk": "cmake", "brew": "cmake",
        "winget": "Kitware.CMake", "choco": "cmake",
    },
    "make": {
        "apt-get": "make", "dnf": "make", "pacman": "make",
        "zypper": "make", "apk": "make", "brew": "make",
        "choco": "make",
    },
    "pkg-config": {
        "apt-get": "pkg-config", "dnf": "pkgconf-pkg-config",
        "pacman": "pkg-config", "zypper": "pkg-config",
        "apk": "pkgconf", "brew": "pkg-config",
        "choco": "pkgconfiglite",
    },
    "glfw3": {
        "apt-get": "libglfw3-dev", "dnf": "glfw-devel",
        "pacman": "glfw-wayland", "zypper": "glfw-devel",
        "apk": "glfw-dev", "brew": "glfw",
        "choco": "glfw",
    },
    "gl": {
        "apt-get": "libgl-dev libgl1-mesa-dev", "dnf": "libglvnd-devel",
        "pacman": "libgl", "zypper": "Mesa-libGL-devel", "apk": "mesa-dev",
    },
    "x11": {
        "apt-get": "libx11-dev", "dnf": "libX11-devel",
        "pacman": "libx11", "zypper": "libX11-devel", "apk": "libx11-dev",
    },
    "freetype2": {
        "apt-get": "libfreetype-dev", "dnf": "freetype-devel",
        "pacman": "freetype2", "zypper": "freetype2-devel",
        "apk": "freetype-dev", "brew": "freetype",
        "choco": "freetype",
    },
    "harfbuzz": {
        "apt-get": "libharfbuzz-dev", "dnf": "harfbuzz-devel",
        "pacman": "harfbuzz", "zypper": "harfbuzz-devel",
        "apk": "harfbuzz-dev", "brew": "harfbuzz",
    },
}

_INSTALL_BASE: dict[str, list[str]] = {
    "apt-get": ["apt-get", "install", "-y"],
    "dnf":     ["dnf", "install", "-y"],
    "pacman":  ["pacman", "-S", "--noconfirm"],
    "zypper":  ["zypper", "install", "-y"],
    "apk":     ["apk", "add"],
    "brew":    ["brew", "install"],
    "winget":  ["winget", "install", "-e",
                "--accept-source-agreements", "--accept-package-agreements"],
    "choco":   ["choco", "install", "-y"],
}

_LABELS = {
    "g++": "g++ (C++23)", "cmake": "cmake", "make": "make",
    "pkg-config": "pkg-config", "glfw3": "GLFW", "gl": "OpenGL",
    "x11": "X11", "freetype2": "FreeType", "harfbuzz": "HarfBuzz",
}


def run(args) -> None:
    verbose = getattr(args, "verbose", False)
    auto_yes = getattr(args, "yes", False)

    log_banner("System Diagnostic")

    # ── OS Info ─────────────────────────────────────────────
    log_step("System Info")
    system = platform.system()
    release = platform.release()
    arch = platform.machine()
    print(f"  {_DIM}OS:{_RESET}           {system} {release} ({arch})")
    print(f"  {_DIM}Python:{_RESET}       {platform.python_version()} ({platform.python_implementation()})")
    print(f"  {_DIM}Host:{_RESET}         {platform.node()}")

    all_ok = True
    missing: list[str] = []

    # ── Core Dependencies ──────────────────────────────────
    log_step("Core Dependencies")

    ok, ver = _check_version("python3", ["--version"])
    _print_status("Python 3.10+", ok, detail=ver)
    if not ok:
        all_ok = False
    elif not _check_min_version(ver, (3, 10)):
        log_muted(f"    → Found {ver}, but {_YELLOW}3.10+ required{_RESET}")
        all_ok = False

    gpp = _gpp()
    if gpp is None:
        ok, ver, gpp_major = False, "", 0
    else:
        ok, ver = _check_version(gpp, ["--version"])
        gpp_major = _gpp_major(ver)
    _print_status("g++ (C++23)", ok, detail=ver)
    if not ok or gpp_major < 14:
        if ok:
            log_muted(f"    → Found {ver}, but {_YELLOW}g++ 14+ required (C++23){_RESET}")
        all_ok = False
        missing.append("g++")

    for exe, label in (("cmake", "cmake"), ("make", "make"), ("pkg-config", "pkg-config")):
        ok, ver = _check_version(exe, ["--version"])
        _print_status(label, ok, detail=ver)
        if not ok:
            all_ok = False
            missing.append(exe)

    # ── Graphics & Windowing ──────────────────────────────
    log_step("Graphics & Windowing")

    ok = system == "Darwin" or _check_lib("glfw3")
    _print_status("GLFW", ok)
    if not ok:
        all_ok = False
        missing.append("glfw3")
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "glfw3"])
        if ok2:
            log_muted(f"    → Version: {ver}")

    ok = system == "Darwin" or _check_lib("gl")
    _print_status("OpenGL", ok)
    if not ok:
        all_ok = False
        missing.append("gl")
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "gl"])
        if ok2:
            log_muted(f"    → Version: {ver}")

    ok = system != "Linux" or _check_lib("x11")
    _print_status("X11", ok)
    if not ok:
        all_ok = False
        missing.append("x11")
    elif verbose and ok and system == "Linux":
        log_muted("    → Usually provided by libx11-dev")

    # ── Text Rendering ─────────────────────────────────────
    log_step("Text Rendering")

    ok = _check_lib("freetype2")
    _print_status("FreeType", ok)
    if not ok:
        all_ok = False
        missing.append("freetype2")
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "freetype2"])
        if ok2:
            log_muted(f"    → Version: {ver}")
            inc = subprocess.run(
                ["pkg-config", "--cflags", "freetype2"],
                capture_output=True, text=True, timeout=5
            ).stdout.strip()
            if inc:
                log_muted(f"    → Headers: {inc}")

    ok = _check_lib("harfbuzz")
    _print_status("HarfBuzz", ok)
    if not ok:
        all_ok = False
        missing.append("harfbuzz")
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "harfbuzz"])
        if ok2:
            log_muted(f"    → Version: {ver}")

    # ── Bundled Runtime ────────────────────────────────────
    log_step("Bundled Runtime")
    rt_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), "runtime")
    vendor_dir = os.path.join(rt_dir, "vendor")

    glad_c = os.path.join(vendor_dir, "glad", "glad.c")
    stb_h = os.path.join(vendor_dir, "stb_image.h")
    stb_c = os.path.join(vendor_dir, "stb_image.c")

    glad_ok = os.path.exists(glad_c)
    stb_ok = os.path.exists(stb_h)

    _print_status("glad (OpenGL loader)", glad_ok)
    _print_status("stb_image (image I/O)", stb_ok)
    if not glad_ok or not stb_ok:
        log_error("Bundled vendor files missing — reinstall the package")
        log_muted("    → Reinstall: pip install --force-reinstall morph")
        all_ok = False

    # ── Dev Runtime Binary ──────────────────────────────────
    from morph.dev import devrt as _devrt
    devrt_path = _devrt.get_devrt_path()
    if os.path.exists(devrt_path):
        log_success("Dev runtime binary found")
        if verbose:
            log_muted(f"    → {devrt_path}")
    else:
        log_warn("Dev runtime binary not found")
        log_dim("  Run `morph dev` to auto-build it")
        cmake_ok = shutil.which("cmake") is not None
        gpp_ok = shutil.which("g++") is not None
        if cmake_ok and gpp_ok:
            log_dim("  cmake and g++ available — will build on first `morph dev`")
        else:
            if system == "Windows":
                hint = "choco install mingw cmake make pkgconfiglite"
            elif system == "Darwin":
                hint = "brew install gcc cmake make pkg-config"
            else:
                hint = "sudo apt install cmake g++"
            log_dim(f"  Install build tools: {hint}")

    # ── Optional Tools ─────────────────────────────────────
    log_step("Optional Tools")

    ok, ver = _check_version("node", ["--version"])
    _print_status("Node.js", ok, detail=ver)

    ok, ver = _check_version("npm", ["--version"])
    _print_status("npm", ok, detail=ver)

    ok, ver = _check_version("tailwindcss", ["--version"])
    if not ok:
        ok2, ver2 = _check_version("npx", ["tailwindcss", "--version"])
        _print_status("Tailwind CSS", ok2, detail=ver2)
    else:
        _print_status("Tailwind CSS", ok, detail=ver)

    tw_paths = _find_tailwind()
    if tw_paths:
        if verbose:
            log_bullet(f"Tailwind found at: {tw_paths[0]}")
        log_success("Tailwind mode: full (all classes available)")
    else:
        log_dim("Tailwind mode: static (500 common classes)")
        log_dim("  Run `npm init -y && npm install tailwindcss` for full support")

    # ── Project check ──────────────────────────────────────
    log_step("Project Check")
    if os.path.exists("morph.config.json"):
        log_success("morph.config.json found")

        with open("morph.config.json") as f:
            import json
            cfg = json.load(f)
        log_key("Name",   cfg.get("name", "?"))
        log_key("Entry",  cfg.get("entry", "?"))
        win = cfg.get("window", {})
        log_key("Window", f"{win.get('width', '?')}×{win.get('height', '?')} — \"{win.get('title', '?')}\"")
        log_key("Output", cfg.get("output", ".morph/"))
        log_key("Deps",   str(len(cfg.get("dependencies", {}))))
        log_key("C++",    str(len(cfg.get("cpp_sources", []))))

        entry_path = cfg.get("entry", "src/App.mx")
        if os.path.exists(entry_path):
            log_success(f"Entry file {entry_path} exists")
        else:
            log_warn(f"Entry file {entry_path} not found ({_YELLOW}run `morph init`?{_RESET})")
    else:
        log_warn("No morph.config.json in current directory")
        log_dim("  Run `morph init` to create a project")

    # ── Result ──────────────────────────────────────────────
    print()
    if all_ok:
        log_banner(f"{_GREEN}{_BOLD}All critical dependencies met{_RESET}")
        log_success("Ready to build!")
        return

    log_banner(f"{_RED}{_BOLD}Some dependencies missing{_RESET}")

    pm = _detect_package_manager(system)
    fixable = [d for d in missing if _PKGS.get(d, {}).get(pm or "")]

    if pm and fixable:
        _offer_install(pm, fixable, auto_yes)
    else:
        log_dim("No automatic install is possible for the missing items on this system.")
        log_dim("Manual instructions for your platform are below.")

    print()
    _print_system_fixes(system)


# ── Auto-install ──────────────────────────────────────────────────────

def _offer_install(pm: str, deps: list[str], auto_yes: bool) -> None:
    """Offer to install missing packages via the detected package manager."""
    packages: list[str] = []
    for d in deps:
        pkgs = _PKGS[d].get(pm)
        if pkgs:
            packages.extend(pkgs.split())
    packages = list(dict.fromkeys(packages))
    cmd = _sudo_prefix() + _INSTALL_BASE[pm] + packages

    log_step("Auto-fix")
    print(f"  {_DIM}Package manager:{_RESET} {_BOLD}{pm}{_RESET}")
    for d in deps:
        print(f"    {_DIM}→{_RESET} {_LABELS.get(d, d)}  ({_CYAN}{_PKGS[d].get(pm)}{_RESET})")
    print()
    print(f"  {_BOLD}Command:{_RESET} {_CYAN}{' '.join(cmd)}{_RESET}")

    if not auto_yes:
        try:
            answer = input("  Install these now? [y/N] ").strip().lower() if sys.stdin.isatty() else "n"
        except EOFError:
            answer = "n"
    else:
        answer = "y"

    if answer not in ("y", "yes"):
        print(f"  {_DIM}Skipped — run the command above manually when ready.{_RESET}")
        return

    if pm == "apt-get":
        upd = _sudo_prefix() + ["apt-get", "update"]
        rc = _run_privileged(upd)
        if rc == 0:
            print(f"  {_GREEN}✓{_RESET} apt-get update")
        else:
            log_warn("apt-get update failed — continuing anyway")

    rc = _run_privileged(cmd)
    if rc is None:
        log_warn("Could not install automatically (sudo needs a password outside an interactive shell).")
        print(f"  Run this manually: {_CYAN}{' '.join(cmd)}{_RESET}")
        return
    if rc != 0:
        log_error(f"Install command failed (exit code {rc})")
        print(f"  Retry manually: {_CYAN}{' '.join(cmd)}{_RESET}")
        return

    log_success("Install finished")
    print()
    _verify_fixed(deps)


def _verify_fixed(deps: list[str]) -> None:
    log_step("Re-check")
    for d in deps:
        _print_status(_LABELS.get(d, d), _check_dep(d))


def _check_dep(dep: str) -> bool:
    if dep == "g++":
        gpp = _gpp()
        if gpp is None:
            return False
        ok, ver = _check_version(gpp, ["--version"])
        return ok and _gpp_major(ver) >= 14
    if dep in ("cmake", "make", "pkg-config"):
        return shutil.which(dep) is not None
    return _check_lib(dep)


def _detect_package_manager(system: str) -> str | None:
    if system == "Linux":
        for pm in ("apt-get", "dnf", "pacman", "zypper", "apk"):
            if shutil.which(pm):
                return pm
    elif system == "Darwin":
        return "brew" if shutil.which("brew") else None
    elif system == "Windows":
        for pm in ("winget", "choco", "scoop"):
            if shutil.which(pm):
                return pm
    return None


def _sudo_prefix() -> list[str]:
    if platform.system() == "Linux":
        try:
            if os.geteuid() == 0:
                return []
        except AttributeError:
            pass
        return ["sudo"]
    return []


def _run_privileged(cmd: list[str]) -> int | None:
    """Run a possibly-root command. Returns the exit code, or None if it can't
    be run non-interactively (sudo needs a password and there's no TTY)."""
    if cmd and cmd[0] == "sudo" and not sys.stdin.isatty():
        test = subprocess.run(["sudo", "-n", "true"], capture_output=True)
        if test.returncode != 0:
            return None
        cmd = ["sudo", "-n"] + cmd[1:]
    try:
        return subprocess.run(cmd).returncode
    except Exception as e:
        log_error(f"  Failed to run {' '.join(cmd)} — {e}")
        return None


def _gpp() -> str | None:
    if shutil.which("g++-14"):
        return "g++-14"
    if shutil.which("g++"):
        return "g++"
    return None


def _gpp_major(ver: str) -> int:
    m = re.search(r"(\d+)\.", ver)
    return int(m.group(1)) if m else 0


# ── Helpers ────────────────────────────────────────────────────

def _check_version(exe: str, args: list[str]) -> tuple[bool, str]:
    path = shutil.which(exe)
    if not path:
        return False, ""
    try:
        result = subprocess.run(
            [path] + args, capture_output=True, text=True, timeout=5
        )
        output = (result.stdout or result.stderr or "").strip()
        first = output.split("\n")[0].strip()
        return True, first
    except Exception:
        return True, "found"


def _check_lib(pkg: str) -> bool:
    pkg_config = shutil.which("pkg-config")
    if pkg_config is None:
        return False
    try:
        result = subprocess.run(
            [pkg_config, "--exists", pkg],
            capture_output=True, timeout=5,
        )
    except Exception:
        return False
    return result.returncode == 0


def _print_status(label: str, ok: bool, detail: str = "") -> None:
    icon = f"{_GREEN}✓{_RESET}" if ok else f"{_RED}✗{_RESET}"
    detail_str = f" {_DIM}({detail}){_RESET}" if detail and ok else ""
    print(f"  {icon} {label}{detail_str}")


def _check_min_version(ver_str: str, minimum: tuple) -> bool:
    match = re.search(r"(\d+)\.(\d+)", ver_str)
    if match:
        major, minor = int(match.group(1)), int(match.group(2))
        return (major, minor) >= minimum
    return False


def _find_tailwind() -> list[str]:
    candidates = [
        os.path.join(".", "node_modules", "tailwindcss"),
    ]
    for d in [".", ".."]:
        p = os.path.join(d, "node_modules", "tailwindcss")
        if os.path.exists(p):
            candidates.append(p)
    return [c for c in candidates if os.path.exists(c)]


def _print_system_fixes(system: str) -> None:
    print(f"  {_BOLD}Install commands for your platform:{_RESET}")
    if system == "Linux":
        print(f"  {_BOLD}Ubuntu/Debian:{_RESET}")
        print(f"    sudo apt update")
        print(f"    sudo apt install g++-14 cmake make pkg-config")
        print(f"    sudo apt install libglfw3-dev libgl-dev libgl1-mesa-dev")
        print(f"    sudo apt install libfreetype-dev libx11-dev libharfbuzz-dev")
        print()
        print(f"  {_BOLD}Fedora:{_RESET}")
        print(f"    sudo dnf install gcc-c++ cmake make pkgconfig")
        print(f"    sudo dnf install glfw-devel libglvnd-devel")
        print(f"    sudo dnf install freetype-devel libX11-devel harfbuzz-devel")
        print()
        print(f"  {_BOLD}Arch:{_RESET}")
        print(f"    sudo pacman -S gcc cmake make pkg-config")
        print(f"    sudo pacman -S glfw-wayland libgl freetype2 libx11 harfbuzz")
        print(f"  {_DIM}Tip: run `morph doctor` and answer [y] to auto-install.{_RESET}")
    elif system == "Darwin":
        print(f"  {_BOLD}macOS (Homebrew):{_RESET}")
        print(f"    brew install gcc cmake make pkg-config")
        print(f"    brew install glfw freetype harfbuzz")
        print(f"  {_DIM}Tip: run `morph doctor` and answer [y] to auto-install.{_RESET}")
    elif system == "Windows":
        print(f"  {_BOLD}Windows:{_RESET}")
        print(f"    Install MSVC Build Tools or MinGW (g++ with C++23 support)")
        print(f"    choco install mingw cmake make pkgconfiglite glfw freetype")
        print(f"    OR use vcpkg: vcpkg install freetype glfw3")
        print(f"  {_DIM}Tip: run `morph doctor` and answer [y] to auto-install via winget/choco.{_RESET}")
    else:
        print(f"  {_DIM}Unsupported platform — see morph docs for manual setup.{_RESET}")
