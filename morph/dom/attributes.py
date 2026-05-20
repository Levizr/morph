"""
Parses Morph-specific HTML attributes:
  morph-open, morph-close, morph-navigate
"""


def parse_morph_attrs(attrs: dict[str, str]) -> dict:
    """Extract morph action attrs from an element's attribute dict."""
    action = {}
    if "morph-open" in attrs:
        action = {"type": "open", "target": attrs["morph-open"]}
    elif "morph-close" in attrs:
        action = {"type": "close", "target": attrs["morph-close"]}
    elif "morph-navigate" in attrs:
        action = {"type": "navigate", "target": attrs["morph-navigate"]}
    return action
