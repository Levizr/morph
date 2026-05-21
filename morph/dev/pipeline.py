from __future__ import annotations
from pathlib import Path

from morph.parser.morph_parser import MorphParser
from morph.parser.jsx_walker import JSXWalker
from morph.style.css_parser import CSSParser
from morph.style.css_fetcher import CSSFetcher
from morph.style.tailwind import TailwindResolver
from morph.ir.builder import IRBuilder
from morph.ir.serializer import IRSerializer
from morph.layout.engine import LayoutEngine
from morph.utils.logger import log_error

# ── Module-level singletons — dev mode mein reuse hote hain ──
_tw_resolver: TailwindResolver | None = None
_fetcher = CSSFetcher()
_css_parser = CSSParser()


def run(config) -> dict | None:
    """
    Full .mx source → IR dict pipeline.
    Returns serialized IR dict on success, None on failure.
    """
    global _tw_resolver
    if _tw_resolver is None:
        _tw_resolver = TailwindResolver(project_root=".")

    try:
        source = Path(config.entry).read_text(encoding="utf-8")

        # 1. Parse .mx → AST  (syntax errors yahan throw honge)
        ast = MorphParser().parse(source)

        # 2. Walk AST → components + imports
        walked = JSXWalker().walk(ast)

        # 3. CSS resolve karo
        css_rules: dict = {}
        for imp in walked.get("imports", []):
            if imp["type"] == "css_local":
                path = Path(config.entry).parent / imp["path"]
                if path.exists():
                    rules = _css_parser.parse_file(str(path))
                    css_rules.update(rules)

            elif imp["type"] == "css_url":
                css_text = _fetcher.fetch(imp["url"])
                if css_text:
                    rules = _css_parser.parse_string(css_text)
                    css_rules.update(rules)

        # 4. Build IR
        ir = IRBuilder(config).build(walked, css_rules, _tw_resolver)
        import json
        print(json.dumps(IRSerializer().to_dict(ir), indent=2))
        # 5. Layout
        LayoutEngine().compute(ir)

        # 6. Serialize → JSON-safe dict (dev socket ke liye)
        return IRSerializer().to_dict(ir)

    except Exception as e:
        log_error(str(e))
        return None
