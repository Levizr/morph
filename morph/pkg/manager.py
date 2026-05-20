from morph.pkg.registry import RegistryClient
from morph.pkg.installer import Installer
from morph.pkg.manifest import PkgManifest
from morph.utils.logger import log_info, log_error


class PackageManager:
    def __init__(self, project_root: str = "."):
        self.root = project_root
        self.registry = RegistryClient()
        self.installer = Installer(project_root)

    def add(self, name: str, version: str = "latest") -> bool:
        log_info(f"Installing {name}...")
        meta = self.registry.fetch(name, version)
        if not meta:
            log_error(f"Package '{name}' not found")
            return False
        self.installer.install(meta)
        log_info(f"✓ {name}@{meta['version']} installed")
        return True

    def remove(self, name: str) -> bool:
        # TODO: remove from .morph/packages/ and config
        return False

    def install_all(self) -> None:
        """Restore all deps from morph.config.json."""
        # TODO: read config.dependencies, call add() for each
        pass

    def list_installed(self) -> list[str]:
        # TODO: scan .morph/packages/
        return []
