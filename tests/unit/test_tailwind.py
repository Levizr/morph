from morph.style.tailwind import TailwindResolver


def test_static_classes():
    tw = TailwindResolver()
    result = tw.resolve("bg-gray-900 rounded-lg p-4 flex")
    assert result.get("background-color") == "#111827"
    assert result.get("border-radius") == "8px"
    assert result.get("padding") == "16px"
    assert result.get("display") == "flex"


def test_arbitrary_color():
    tw = TailwindResolver()
    result = tw.resolve("bg-[#ff0000]")
    assert result.get("background-color") == "#ff0000"


def test_arbitrary_width():
    tw = TailwindResolver()
    result = tw.resolve("w-[200px]")
    assert result.get("width") == "200px"


def test_unknown_class_skipped():
    tw = TailwindResolver()
    result = tw.resolve("animate-spin")   # not in static map
    assert "animation" not in result      # silently skipped
