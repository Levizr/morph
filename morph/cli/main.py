import argparse
import sys


def main():
    parser = argparse.ArgumentParser(
        prog="morph",
        description="Build native OpenGL UIs from HTML/CSS/JS",
    )
    sub = parser.add_subparsers(dest="command", metavar="command")

    p_init = sub.add_parser("init",  help="Scaffold a new project")
    p_init.add_argument("name", help="Project name")

    sub.add_parser("dev",   help="Start dev mode with live reload")
    sub.add_parser("build", help="Build optimized production binary")

    p_pkg = sub.add_parser("pkg", help="Package manager")
    pkg_sub = p_pkg.add_subparsers(dest="pkg_command")
    pkg_add = pkg_sub.add_parser("add")
    pkg_add.add_argument("package")
    pkg_sub.add_parser("install")
    pkg_sub.add_parser("list")
    pkg_rm = pkg_sub.add_parser("remove")
    pkg_rm.add_argument("package")
    pkg_search = pkg_sub.add_parser("search")
    pkg_search.add_argument("query")

    sub.add_parser("doctor", help="Check system dependencies")

    args = parser.parse_args()

    match args.command:
        case "init":
            from morph.cli.cmd_init import run
            run(args.name)
        case "dev":
            from morph.cli.cmd_dev import run
            run()
        case "build":
            from morph.cli.cmd_build import run
            run()
        case "pkg":
            from morph.cli.cmd_pkg import run
            run(args)
        case "doctor":
            from morph.cli.cmd_doctor import run
            run()
        case _:
            parser.print_help()
            sys.exit(0)
