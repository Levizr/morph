"""Central registry of all CSS properties — single source of truth.

Each entry maps a CSS property to:
  - feature: the MORPH_FEATURE_* flag (None = always included)
  - type: value type for parsing/validation
  - constraints: property inter-dependencies (enables/disables)
"""

CSS_REGISTRY = {
    # ── Base (always present) ──────────────────────────────
    "width":                  {"feature": None,  "type": "px"},
    "height":                 {"feature": None,  "type": "px"},
    "max-width":              {"feature": None,  "type": "px"},
    "background-color":       {"feature": None,  "type": "color"},
    "color":                  {"feature": None,  "type": "color"},
    "border-radius":          {"feature": "radius", "type": "px"},
    "font-size":              {"feature": None,  "type": "px"},
    "font-weight":            {"feature": "bold", "type": "keyword"},
    "text-align":             {"feature": None,  "type": "keyword"},
    "margin":                 {"feature": None,  "type": "spacing"},
    "margin-top":             {"feature": None,  "type": "px"},
    "margin-right":           {"feature": None,  "type": "px"},
    "margin-bottom":          {"feature": None,  "type": "px"},
    "margin-left":            {"feature": None,  "type": "px"},
    "padding":                {"feature": None,  "type": "spacing"},
    "padding-top":            {"feature": None,  "type": "px"},
    "padding-right":          {"feature": None,  "type": "px"},
    "padding-bottom":         {"feature": None,  "type": "px"},
    "padding-left":           {"feature": None,  "type": "px"},
    "overflow":               {"feature": None,  "type": "keyword"},
    "display":                {"feature": None,  "type": "keyword",
                               "values": {"flex": {"enables": ["flex-direction", "justify-content", "align-items", "gap", "flex-wrap", "flex-grow", "flex-shrink", "flex-basis"]}}},
    "box-sizing":             {"feature": None,  "type": "keyword"},
    "flex-direction":         {"feature": "flex", "type": "keyword"},
    "position":               {"feature": None,  "type": "keyword",
                               "values": {"absolute": {"enables": ["left", "right", "top", "bottom"]},
                                          "relative": {"enables": ["left", "right", "top", "bottom"]}}},

    # ── Flex ───────────────────────────────────────────────
    "justify-content":        {"feature": "flex", "type": "keyword"},
    "align-items":            {"feature": "flex", "type": "keyword"},
    "flex-wrap":              {"feature": "flex", "type": "keyword"},
    "flex-grow":              {"feature": "flex", "type": "number"},
    "flex-shrink":            {"feature": "flex", "type": "number"},
    "flex-basis":             {"feature": "flex", "type": "keyword"},
    "gap":                    {"feature": "flex", "type": "px"},

    # ── Position ───────────────────────────────────────────
    "left":                   {"feature": "position", "type": "px"},
    "right":                  {"feature": "position", "type": "px"},
    "top":                    {"feature": "position", "type": "px"},
    "bottom":                 {"feature": "position", "type": "px"},

    # ── Scrollbar ──────────────────────────────────────────
    "scrollbar-width":         {"feature": "scroll", "type": "px"},
    "scrollbar-track-color":   {"feature": "scroll", "type": "color"},
    "scrollbar-thumb-color":   {"feature": "scroll", "type": "color"},
    "scrollbar-border-radius": {"feature": "scroll", "type": "px"},

    # ── Interaction ────────────────────────────────────────
    "cursor":                 {"feature": "cursor", "type": "keyword"},

    # ── Border ─────────────────────────────────────────────
    "border-width":           {"feature": "border", "type": "px"},
    "border-color":           {"feature": "border", "type": "color"},
    "border-style":           {"feature": "border", "type": "keyword"},
    "border":                 {"feature": "border", "type": "shorthand"},
}


def feature_for_property(prop: str) -> str | None:
    """Return the feature flag for a CSS property, or None if always included."""
    entry = CSS_REGISTRY.get(prop)
    return entry["feature"] if entry else None


def properties_for_feature(feature: str) -> list[str]:
    """Return all CSS properties that belong to a given feature."""
    return [p for p, e in CSS_REGISTRY.items() if e["feature"] == feature]


def dependencies(prop: str, value: str) -> list[str]:
    """Return properties enabled by setting `prop` to `value`."""
    entry = CSS_REGISTRY.get(prop)
    if entry and "values" in entry:
        val_entry = entry["values"].get(value)
        if val_entry:
            return val_entry.get("enables", [])
    return []
