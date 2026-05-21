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

    ok, ver = _check_version("g++", ["--version"])
    _print_status("g++", ok, detail=ver)
    if not ok:
        all_ok = False
    elif not _check_gpp_version(ver, 11):
        log_muted(f"    → Found {ver}, but {_YELLOW}g++ 11+ recommended{_RESET}")

    ok, ver = _check_version("cmake", ["--version"])
    _print_status("cmake", ok, detail=ver)

    ok, ver = _check_version("make", ["--version"])
    _print_status("make", ok, detail=ver)

    ok, ver = _check_version("pkg-config", ["--version"])
    _print_status("pkg-config", ok, detail=ver)

    # ── OpenGL ──────────────────────────────────────────────
    log_step("OpenGL / Graphics")

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

    # ── Optional ────────────────────────────────────────────
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

    # ── Tailwind mode ──────────────────────────────────────
    tw_paths = _find_tailwind()
    if tw_paths:
        log_bullet(f"Tailwind found at: {tw_paths[0]}")
        if verbose:
            log_bullet(f"Candidates: {', '.join(tw_paths[:3])}")
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
        log_key("Output", cfg.get("output", "dist/"))
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
        # Take first line only
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
    # Check parent dirs too
    for d in [".", ".."]:
        p = os.path.join(d, "node_modules", "tailwindcss")
        if os.path.exists(p):
            candidates.append(p)
    return [c for c in candidates if os.path.exists(c)]


def _print_system_fixes(system: str, ok: bool = True) -> None:
    if system == "Linux":
        print(f"  {_BOLD}Ubuntu/Debian:{_RESET}")
        print(f"    sudo apt update")
        print(f"    sudo apt install g++ libglfw3-dev libgl-dev libgl1-mesa-dev pkg-config")
        print(f"    sudo apt install cmake make")
        print()
        print(f"  {_BOLD}Fedora:{_RESET}")
        print(f"    sudo dnf install gcc-c++ glfw-devel libglvnd-devel pkgconfig")
        print()
        print(f"  {_BOLD}Arch:{_RESET}")
        print(f"    sudo pacman -S gcc glfw-wayland libgl pkg-config")
    elif system == "Darwin":
        print(f"  {_BOLD}macOS:{_RESET}")
        print(f"    brew install gcc glfw cmake pkg-config")
    elif system == "Windows":
        print(f"  {_BOLD}Windows:{_RESET}")
        print(f"    Install MSVC Build Tools or MinGW")
        print(f"    GLFW is bundled — no action needed")
