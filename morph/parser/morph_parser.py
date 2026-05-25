from __future__ import annotations
from tree_sitter import Language, Parser, Node
import tree_sitter_javascript as tsjs


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
    Parses .mx files (JSX syntax) using tree-sitter.
    Returns a tree-sitter Node (root) — JSXWalker walks it.
    """

    def __init__(self):
        self._lang   = Language(tsjs.language())
        self._parser = Parser(self._lang)

    def parse(self, source: str) -> Node:
        tree = self._parser.parse(source.encode("utf-8"))
        return tree.root_node
