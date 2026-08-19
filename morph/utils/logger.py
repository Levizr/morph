"""Colored terminal output for [morph] prefix messages."""

from __future__ import annotations
from morph.parser.errors import MorphParseError

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

def _print_code_frame(file_path, line, col, source_lines, ctx=2) -> None:
    """Print the ┌──┐ code box with the offending line and caret."""
    loc = str(file_path) if file_path else ""
    if line:
        loc += f":{line}"
        if col:
            loc += f":{col}"

    if loc:
        print(f"  {_DIM}──>{_RESET} {_CYAN}{loc}{_RESET}")

    if source_lines and line > 0:
        start = max(0, line - 1 - ctx)
        end = min(len(source_lines), line + ctx)

        print(f"   {_DIM}──┐{_RESET}")
        for i in range(start, end):
            line_num = i + 1
            prefix = f"{_RED}{_BOLD}>{_RESET}" if line_num == line else " "
            gutter = f"{_DIM}{line_num:4}{_RESET}"
            code_line = source_lines[i].rstrip()
            print(f"   {prefix} {gutter} {_DIM}│{_RESET} {code_line}")
            if line_num == line and col:
                indent = max(0, col - 1)
                width = max(1, len(code_line) - indent)
                if width > 20:
                    width = 20
                pointer = " " * indent + _RED + "^" * width + _RESET
                print(f"     {_DIM}   │{_RESET} {pointer}")
        print(f"   {_DIM}──┘{_RESET}")


def log_parse_error(error: MorphParseError) -> None:
    """Display a parse error with file location, code context, and colors."""
    print(f"\n  {_RED}{_BOLD}error{_RESET}{_DIM}:{_RESET} {_BOLD}{str(error)}{_RESET}")
    _print_code_frame(error.file_path, error.line, error.col,
                      error.source_lines)


def log_lint_errors(errors: list) -> None:
    """Display lint findings with severity colors and code context.

    ``errors`` items are morph.checker.errors.LintError (any object with
    severity/code/message/suggestion/file_path/line/col/source_lines).
    """
    from morph.checker.errors import LintError

    errors = [e for e in errors if isinstance(e, LintError)]
    if not errors:
        return
    n_err = sum(1 for e in errors if e.severity == "error")
    n_warn = len(errors) - n_err

    summary = "lint"
    if n_err:
        summary += f" {_BOLD}{_RED}{n_err} error(s){_RESET}"
    if n_warn:
        summary += f" {_BOLD}{_YELLOW}{n_warn} warning(s){_RESET}"
    print(f"\n  {_CYAN}◆{_RESET}  {_BOLD}{summary}{_RESET}")

    for e in errors:
        color = _RED if e.severity == "error" else _YELLOW
        label = "error" if e.severity == "error" else "warning"
        print(f"\n  {color}{_BOLD}{label}{_RESET}{_DIM}:{_RESET} "
              f"{_BOLD}{e.code}{_RESET}{_DIM}:{_RESET} {e.message}")
        if e.suggestion:
            print(f"  {_DIM}  hint:{_RESET} {e.suggestion}")
        _print_code_frame(e.file_path, e.line, e.col, e.source_lines)
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
