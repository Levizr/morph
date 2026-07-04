from __future__ import annotations
from tree_sitter import Language, Parser, Node, Point
import tree_sitter_typescript as tsts

from morph.parser.errors import MorphParseError


def collect_errors(node: Node) -> list[Node]:
    """Recursively collect all ERROR and MISSING nodes from the AST."""
    errors = []
    if node.type == "ERROR" or node.is_missing:
        errors.append(node)
    for child in node.children:
        errors.extend(collect_errors(child))
    return errors


class MorphParser:
    """
    Parses .mx files (TSX syntax) using tree-sitter.
    Raises MorphParseError on syntax errors.
    """

    def __init__(self):
        self._lang   = Language(tsts.language_tsx())
        self._parser = Parser(self._lang)

    def parse(self, source: str) -> Node:
        tree = self._parser.parse(source.encode("utf-8"))
        errors = collect_errors(tree.root_node)
        if errors:
            lines = source.split("\n")
            err = errors[0]
            start = err.start_point
            msg = self._format_error(err, lines)
            raise MorphParseError(msg, line=start.row + 1, col=start.column + 1,
                                  source_lines=lines)
        return tree.root_node

    @staticmethod
    def _format_error(node: Node, lines: list[str]) -> str:
        start = node.start_point
        snippet = lines[start.row][:80] if start.row < len(lines) else ""
        ctx = f" at line {start.row + 1}, col {start.column + 1}"
        if node.is_missing:
            return f"Missing expected token{ctx}: near {snippet!r}"
        return f"Unexpected token{ctx}: near {snippet!r}"
