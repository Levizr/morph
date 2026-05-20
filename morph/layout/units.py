"""Layout-level unit resolution (relative to computed parent sizes)."""


def resolve_percent(value: float, parent: float) -> float:
    return value * parent
