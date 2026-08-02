from __future__ import annotations
from tree_sitter import Language, Parser, Node, Point
import tree_sitter_typescript as tsts

from morph.parser.errors import MorphParseError


# Friendlier names for symbols tree-sitter reports on missing tokens.
_MISSING_FRIENDLY = {
    ")": "')'",
    "}": "'}'",
    "]": "']'",
    ";": "';'",
    ",": "','",
    "jsx_closing_element": "a closing JSX tag (e.g. </div>)",
    "jsx_closing_tag": "a closing JSX tag",
}


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

    def parse(self, source: str, file_path: str | None = None) -> Node:
        tree = self._parser.parse(source.encode("utf-8"))
        errors = collect_errors(tree.root_node)
        if errors:
            lines = source.split("\n")
            err = errors[0]
            start = err.start_point
            msg = self._format_error(err, lines)
            raise MorphParseError(msg, file_path=file_path,
                                  line=start.row + 1, col=start.column + 1,
                                  source_lines=lines)
        return tree.root_node

    @staticmethod
    def _format_error(node: Node, lines: list[str]) -> str:
        start = node.start_point
        row, col = start.row, start.column

        if node.is_missing:
            # tree-sitter reports the expected symbol as the missing node's type.
            expected = _MISSING_FRIENDLY.get(node.type, node.type)
            return f"Missing expected {expected}"

        # ERROR node: quote the text right under the caret.
        line = lines[row] if row < len(lines) else ""
        snippet = line[col:col + 40].strip()
        if snippet:
            return f"Unexpected token {snippet!r}"
        return "Unexpected syntax"
