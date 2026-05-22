"""Morph UI Framework — Build native OpenGL UIs from .mx files."""

__version__ = "0.0.2"


def __getattr__(name):
    """Lazy imports so CLI (--help, doctor, etc.) works without tree-sitter."""
    import importlib
    _LAZY = {
        "MorphParser":     "morph.parser.morph_parser",
        "JSXWalker":       "morph.parser.jsx_walker",
        "CSSParser":       "morph.style.css_parser",
        "CSSFetcher":      "morph.style.css_fetcher",
        "TailwindResolver":"morph.style.tailwind",
        "IRBuilder":       "morph.ir.builder",
        "LayoutEngine":    "morph.layout.engine",
        "Emitter":         "morph.codegen.emitter",
        "load_config":     "morph.config.loader",
    }
    if name in _LAZY:
        module = importlib.import_module(_LAZY[name])
        return getattr(module, name)
    raise AttributeError(f"module 'morph' has no attribute {name!r}")


__all__ = [
    "MorphParser", "JSXWalker",
    "CSSParser", "CSSFetcher", "TailwindResolver",
    "IRBuilder",
    "LayoutEngine",
    "Emitter",
    "load_config",
]
