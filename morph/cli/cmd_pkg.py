from morph.pkg.manager import PackageManager
from morph.utils.logger import log_error, log_info, log_success, log_step


def run(args) -> None:
    pm = PackageManager()
    match args.pkg_command:
        case "add":
            log_step(f"Installing package: {args.package}")
            pm.add(args.package)
            log_success(f"Package '{args.package}' installed")
        case "remove":
            pm.remove(args.package)
            log_success(f"Package '{args.package}' removed")
        case "install":
            log_step("Restoring packages")
            pm.install_all()
            log_success("All packages installed")
        case "list":
            pkgs = pm.list_installed()
            if pkgs:
                log_info("Installed packages:")
                for p in pkgs:
                    print(f"  • {p}")
            else:
                log_info("No packages installed")
        case "search":
            results = pm.registry.search(args.query)
            if results:
                log_info(f"Search results for '{args.query}':")
                for r in results:
                    desc = r.get("description", "")
                    print(f"  • {r['name']}@{r['version']}  — {desc}")
            else:
                log_info(f"No results for '{args.query}'")
        case _:
            print("Usage: morph pkg [add|remove|install|list|search]")
