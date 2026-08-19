"""Single source of truth for what the morph runtime actually supports.

All tables are imported from the existing registries (IR builder, style
parser) so the linter can never drift from the compiler.
"""

from __future__ import annotations

from morph.ir.builder import _UA_DEFAULTS, _CSS_TO_IR
from morph.style.properties import KNOWN_PROPERTIES

# ── Elements ─────────────────────────────────────────────────────────
SUPPORTED_TAGS: frozenset[str] = frozenset(_UA_DEFAULTS) | {"morph-window"}

# Tags the runtime registers but does not fully implement yet.
STUB_TAGS: frozenset[str] = frozenset({"input", "select", "textarea"})

# ── Props ────────────────────────────────────────────────────────────
GLOBAL_PROPS: frozenset[str] = frozenset({"className", "class", "id", "style", "key"})

EVENT_PROPS: frozenset[str] = frozenset({
    "onClick", "onDoubleClick",
    "onMouseDown", "onMouseUp", "onMouseEnter", "onMouseLeave",
    "onKeyUp", "onKeyDown",
})

MORPH_ACTIONS: frozenset[str] = frozenset({
    "morph-open", "morph-close", "morph-navigate",
})

WINDOW_PROPS: frozenset[str] = frozenset({
    "title", "width", "height",
    "minWidth", "maxWidth", "minHeight", "maxHeight",
})

WINDOW_SIZE_PROPS: frozenset[str] = frozenset({
    "width", "height", "minWidth", "maxWidth", "minHeight", "maxHeight",
})

# Per-element props on top of the global ones.
TAG_PROPS: dict[str, frozenset[str]] = {
    "img":          frozenset({"src", "alt", "width", "height"}),
    "a":            frozenset({"href", "target"}),
    "morph-window": WINDOW_PROPS,
}

WINDOW_CONFIG_KEYS: frozenset[str] = WINDOW_PROPS

# ── CSS ──────────────────────────────────────────────────────────────
# Inline style keys and .css declarations the pipeline understands.
# Shorthands parsed manually by IRBuilder on top of _CSS_TO_IR.
CSS_PROPERTIES: frozenset[str] = (
    frozenset(KNOWN_PROPERTIES)
    | frozenset(_CSS_TO_IR)
    | frozenset({
        "border", "flex", "transition",
        "transition-duration", "transition-timing-function",
        "animation",
    })
)

# Valid values for enum-like properties (warning-level check).
CSS_VALUE_SETS: dict[str, frozenset[str]] = {
    "display":          frozenset({"block", "inline", "inline-block", "flex", "hidden", "none"}),
    "flex-direction":   frozenset({"row", "column", "row-reverse", "column-reverse"}),
    "position":         frozenset({"static", "relative", "absolute", "fixed"}),
    "text-align":       frozenset({"left", "right", "center", "justify"}),
    "overflow":         frozenset({"visible", "hidden", "scroll", "auto"}),
    "cursor":           frozenset({"default", "pointer", "text"}),
    "box-sizing":       frozenset({"content-box", "border-box"}),
    "flex-wrap":        frozenset({"nowrap", "wrap", "wrap-reverse"}),
    "border-style":     frozenset({"none", "solid", "dashed", "dotted", "double",
                                   "groove", "ridge", "inset", "outset", "hidden"}),
    "font-weight":      frozenset({"normal", "bold", "lighter", "bolder"}),
}

# ── JS surface ───────────────────────────────────────────────────────
MORPH_BUILTINS: frozenset[str] = frozenset({
    "CSS", "morphState", "morphEffect", "morphLog", "morphEmit",
})

_CPP_EXTS = (".cpp", ".cc", ".cxx", ".c++", ".h", ".hpp", ".hxx")
CSS_EXTS = (".css",)


def import_kind(source: str) -> str:
    """Classify an import source: css | cpp | morph | url | other."""
    if source == "morph":
        return "morph"
    if source.startswith("http://") or source.startswith("https://"):
        return "url"
    if source.lower().endswith(_CPP_EXTS):
        return "cpp"
    if source.lower().endswith(CSS_EXTS):
        return "css"
    return "other"


# ── Rules metadata ───────────────────────────────────────────────────
# code → human description (used by `morph check` help output).
RULE_DESCRIPTIONS: dict[str, str] = {
    "mx-export":          "file must export exactly one default component",
    "mx-component-name":  "default export must be a named function",
    "mx-window-conflict": "use either <morph-window> or windowConfig, not both",
    "mx-window-missing":  "component must render a <morph-window> root or export windowConfig",
    "mx-windowconfig-key":  "unknown windowConfig key",
    "mx-windowconfig-type": "windowConfig value has the wrong type",
    "mx-window-prop":       "unknown prop on <morph-window>",
    "mx-window-prop-type":  "window size prop must be a number",
    "mx-tag":             "unknown HTML element",
    "mx-tag-stub":        "element is not fully implemented in the runtime",
    "mx-prop":            "prop is not supported on this element",
    "mx-img-src":         "<img> requires a src attribute",
    "mx-event-value":     "event handler must be a function",
    "mx-morph-action":    "morph-open/-close/-navigate value must be a string",
    "mx-key-misuse":      "key prop is only meaningful inside a .map() list",
    "mx-dup-class":       "use className or class, not both",
    "mx-style-prop":      "unsupported CSS property in inline style",
    "mx-style-value":     "invalid value for CSS property",
    "mx-tailwind-class":  "unknown Tailwind utility class",
    "mx-css-prop":        "unsupported CSS property in .css file",
    "mx-css-file-missing": "imported CSS file not found",
    "mx-import-morph":    "unknown import from 'morph'",
    "mx-import-missing":  "imported file not found",
    "mx-import-type":     "unsupported import kind",
    "mx-state-pattern":   "morphState must be destructured as [getter, setter]",
    "mx-effect-cb":       "morphEffect first argument must be a function",
    "mx-effect-deps":     "morphEffect dependency is not a state variable",
    "mx-transpile":       "JS cannot be compiled to C++ (expressions, events, effects, functions, globals)",
    "mx-list-key":        ".map() list item should have a key prop",
    "mx-js-global":       "browser/JS global not available in the native runtime",
    "mx-js-member":       "member access on unsupported JS builtin (Math/JSON/Date/console)",
    "mx-js-method":       "method not implemented by the native runtime type",
    "mx-js-op":           "operator not supported by the C++ translator",
    "mx-js-syntax":       "JS construct not supported by the C++ translator",
}

DEFAULT_SEVERITIES: dict[str, str] = {
    "mx-export":          "error",
    "mx-component-name":  "error",
    "mx-window-conflict": "error",
    "mx-window-missing":  "error",
    "mx-windowconfig-key":  "error",
    "mx-windowconfig-type": "error",
    "mx-window-prop":       "error",
    "mx-window-prop-type":  "error",
    "mx-tag":             "error",
    "mx-tag-stub":        "warning",
    "mx-prop":            "error",
    "mx-img-src":         "error",
    "mx-event-value":     "error",
    "mx-morph-action":    "error",
    "mx-key-misuse":      "warning",
    "mx-dup-class":       "warning",
    "mx-style-prop":      "error",
    "mx-style-value":     "warning",
    "mx-tailwind-class":  "warning",
    "mx-css-prop":        "warning",
    "mx-css-file-missing": "error",
    "mx-import-morph":    "error",
    "mx-import-missing":  "error",
    "mx-import-type":     "warning",
    "mx-state-pattern":   "error",
    "mx-effect-cb":       "error",
    "mx-effect-deps":     "warning",
    "mx-transpile":       "error",
    "mx-list-key":        "warning",
    "mx-js-global":       "error",
    "mx-js-member":       "error",
    "mx-js-method":       "error",
    "mx-js-op":           "error",
    "mx-js-syntax":       "error",
}