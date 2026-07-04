import json
import os
from morph.config.schema import MorphConfig
from morph.utils.logger import log_error


def load_config(path: str = "morph.config.json") -> MorphConfig:
    try:
        if not os.path.exists(path):
            raise FileNotFoundError(f"morph.config.json not found in {os.getcwd()}")
        with open(path) as f:
            data = json.load(f)
        return MorphConfig.from_dict(data)
    except Exception as e:
        log_error(f"Failed to load morph.config.json: {e}")
        exit(1)


def save_config(config: MorphConfig, path: str = "morph.config.json") -> None:
    with open(path, "w") as f:
        json.dump(config.to_dict(), f, indent=2)
