"""Convert CSS unit strings to float pixel values.

Sentinel
--------
`float("inf")` signals a value that cannot be resolved at IR-build time
and must be deferred to the layout engine (e.g. ``auto``, ``%``, ``vh``, ``vw``).
"""

import math

# Sentinel for values that need layout-time resolution
DEFERRED = math.inf


def to_px(value: str, parent_px: float = 0.0, root_px: float = 16.0) -> float:
    value = value.strip()
    if not value or value == "auto":
        return DEFERRED
    if value.endswith("px"):
        return float(value[:-2])
    if value.endswith("%"):
        if parent_px:
            return (float(value[:-1]) / 100.0) * parent_px
        return DEFERRED
    if value.endswith("rem"):
        return float(value[:-3]) * root_px
    if value.endswith("em"):
        return float(value[:-2]) * root_px
    if value.endswith("vh"):
        return DEFERRED
    if value.endswith("vw"):
        return DEFERRED
    if value.endswith("pt"):
        return float(value[:-2]) * 1.333333
    if value.endswith("pc"):
        return float(value[:-2]) * 16.0
    if value.endswith("cm"):
        return float(value[:-2]) * 37.795
    if value.endswith("mm"):
        return float(value[:-2]) * 3.7795
    if value.endswith("in"):
        return float(value[:-2]) * 96.0
    try:
        return float(value)
    except ValueError:
        pass
    return 0.0


def needs_layout(value: str) -> bool:
    """True if the CSS value string requires layout-time resolution."""
    if not value or value == "auto":
        return True
    v = value.strip()
    if v.endswith("%") or v.endswith("vh") or v.endswith("vw"):
        return True
    return False


def resolve(value: str, parent_px: float, viewport_px: float = 0.0,
             root_px: float = 16.0) -> float:
    """Resolve a CSS value at layout time when parent / viewport sizes are known."""
    v = value.strip()
    if not v or v == "auto":
        return 0.0  # auto resolves to 0 for non-margin/non-width contexts at this level
    if v.endswith("px"):
        return float(v[:-2])
    if v.endswith("%"):
        return (float(v[:-1]) / 100.0) * parent_px
    if v.endswith("vh"):
        return (float(v[:-2]) / 100.0) * viewport_px
    if v.endswith("vw"):
        return (float(v[:-2]) / 100.0) * viewport_px
    if v.endswith("rem"):
        return float(v[:-3]) * root_px
    if v.endswith("em"):
        return float(v[:-2]) * root_px
    if v.endswith("pt"):
        return float(v[:-2]) * 1.333333
    if v.endswith("pc"):
        return float(v[:-2]) * 16.0
    if v.endswith("cm"):
        return float(v[:-2]) * 37.795
    if v.endswith("mm"):
        return float(v[:-2]) * 3.7795
    if v.endswith("in"):
        return float(v[:-2]) * 96.0
    try:
        return float(v)
    except ValueError:
        return 0.0
