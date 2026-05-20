from __future__ import annotations
from morph.dom.node import DOMNode, ElementNode


class DOMTree:
    """Holds the root of the parsed DOM and provides traversal helpers."""

    def __init__(self, root: ElementNode | None = None):
        self.root = root

    def walk(self):
        """Depth-first generator over all nodes."""
        def _walk(node: DOMNode):
            yield node
            for child in node.children:
                yield from _walk(child)
        if self.root:
            yield from _walk(self.root)

    def __repr__(self):
        return f"DOMTree(root={self.root!r})"
