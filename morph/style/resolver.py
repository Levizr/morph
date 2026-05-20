from morph.style.sheet import StyleSheet
from morph.dom.tree import DOMTree
from morph.dom.node import ElementNode


class StyleResolver:
    """Applies stylesheet rules onto DOM nodes (computed_style)."""

    def __init__(self, sheet: StyleSheet):
        self.sheet = sheet

    def resolve(self, tree: DOMTree) -> None:
        for node in tree.walk():
            if isinstance(node, ElementNode):
                self._apply(node)

    def _apply(self, node: ElementNode) -> None:
        # TODO: selector matching + cascade + specificity
        # inline styles override sheet rules
        inline = self._parse_inline(node.attrs.get("style", ""))
        node.computed_style = inline

    def _parse_inline(self, style_str: str) -> dict[str, str]:
        result = {}
        for decl in style_str.split(";"):
            decl = decl.strip()
            if ":" in decl:
                prop, _, val = decl.partition(":")
                result[prop.strip()] = val.strip()
        return result
