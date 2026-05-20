import os
import shutil
import json


TEMPLATES = os.path.join(os.path.dirname(__file__), "../../templates/default")


def run(name: str) -> None:
    if os.path.exists(name):
        print(f"[morph] Error: '{name}' already exists")
        return

    shutil.copytree(TEMPLATES, name)

    cfg_path = os.path.join(name, "morph.config.json")
    with open(cfg_path) as f:
        cfg = json.load(f)
    cfg["name"] = name
    cfg["window"]["title"] = name
    with open(cfg_path, "w") as f:
        json.dump(cfg, f, indent=2)

    print(f"[morph] Created project \033[32m{name}\033[0m")
    print(f"  cd {name}")
    print(f"  morph dev")
