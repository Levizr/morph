from __future__ import annotations
from tree_sitter import Language, Parser, Node
import tree_sitter_css as tscss


class CSSParser:
    """
    Parses CSS source (file or string) using tree-sitter-css.
    Returns { "selector": { "property": "value" } }
    """

    def __init__(self):
        self._lang   = Language(tscss.language())
        self._parser = Parser(self._lang)

    def parse_file(self, path: str) -> dict:
        source = open(path, encoding="utf-8").read()
        return self.parse_string(source)

    def parse_string(self, css: str) -> dict:
        if not css.strip():
            return {}
        tree = self._parser.parse(css.encode("utf-8"))
        return self._extract_rules(tree.root_node)

    # ── Internal ──────────────────────────────────────────────

    def _extract_rules(self, root: Node) -> dict:
        rules = {}
        for node in root.children:
            if node.type == "rule_set":
                selector     = self._get_selector(node)
                declarations = self._get_declarations(node)
                if selector and declarations:
                    # merge if selector already seen
                    rules.setdefault(selector, {}).update(declarations)
        return rules

    def _get_selector(self, rule_node: Node) -> str:
        for child in rule_node.children:
            if child.type == "selectors":
                return child.text.decode("utf-8").strip()
        return ""

    def _get_declarations(self, rule_node: Node) -> dict:
        decls = {}
        for child in rule_node.children:
            if child.type == "block":
                for item in child.children:
                    if item.type == "declaration":
                        prop, val = self._parse_declaration(item)
                        if prop and val:
                            decls[prop] = val
        return decls

    def _parse_declaration(self, node: Node) -> tuple[str, str]:
        prop = ""
        val_parts = []
        for child in node.children:
            if child.type == "property_name":
                prop = child.text.decode("utf-8").strip()
            elif child.type not in (":", ";", "comment"):
                text = child.text.decode("utf-8").strip()
                if text:
                    val_parts.append(text)
        return prop, " ".join(val_parts)
