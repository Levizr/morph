from __future__ import annotations
from pathlib import Path

from morph.parser.morph_parser import MorphParser, collect_errors
from morph.parser.jsx_walker import JSXWalker
from morph.parser.errors import MorphParseError
from morph.style.css_parser import CSSParser
from morph.style.css_fetcher import CSSFetcher
from morph.style.tailwind import TailwindResolver
from morph.ir.builder import IRBuilder
from morph.ir.serializer import IRSerializer
from morph.layout.engine import LayoutEngine
from morph.utils.logger import log_error, log_parse_error

_tw_resolver: TailwindResolver | None = None
_fetcher = CSSFetcher()
_css_parser = CSSParser()


def run(config) -> dict | None:
    global _tw_resolver
    if _tw_resolver is None:
        _tw_resolver = TailwindResolver(project_root=".")

    try:
        source = Path(config.entry).read_text(encoding="utf-8")
        source_lines = source.split("\n")

        # ── 1. Parse .mx → AST ────────────────────────────────
        ast = MorphParser().parse(source)

        # ── Validate for unrecoverable errors ──────────────────
        for err_node in collect_errors(ast):
            if err_node.is_missing:
                raise MorphParseError(
                    "Missing required syntax element",
                    file_path=str(config.entry),
                    line=err_node.start_point[0] + 1,
                    col=err_node.start_point[1] + 1,
                    source_lines=source_lines,
                )
            if err_node.type == "ERROR":
                text = err_node.text.decode()
                # Recoverable errors are bare & in text content — no JSX syntax
                if "<" in text or "{" in text or "}" in text:
                    raise MorphParseError(
                        text.strip(),
                        file_path=str(config.entry),
                        line=err_node.start_point[0] + 1,
                        col=err_node.start_point[1] + 1,
                        source_lines=source_lines,
                    )

        # ── 2. Walk AST → components + imports ────────────────
        walked = JSXWalker().walk(ast)

        # ── 3. CSS resolve ────────────────────────────────────
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

        # ── 4. Build IR ───────────────────────────────────────
        ir = IRBuilder(config).build(walked, css_rules, _tw_resolver)

        # ── 5. Layout ─────────────────────────────────────────
        LayoutEngine().compute(ir)

        # ── 6. Serialize ──────────────────────────────────────
        return IRSerializer().to_dict(ir)

    except MorphParseError as e:
        log_parse_error(e)
        return None
    except Exception as e:
        log_error(f"{type(e).__name__}: {e}")
        return None
