import json
import urllib.request

REGISTRY_BASE = "https://registry.morph-ui.dev"


class RegistryClient:
    def fetch(self, name: str, version: str = "latest") -> dict | None:
        url = f"{REGISTRY_BASE}/packages/{name}/{version}.json"
        try:
            with urllib.request.urlopen(url, timeout=10) as r:
                return json.loads(r.read())
        except Exception:
            return None

    def search(self, query: str) -> list[dict]:
        url = f"{REGISTRY_BASE}/search?q={query}"
        try:
            with urllib.request.urlopen(url, timeout=10) as r:
                return json.loads(r.read()).get("results", [])
        except Exception:
            return []
