"""Colored terminal output for [morph] prefix messages."""

_RESET   = "\033[0m"
_BOLD    = "\033[1m"
_DIM     = "\033[2m"
_ITALIC  = "\033[3m"

_BLACK   = "\033[30m"
_RED     = "\033[31m"
_GREEN   = "\033[32m"
_YELLOW  = "\033[33m"
_BLUE    = "\033[34m"
_MAGENTA = "\033[35m"
_CYAN    = "\033[36m"
_WHITE   = "\033[37m"

_BG_RED    = "\033[41m"
_BG_GREEN  = "\033[42m"
_BG_YELLOW = "\033[43m"
_BG_BLUE   = "\033[44m"
_BG_CYAN   = "\033[46m"


def _c(color: str, msg: str) -> str:
    return f"{color}{msg}{_RESET}"


def _wrap(prefix: str, color: str, msg: str) -> None:
    print(f"{_DIM}[{_RESET}{color}morph{_RESET}{_DIM}]{_RESET} {msg}")


def log_info(msg: str)        -> None: print(f"{_CYAN}  ◆{_RESET}  {msg}")
def log_success(msg: str)     -> None: print(f"{_GREEN}  ✓{_RESET}  {msg}")
def log_warn(msg: str)        -> None: print(f"{_YELLOW}  ⚠{_RESET}  {msg}")
def log_error(msg: str)       -> None: print(f"{_RED}  ✗{_RESET}  {msg}")
def log_dim(msg: str)         -> None: print(f"{_DIM}  · {msg}{_RESET}")
def log_step(msg: str)        -> None: print(f"\n{_BOLD}{_BLUE}  →{_RESET} {_BOLD}{msg}{_RESET}")
def log_header(msg: str)      -> None: print(f"\n{_BOLD}{_WHITE}{msg}{_RESET}")
def log_muted(msg: str)       -> None: print(f"{_DIM}    {msg}{_RESET}")
def log_bullet(msg: str)      -> None: print(f"{_CYAN}    •{_RESET} {msg}")
def log_key(label: str, val: str) -> None:
    print(f"    {_DIM}{label}:{_RESET} {_BOLD}{val}{_RESET}")
def log_banner(title: str) -> None:
    width = 60
    line = _DIM + "─" * width + _RESET
    print(f"\n  {line}")
    print(f"  {_BOLD}{_CYAN}  {title}{_RESET}")
    print(f"  {line}")


def print_features() -> None:
    features = [
        ("Build native UIs", "Compile .mx files → native OpenGL binary (no browser, no Electron)"),
        ("Hot Reload Dev",   "Edit code → instant window update via Unix socket IPC"),
        ("Tailwind CSS",     "500+ built-in utility classes + arbitrary values"),
        ("Custom C++ Nodes", "Drop into C++ for full OpenGL control"),
        ("Package System",   "Install community packages via `morph pkg add`"),
        ("Viewports",        "Embed raw OpenGL canvases inside your UI layout"),
    ]
    for title, desc in features:
        print(f"  {_BOLD}{_GREEN}▸{_RESET} {_BOLD}{title}{_RESET}")
        print(f"    {_DIM}{desc}{_RESET}")
    print()


def print_logo() -> None:
    logo = (
        f"  {_CYAN}{_BOLD}███╗   ███╗ ██████╗ ██████╗ ██████╗ ██╗  ██╗{_RESET}\n"
        f"  {_CYAN}{_BOLD}████╗ ████║██╔═══██╗██╔══██╗██╔══██╗██║  ██║{_RESET}\n"
        f"  {_CYAN}{_BOLD}██╔████╔██║██║   ██║██████╔╝██████╔╝███████║{_RESET}\n"
        f"  {_CYAN}{_BOLD}██║╚██╔╝██║██║   ██║██╔══██╗██╔═══╝ ██╔══██║{_RESET}\n"
        f"  {_CYAN}{_BOLD}██║ ╚═╝ ██║╚██████╔╝██║  ██║██║     ██║  ██║{_RESET}\n"
        f"  {_CYAN}{_BOLD}╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝     ╚═╝  ╚═╝{_RESET}\n"
    )
    print(logo)
    print(f"  {_DIM}Build native OpenGL Applications with HTML, CSS, and JavaScript{_RESET}")
    print(f"  {_DIM}No browser. No Electron. No WebView.{_RESET}\n")
