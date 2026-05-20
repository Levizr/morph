"""Colored terminal output for [morph] prefix messages."""

_RESET  = "\033[0m"
_GREEN  = "\033[32m"
_RED    = "\033[31m"
_YELLOW = "\033[33m"
_CYAN   = "\033[36m"
_DIM    = "\033[2m"


def log_info(msg: str)    -> None: print(f"{_CYAN}[morph]{_RESET} {msg}")
def log_success(msg: str) -> None: print(f"{_GREEN}[morph]{_RESET} {msg}")
def log_warn(msg: str)    -> None: print(f"{_YELLOW}[morph]{_RESET} {msg}")
def log_error(msg: str)   -> None: print(f"{_RED}[morph] Error:{_RESET} {msg}")
def log_dim(msg: str)     -> None: print(f"{_DIM}[morph]{_RESET} {_DIM}{msg}{_RESET}")
