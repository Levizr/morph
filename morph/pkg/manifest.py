import json
from dataclasses import dataclass, field


@dataclass
class PkgManifest:
    name: str
    version: str
    pkg_type: str = "morph-native"          # morph-native | cpp | compat
    js_entry: str = ""
    runtime_headers: list[str] = field(default_factory=list)
    github: str = ""
    description: str = ""

    @staticmethod
    def from_file(path: str) -> "PkgManifest":
        with open(path) as f:
            data = json.load(f)
        return PkgManifest(
            name=data["name"],
            version=data["version"],
            pkg_type=data.get("type", "morph-native"),
            js_entry=data.get("js_entry", ""),
            runtime_headers=data.get("runtime_headers", []),
            github=data.get("github", ""),
            description=data.get("description", ""),
        )
