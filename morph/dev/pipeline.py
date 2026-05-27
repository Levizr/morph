from __future__ import annotations
import time
from pathlib import Path

from morph.parser.morph_parser import MorphParser, collect_errors
from morph.parser.jsx_walker import JSXWalker
from morph.parser.errors import MorphParseError
from morph.style.css_parser import CSSParser
from morph.style.css_fetcher import CSSFetcher
from morph.style.tailwind import TailwindResolver
from morph.ir.builder import IRBuilder
from morph.ir.serializer import IRSerializer, _clean_inf
from morph.layout.engine import LayoutEngine
from morph.utils.logger import log_error, log_parse_error, log_dim, log_warn

_tw_resolver: TailwindResolver | None = None
_fetcher = CSSFetcher()
_css_parser = CSSParser()


def _fmt(secs: float) -> str:
    if secs < 0.5:
        return f"{secs*1000:.1f}ms"
    return f"{secs:.2f}s"


def run(config) -> dict | None:
    global _tw_resolver
    if _tw_resolver is None:
        _tw_resolver = TailwindResolver(project_root=".")

    ts = time.time()

    try:
        source = Path(config.entry).read_text(encoding="utf-8")
        source_lines = source.split("\n")

        t = time.time()
        ast = MorphParser().parse(source)
        log_dim(f"parsed  {Path(config.entry).name}  in {_fmt(time.time() - t)}")

        t = time.time()
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
                if "<" in text or "{" in text or "}" in text:
                    raise MorphParseError(
                        text.strip(),
                        file_path=str(config.entry),
                        line=err_node.start_point[0] + 1,
                        col=err_node.start_point[1] + 1,
                        source_lines=source_lines,
                    )
        log_dim(f"validated  in {_fmt(time.time() - t)}")

        t = time.time()
        walked = JSXWalker().walk(ast)
        log_dim(f"walked  AST  in {_fmt(time.time() - t)}")

        t = time.time()
        css_rules: dict = {}
        for imp in walked.get("imports", []):
            if imp["type"] == "css_local":
                path = Path(config.entry).parent / imp["path"]
                if path.exists():
                    rules = _css_parser.parse_file(str(path))
                    css_rules.update(rules)
                else:
                    log_warn(f"CSS file not found: {path} (imported from {config.entry})")
            elif imp["type"] == "css_url":
                css_text = _fetcher.fetch(imp["url"])
                if css_text:
                    rules = _css_parser.parse_string(css_text)
                    css_rules.update(rules)
        log_dim(f"resolved  CSS  in {_fmt(time.time() - t)}")

        t = time.time()
        ir = IRBuilder(config).build(walked, css_rules, _tw_resolver)
        log_dim(f"built  IR  in {_fmt(time.time() - t)}")

        t = time.time()
        LayoutEngine().compute(ir)
        log_dim(f"laid  out  in {_fmt(time.time() - t)}")

        t = time.time()
        result = _clean_inf(IRSerializer().to_dict(ir))
        log_dim(f"serialized  in {_fmt(time.time() - t)}")

        log_dim(f"total  {_fmt(time.time() - ts)}")
        return result

    except MorphParseError as e:
        log_parse_error(e)
        return None
    except Exception as e:
        log_error(f"{type(e).__name__}: {e}")
        return None
