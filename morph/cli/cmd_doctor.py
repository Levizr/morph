import shutil
import subprocess
import platform
import os
import re

from morph.utils.logger import (
    log_info, log_success, log_warn, log_error, log_dim, log_step,
    log_header, log_banner, log_bullet, log_key, log_muted,
    _BOLD, _GREEN, _RED, _YELLOW, _CYAN, _DIM, _RESET,
)


def run(args) -> None:
    verbose = getattr(args, "verbose", False)

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

    # ── Core Dependencies ──────────────────────────────────
    log_step("Core Dependencies")

    ok, ver = _check_version("python3", ["--version"])
    _print_status("Python 3.10+", ok, detail=ver)
    if not ok:
        all_ok = False
    elif not _check_min_version(ver, (3, 10)):
        log_muted(f"    → Found {ver}, but {_YELLOW}3.10+ required{_RESET}")
        all_ok = False

    ok, ver = _check_version("g++-14", ["--version"])
    if not ok:
        ok, ver = _check_version("g++", ["--version"])
    _print_status("g++ (C++23)", ok, detail=ver)
    if not ok:
        all_ok = False
    elif not _check_gpp_version(ver, 14):
        log_muted(f"    → Found {ver}, but {_YELLOW}g++ 14+ required (C++23){_RESET}")
        all_ok = False

    ok, ver = _check_version("cmake", ["--version"])
    _print_status("cmake", ok, detail=ver)

    ok, ver = _check_version("make", ["--version"])
    _print_status("make", ok, detail=ver)

    ok, ver = _check_version("pkg-config", ["--version"])
    _print_status("pkg-config", ok, detail=ver)

    # ── Graphics & Windowing ──────────────────────────────
    log_step("Graphics & Windowing")

    ok = _check_lib("glfw3")
    _print_status("GLFW", ok)
    if not ok:
        log_muted(f"    → Install: {_YELLOW}sudo apt install libglfw3-dev{_RESET} (Linux)")
        log_muted(f"    →          {_YELLOW}brew install glfw{_RESET} (macOS)")
        all_ok = False
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "glfw3"])
        if ok2:
            log_muted(f"    → Version: {ver}")

    ok = _check_lib("gl")
    _print_status("OpenGL", ok)
    if not ok:
        log_muted(f"    → Install: {_YELLOW}sudo apt install libgl-dev{_RESET} (Linux)")
        all_ok = False
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "gl"])
        if ok2:
            log_muted(f"    → Version: {ver}")

    ok = _check_lib("x11")
    _print_status("X11", ok)
    if not ok and system == "Linux":
        log_muted(f"    → Install: {_YELLOW}sudo apt install libx11-dev{_RESET} (Linux)")
        all_ok = False
    elif verbose and ok:
        log_muted("    → Usually provided by libx11-dev")

    # ── Text Rendering ─────────────────────────────────────
    log_step("Text Rendering")

    ok = _check_lib("freetype2")
    _print_status("FreeType", ok)
    if not ok:
        log_muted(f"    → Install: {_YELLOW}sudo apt install libfreetype-dev{_RESET} (Linux)")
        log_muted(f"    →          {_YELLOW}brew install freetype{_RESET} (macOS)")
        all_ok = False
    elif verbose:
        ok2, ver = _check_version("pkg-config", ["--modversion", "freetype2"])
        if ok2:
            log_muted(f"    → Version: {ver}")
            # Also check for FreeType headers
            inc = subprocess.run(
                ["pkg-config", "--cflags", "freetype2"],
                capture_output=True, text=True, timeout=5
            ).stdout.strip()
            if inc:
                log_muted(f"    → Headers: {inc}")

    # ── Bundled Runtime ────────────────────────────────────
    log_step("Bundled Runtime")
    rt_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), "runtime")
    vendor_dir = os.path.join(rt_dir, "vendor")

    glad_c = os.path.join(vendor_dir, "glad", "glad.c")
    stb_c = os.path.join(vendor_dir, "stb_image.c")

    glad_ok = os.path.exists(glad_c)
    stb_ok = os.path.exists(stb_c)

    _print_status("glad (OpenGL loader)", glad_ok)
    _print_status("stb_image (image I/O)", stb_ok)
    if not glad_ok or not stb_ok:
        log_error("Bundled vendor files missing — reinstall the package")
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
            log_dim("  Install build tools: sudo apt install cmake g++")

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
    else:
        log_banner(f"{_RED}{_BOLD}Some dependencies missing{_RESET}")
        log_warn("Fix the issues above, then run `morph doctor` again")
        print()
        _print_system_fixes(system, ok)


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
    result = subprocess.run(
        ["pkg-config", "--exists", pkg],
        capture_output=True, timeout=5,
    )
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


def _check_gpp_version(ver_str: str, minimum: int) -> bool:
    match = re.search(r"(\d+)\.", ver_str)
    if match:
        major = int(match.group(1))
        return major >= minimum
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


def _print_system_fixes(system: str, ok: bool = True) -> None:
    if system == "Linux":
        print(f"  {_BOLD}Ubuntu/Debian:{_RESET}")
        print(f"    sudo apt update")
        print(f"    sudo apt install g++-14 cmake make pkg-config")
        print(f"    sudo apt install libglfw3-dev libgl-dev libgl1-mesa-dev")
        print(f"    sudo apt install libfreetype-dev libx11-dev")
        print()
        print(f"  {_BOLD}Fedora:{_RESET}")
        print(f"    sudo dnf install gcc-c++ cmake make pkgconfig")
        print(f"    sudo dnf install glfw-devel libglvnd-devel")
        print(f"    sudo dnf install freetype-devel libX11-devel")
        print()
        print(f"  {_BOLD}Arch:{_RESET}")
        print(f"    sudo pacman -S gcc cmake make pkg-config")
        print(f"    sudo pacman -S glfw-wayland libgl freetype2 libx11")
    elif system == "Darwin":
        print(f"  {_BOLD}macOS:{_RESET}")
        print(f"    brew install gcc cmake pkg-config")
        print(f"    brew install glfw freetype")
    elif system == "Windows":
        print(f"  {_BOLD}Windows:{_RESET}")
        print(f"    Install MSVC Build Tools or MinGW")
        print(f"    GLFW is bundled — no action needed")
        print(f"    Use vcpkg: vcpkg install freetype")
