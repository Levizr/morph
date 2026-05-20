from morph.ir.builder import IRBuilder
from morph.dom.tree import DOMTree


def test_empty_dom_returns_empty_ir():
    ir = IRBuilder().build(DOMTree(), [])
    assert ir == []
