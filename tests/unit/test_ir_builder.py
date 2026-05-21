from morph.ir.builder import IRBuilder
from morph.style.tailwind import TailwindResolver


def test_empty_dom_returns_empty_ir():
    ir = IRBuilder().build({}, {}, TailwindResolver(project_root="."))
    assert ir == []
