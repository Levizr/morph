class DependencyResolver:
    """Resolves transitive dependencies from package manifests."""

    def resolve(self, direct: dict[str, str]) -> dict[str, str]:
        # TODO: topological sort + version conflict detection
        return direct
