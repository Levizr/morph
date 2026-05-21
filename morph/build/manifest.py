import json
import os
from dataclasses import dataclass


@dataclass
class BuildManifest:
    binary: str
    size_bytes: int
    mode: str
    headers_used: list[str]

    def save(self, path: str) -> None:
        with open(path, "w") as f:
            json.dump(self.__dict__, f, indent=2)
