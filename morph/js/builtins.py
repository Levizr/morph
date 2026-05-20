"""
Morph built-in JS functions available in app.js.
These are resolved at AST-walk time, not executed.
"""

BUILTINS: dict[str, str] = {
    "morphLog":    "debug_print",
    "morphEmit":   "event_emit",
    "morphState":  "state_set",
}
