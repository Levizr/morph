from __future__ import annotations
from tree_sitter import Language, Parser, Node
import tree_sitter_css as tscss


class CSSParser:
    """
    Parses CSS source (file or string) using tree-sitter-css.

    - ``parse_string`` / ``parse_file`` return ``{ "selector": { "property": "value" } }``
    - ``parse_sheet`` returns ``(rules, keyframes)`` where ``keyframes`` is
      ``{ "name": [(offset, {"property": "value"}), ...] }`` with offsets in
      ``0..1``.  ``from``/``to`` map to ``0.0``/``1.0``, multiple selectors in
      one block (``68%, 72%``) expand to separate entries, duplicate offsets
      are merged per-property (later blocks win), and declarations carrying
      ``!important`` are ignored (per the CSS spec).
    """

    def __init__(self):
        self._lang   = Language(tscss.language())
        self._parser = Parser(self._lang)

    def parse_file(self, path: str) -> dict:
        return self.parse_sheet(open(path, encoding="utf-8").read())[0]

    def parse_string(self, css: str) -> dict:
        return self.parse_sheet(css)[0]

    def parse_sheet(self, css: str) -> tuple[dict, dict]:
        """Parse a full stylesheet.  Returns (rules, keyframes)."""
        if not css.strip():
            return {}, {}
        tree = self._parser.parse(css.encode("utf-8"))
        root = tree.root_node
        return self._extract_rules(root), self._extract_keyframes(root)

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

    def _extract_keyframes(self, root: Node) -> dict:
        keyframes = {}
        for node in root.children:
            if node.type != "keyframes_statement":
                continue
            name = self._get_keyframes_name(node)
            if not name:
                continue
            for child in node.children:
                if child.type == "keyframe_block_list":
                    for block in child.children:
                        if block.type != "keyframe_block":
                            continue
                        offsets = self._get_keyframe_offsets(block)
                        decls = self._get_keyframe_declarations(block)
                        if not offsets or not decls:
                            continue
                        entry = keyframes.setdefault(name, [])
                        for off in offsets:
                            self._merge_keyframe(entry, off, decls)
        return keyframes

    def _get_keyframes_name(self, statement: Node) -> str:
        for child in statement.children:
            if child.type == "keyframes_name":
                return child.text.decode("utf-8").strip()
        return ""

    def _get_keyframe_offsets(self, block: Node) -> list[float]:
        offsets = []
        for child in block.children:
            if child.type == "integer_value":
                text = child.text.decode("utf-8").strip()
                if text.endswith("%"):
                    try:
                        offsets.append(float(text[:-1]) / 100.0)
                    except ValueError:
                        pass
                else:
                    # bare unitless number inside a keyframe selector — invalid, skip
                    continue
            elif child.type == "from":
                offsets.append(0.0)
            elif child.type == "to":
                offsets.append(1.0)
        return offsets

    def _get_keyframe_declarations(self, block: Node) -> dict:
        decls = {}
        for child in block.children:
            if child.type != "block":
                continue
            for item in child.children:
                if item.type != "declaration":
                    continue
                # `!important` in a keyframe is ignored per spec
                if self._has_important(item):
                    continue
                prop, val = self._parse_declaration(item)
                if prop and val:
                    decls[prop] = val
        return decls

    def _has_important(self, decl: Node) -> bool:
        for child in decl.children:
            if child.type == "important":
                return True
        return False

    def _merge_keyframe(self, entry: list, offset: float, decls: dict) -> None:
        """Merge declarations into a keyframe at ``offset``.

        When a keyframe is defined multiple times (e.g. two ``50%`` blocks),
        all values are considered: properties from each block are kept, with
        later blocks overriding per-property.
        """
        for existing in entry:
            if abs(existing[0] - offset) < 1e-9:
                existing[1].update(decls)
                return
        entry.append([offset, dict(decls)])

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
