"""`morph check` — lint .mx files without compiling.

Exit code 0 = no lint errors, 1 = errors found (or parse failure).
"""

from __future__ import annotations

import sys
from pathlib import Path

from morph.config.loader import load_config
from morph.parser.morph_parser import MorphParser
from morph.parser.jsx_walker import JSXWalker
from morph.parser.errors import MorphParseError
from morph.utils.logger import log_lint_errors, log_parse_error, log_success, log_dim


def run(args=None) -> None:
    config = load_config()
    if args and getattr(args, "entry", None):
        config.entry = args.entry

    entry = Path(config.entry)
    if not entry.exists():
        from morph.utils.logger import log_error
        log_error(f"Entry file not found: {config.entry}")
        sys.exit(1)

    source = entry.read_text(encoding="utf-8")

    try:
        ast = MorphParser().parse(source, file_path=str(entry))
    except MorphParseError as e:
        log_parse_error(e)
        sys.exit(1)

    walked = JSXWalker().walk(ast)

    from morph.checker.checker import Checker
    errors = Checker().run(source, ast, walked, file_path=str(entry),
                           config=config)

    if not errors:
        log_success(f"No lint issues in {entry}")
        return

    log_lint_errors(errors)

    n_err = sum(1 for e in errors if e.severity == "error")
    if n_err:
        log_dim(f"{n_err} error(s) — run `morph dev` or `morph build` to see them in context")
        sys.exit(1)