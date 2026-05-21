from __future__ import annotations
from tree_sitter import Language, Parser, Node
import tree_sitter_javascript as tsjs
from morph.parser.errors import MorphParseError


class MorphParser:
    """
    Parses .mx files (JSX syntax) using tree-sitter.
    Returns a tree-sitter Node (root) — JSXWalker walks it.
    """

    def __init__(self):
        self._lang   = Language(tsjs.language())
        self._parser = Parser(self._lang)

    def parse(self, source: str) -> Node:
        """
        Parse .mx source, return root AST node.
        Raises MorphParseError if tree-sitter reports syntax errors.
        """
        tree = self._parser.parse(source.encode("utf-8"))
        root = tree.root_node

        if root.has_error:
            errors = self._collect_errors(root)
            raise MorphParseError(
                f"Syntax error(s) in .mx file:\n" +
                "\n".join(
                    f"  Line {e.start_point[0]+1}:{e.start_point[1]+1} — "
                    f"unexpected '{e.text.decode()}'"
                    for e in errors
                )
            )
        return root

    def _collect_errors(self, node: Node) -> list[Node]:
        """Recursively collect all ERROR nodes from the AST."""
        errors = []
        if node.type == "ERROR" or node.is_missing:
            errors.append(node)
        for child in node.children:
            errors.extend(self._collect_errors(child))
        return errors
