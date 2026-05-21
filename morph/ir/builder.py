from __future__ import annotations

from morph.ir.node import IRNode, IRWindow, IRPage, IRViewport
from morph.style.tailwind import TailwindResolver


class IRBuilder:
    """Converts a resolved DOMTree + JS intents into an IR tree."""

    def __init__(self, config=None):
        self.config = config
        self._counter = 0

    def build(
        self,
        walked: dict,
        css_rules: dict,
        tw_resolver: TailwindResolver,
    ) -> list[IRWindow]:
        # TODO: walk walked tree, apply CSS + Tailwind, build IRWindows + IRNodes
        import json
        print(json.dumps(walked, indent=2))
        print(css_rules)
        print(tw_resolver)
        return []

    def _next_id(self) -> str:
        self._counter += 1
        return f"node_{self._counter:04d}"
