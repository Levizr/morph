from morph.layout.engine import LayoutEngine


def test_empty_windows():
    LayoutEngine().compute([])   # should not raise
