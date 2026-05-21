import argparse
import sys

from morph.utils.logger import print_logo, print_features, log_info


def main():
    parser = argparse.ArgumentParser(
        prog="morph",
        description="Build native OpenGL Applicationsfrom .mx files",
        add_help=False,
    )
    parser.add_argument(
        "-h", "--help", action="store_true",
        help="Show this help message and exit"
    )
    parser.add_argument(
        "-v", "--version", action="store_true",
        help="Show version information"
    )
    sub = parser.add_subparsers(dest="command", metavar="command")

    p_init = sub.add_parser("init",  help="Scaffold a new .mx project")
    p_init.add_argument("name", nargs="?", default=None,
                        help="Project name (use '.' for current directory)")
    p_init.add_argument("--width",  type=int, default=None,
                        help="Window width (default: 800)")
    p_init.add_argument("--height", type=int, default=None,
                        help="Window height (default: 600)")
    p_init.add_argument("--title",  type=str, default=None,
                        help="Window title (default: project name)")
    p_init.add_argument("--entry",  type=str, default=None,
                        help="Entry .mx file (default: src/App.mx)")
    p_init.add_argument("--yes", "-y", action="store_true",
                        help="Skip interactive wizard, use defaults")

    p_dev = sub.add_parser("dev",   help="Start dev mode with live reload")
    p_dev.add_argument("--entry", type=str, default=None,
                       help="Override entry .mx file")

    p_build = sub.add_parser("build", help="Build optimized production binary")
    p_build.add_argument("--entry", type=str, default=None,
                         help="Override entry .mx file")
    p_build.add_argument("--output", type=str, default=None,
                         help="Output directory (default: dist/)")

    p_pkg = sub.add_parser("pkg", help="Package manager")
    pkg_sub = p_pkg.add_subparsers(dest="pkg_command")
    pkg_add = pkg_sub.add_parser("add");    pkg_add.add_argument("package")
    pkg_rm  = pkg_sub.add_parser("remove"); pkg_rm.add_argument("package")
    pkg_s   = pkg_sub.add_parser("search"); pkg_s.add_argument("query")
    pkg_sub.add_parser("install")
    pkg_sub.add_parser("list")

    p_doctor = sub.add_parser("doctor", help="Check system dependencies")
    p_doctor.add_argument("-v", "--verbose", action="store_true",
                          help="Show detailed version info")

    sub.add_parser("cache", help="Manage fetched CSS cache")

    args = parser.parse_args()

    if args.version:
        _show_version()
        return

    if args.help or args.command is None:
        _show_welcome(parser)
        return

    match args.command:
        case "init":
            from morph.cli.cmd_init import run
            run(args)
        case "dev":
            from morph.cli.cmd_dev import run
            run(args)
        case "build":
            from morph.cli.cmd_build import run
            run(args)
        case "pkg":
            from morph.cli.cmd_pkg import run
            run(args)
        case "doctor":
            from morph.cli.cmd_doctor import run
            run(args)
        case "cache":
            from morph.cli.cmd_cache import run
            run()
        case _:
            _show_welcome(parser)
            sys.exit(0)


def _show_version() -> None:
    from morph.utils.logger import _BOLD, _CYAN, _DIM, _RESET
    try:
        from importlib.metadata import version
        v = version("morph-ui")
    except Exception:
        v = "dev"
    print(f"{_BOLD}{_CYAN}morph{_RESET} {_DIM}v{v}{_RESET}")


def _show_welcome(parser: argparse.ArgumentParser) -> None:
    from morph.utils.logger import _BOLD, _GREEN, _YELLOW, _CYAN, _DIM, _RESET

    print_logo()

    print(f"  {_BOLD}USAGE{_RESET}")
    print(f"    {_BOLD}$ morph <command>{_RESET} [options]\n")

    print(f"  {_BOLD}COMMANDS{_RESET}")
    cmds = [
        ("init   [name]",  "Scaffold a new .mx project (interactive wizard)"),
        ("dev",            "Start dev mode with live hot reload"),
        ("build",          "Compile .mx → native OpenGL binary"),
        ("pkg   <add|rm|install|list|search>",  "Package manager"),
        ("doctor",         "Verify system dependencies"),
        ("cache",          "Clear fetched CSS cache"),
    ]
    for cmd, desc in cmds:
        print(f"    {_BOLD}{_GREEN}{cmd:<41}{_RESET}{_DIM}{desc}{_RESET}")

    print(f"\n  {_BOLD}OPTIONS{_RESET}")
    print(f"    {_BOLD}{_GREEN}-h, --help              {_RESET}{_DIM}Show this help{_RESET}")
    print(f"    {_BOLD}{_GREEN}-v, --version           {_RESET}{_DIM}Show version{_RESET}")

    print(f"\n  {_BOLD}FEATURES{_RESET}")
    print_features()

    print(f"  {_DIM}────────────────────────────────────────────────────────────{_RESET}")
    print(f"  {_DIM}Run {_RESET}{_YELLOW}morph <command> --help{_RESET}{_DIM} for command-specific options{_RESET}\n")
