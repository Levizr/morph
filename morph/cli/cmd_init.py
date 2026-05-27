import os
import shutil
import json

from morph.utils.logger import (
    log_info, log_success, log_error, log_step, log_header,
    log_key, log_banner, log_muted, log_bullet,
    _BOLD, _GREEN, _CYAN, _DIM, _YELLOW, _RESET,
)

TEMPLATES = os.path.join(os.path.dirname(__file__), "../templates/default")


def run(args) -> None:
    name = args.name
    is_current_dir = name == "."

    # ── Interactive wizard ──────────────────────────────────
    if name is None:
        name = _interactive_wizard(args)
        if name is None:
            return
        is_current_dir = name == "."

    # ── Validate ────────────────────────────────────────────
    if is_current_dir:
        target = os.getcwd()
        project_name = os.path.basename(target)
        if os.path.exists(os.path.join(target, "morph.config.json")):
            log_error(f"morph.config.json already exists in {target}")
            return
    else:
        target = os.path.join(os.getcwd(), name)
        if os.path.exists(target):
            log_error(f"'{name}' already exists")
            return
        project_name = name

    log_banner(f"Creating project: {_BOLD}{project_name}")

    # ── Step 1: Scaffold ────────────────────────────────────
    log_step("Scaffolding project structure")

    if is_current_dir:
        for item in os.listdir(TEMPLATES):
            src = os.path.join(TEMPLATES, item)
            dst = os.path.join(target, item)
            if os.path.isdir(src):
                shutil.copytree(src, dst, dirs_exist_ok=True)
            else:
                shutil.copy2(src, dst)
    else:
        shutil.copytree(TEMPLATES, target)
    log_success("Project structure created")
    log_bullet(f"Target: {_BOLD}{target}")

    # ── Step 2: Configure ───────────────────────────────────
    log_step("Configuring project")

    cfg_path = os.path.join(target, "morph.config.json")
    with open(cfg_path) as f:
        cfg = json.load(f)

    cfg["name"] = project_name
    if args.title:
        cfg["window"]["title"] = args.title
    else:
        cfg["window"]["title"] = project_name
    if args.width:
        cfg["window"]["width"] = args.width
    if args.height:
        cfg["window"]["height"] = args.height
    if args.entry:
        cfg["entry"] = args.entry

    with open(cfg_path, "w") as f:
        json.dump(cfg, f, indent=2)

    log_success("morph.config.json written")
    log_key("Name",   cfg["name"])
    log_key("Entry",  cfg["entry"])
    log_key("Window", f"{cfg['window']['width']}×{cfg['window']['height']} — \"{cfg['window']['title']}\"")
    log_key("Output", cfg["output"])

    # ── Summary ─────────────────────────────────────────────
    print()
    log_success(f"Project {_BOLD}{_GREEN}{project_name}{_RESET} created!")

    if is_current_dir:
        print(f"\n  {_BOLD}Next:{_RESET}")
        print(f"    {_CYAN}▸{_RESET}  morph dev")
    else:
        print(f"\n  {_BOLD}Next:{_RESET}")
        print(f"    {_CYAN}▸{_RESET}  cd {name}")
        print(f"    {_CYAN}▸{_RESET}  morph dev")
    print()


def _interactive_wizard(args) -> str | None:
    """Next.js-style step-by-step project creation wizard."""

    log_banner("Create a new Morph project")

    # ── Step 1: Project name ────────────────────────────────
    log_step("Project name")
    print(f"  {_DIM}Use '.' for current directory, or enter a name:{_RESET}")

    try:
        raw = input(f"  {_CYAN}❯{_RESET} ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        log_error("Cancelled")
        return None

    if not raw:
        log_error("Name is required")
        return None

    name = raw
    is_current_dir = name == "."

    # ── Step 2: Window config ───────────────────────────────
    log_step("Window configuration")
    width = args.width
    height = args.height
    title = args.title

    if not args.yes:
        if width is None:
            try:
                w = input(f"  {_DIM}Width{_RESET}  {_CYAN}(800){_RESET} ❯ ").strip()
                width = int(w) if w else 800
            except (ValueError, EOFError, KeyboardInterrupt):
                width = 800

        if height is None:
            try:
                h = input(f"  {_DIM}Height{_RESET} {_CYAN}(600){_RESET} ❯ ").strip()
                height = int(h) if h else 600
            except (ValueError, EOFError, KeyboardInterrupt):
                height = 600

        if title is None:
            default_title = os.path.basename(os.getcwd()) if is_current_dir else name
            try:
                t = input(f"  {_DIM}Title{_RESET}  {_CYAN}({default_title}){_RESET} ❯ ").strip()
                title = t if t else default_title
            except (EOFError, KeyboardInterrupt):
                title = default_title

    # ── Step 3: Entry file ──────────────────────────────────
    entry = args.entry
    if entry is None and not args.yes:
        log_step("Entry file")
        try:
            e = input(f"  {_DIM}Entry .mx file{_RESET} {_CYAN}(src/App.mx){_RESET} ❯ ").strip()
            entry = e if e else "src/App.mx"
        except (EOFError, KeyboardInterrupt):
            entry = "src/App.mx"

    # ── Summary ─────────────────────────────────────────────
    log_step("Summary")
    shown_name = f"{_BOLD}{name}{_RESET}" if not is_current_dir else f"{_BOLD}.{_RESET} ({_DIM}current directory{_RESET})"
    print(f"  {_DIM}Project:{_RESET}  {shown_name}")
    print(f"  {_DIM}Window:{_RESET}   {width}×{height} — \"{title}\"")
    print(f"  {_DIM}Entry:{_RESET}    {entry}")

    if not args.yes:
        try:
            confirm = input(f"\n  {_CYAN}❯{_RESET} {_BOLD}Proceed?{_RESET} {_DIM}(Y/n){_RESET} ").strip().lower()
            if confirm in ("n", "no"):
                log_error("Cancelled")
                return None
        except (EOFError, KeyboardInterrupt):
            print()
            log_error("Cancelled")
            return None

    # Store on args so run() picks them up
    args.width = width
    args.height = height
    args.title = title
    args.entry = entry

    return name
