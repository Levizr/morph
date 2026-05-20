from morph.pkg.manager import PackageManager


def run(args) -> None:
    pm = PackageManager()
    match args.pkg_command:
        case "add":
            pm.add(args.package)
        case "remove":
            pm.remove(args.package)
        case "install":
            pm.install_all()
        case "list":
            pkgs = pm.list_installed()
            if pkgs:
                for p in pkgs:
                    print(f"  {p}")
            else:
                print("[morph] No packages installed")
        case "search":
            results = pm.registry.search(args.query)
            for r in results:
                print(f"  {r['name']}@{r['version']} — {r.get('description','')}")
        case _:
            print("Usage: morph pkg [add|remove|install|list|search]")
