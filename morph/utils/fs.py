import os
import pathlib


def ensure_dir(path: str) -> None:
    os.makedirs(path, exist_ok=True)


def project_root() -> pathlib.Path:
    """Walk up from cwd until morph.config.json is found."""
    current = pathlib.Path.cwd()
    for parent in [current, *current.parents]:
        if (parent / "morph.config.json").exists():
            return parent
    return current


def read_text(path: str) -> str:
    return pathlib.Path(path).read_text(encoding="utf-8")


def write_text(path: str, content: str) -> None:
    pathlib.Path(path).write_text(content, encoding="utf-8")
