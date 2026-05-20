from morph.dom.tree import DOMTree
from morph.ir.node import IRNode, IRWindow, IRPage, IRViewport


class IRBuilder:
    """Converts a resolved DOMTree + JS intents into an IR tree."""

    def __init__(self, config=None):
        self.config = config
        self._counter = 0

    def build(self, dom: DOMTree, js_intents: list[dict]) -> list[IRWindow]:
        # TODO: walk DOM, build IRWindows + IRNodes
        return []

    def _next_id(self) -> str:
        self._counter += 1
        return f"node_{self._counter:04d}"
