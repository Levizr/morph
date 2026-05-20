from __future__ import annotations
from morph.dom.node import DOMNode, ElementNode
from morph.dom.tree import DOMTree


def find_by_id(tree: DOMTree, node_id: str) -> ElementNode | None:
    for node in tree.walk():
        if isinstance(node, ElementNode) and node.id == node_id:
            return node
    return None


def find_by_tag(tree: DOMTree, tag: str) -> list[ElementNode]:
    return [
        n for n in tree.walk()
        if isinstance(n, ElementNode) and n.tag == tag
    ]


def query_selector(tree: DOMTree, selector: str) -> ElementNode | None:
    # TODO: implement CSS selector matching
    return None
