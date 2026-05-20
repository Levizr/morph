import json
import os
from morph.config.schema import MorphConfig


def load_config(path: str = "morph.config.json") -> MorphConfig:
    if not os.path.exists(path):
        raise FileNotFoundError(f"morph.config.json not found in {os.getcwd()}")
    with open(path) as f:
        data = json.load(f)
    return MorphConfig.from_dict(data)


def save_config(config: MorphConfig, path: str = "morph.config.json") -> None:
    with open(path, "w") as f:
        json.dump(config.to_dict(), f, indent=2)
