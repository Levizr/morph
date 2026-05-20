import shutil
import subprocess
import platform


def _check(label: str, cmd: str) -> bool:
    found = shutil.which(cmd) is not None
    icon  = "\033[32m✓\033[0m" if found else "\033[31m✗\033[0m"
    print(f"  {icon} {label}")
    return found


def _check_lib(label: str, pkg: str) -> bool:
    result = subprocess.run(
        ["pkg-config", "--exists", pkg],
        capture_output=True,
    )
    found = result.returncode == 0
    icon  = "\033[32m✓\033[0m" if found else "\033[31m✗\033[0m"
    print(f"  {icon} {label}")
    return found


def run() -> None:
    print("[morph] Checking system dependencies...\n")
    ok = {}
    ok["python"] = _check("Python 3.10+", "python3")
    ok["gpp"]    = _check("g++",          "g++")
    ok["node"]   = _check("Node.js",      "node")
    ok["glfw"]   = _check_lib("libglfw3", "glfw3")
    ok["gl"]     = _check_lib("libgl",    "gl")

    if all(ok.values()):
        print("\n[morph] \033[32mEverything looks good!\033[0m")
        return

    print("\n[morph] Fix missing deps:\n")
    system = platform.system()
    if system == "Linux":
        missing_pkgs = []
        if not ok["glfw"]: missing_pkgs.append("libglfw3-dev")
        if not ok["gl"]:   missing_pkgs.append("libgl-dev")
        if not ok["gpp"]:  missing_pkgs.append("g++")
        if missing_pkgs:
            print(f"  Ubuntu/Debian:")
            print(f"    sudo apt install {' '.join(missing_pkgs)}\n")
    elif system == "Darwin":
        if not ok["glfw"]:
            print("  macOS:\n    brew install glfw\n")
    elif system == "Windows":
        print("  Windows: GLFW is bundled — no action needed\n")
