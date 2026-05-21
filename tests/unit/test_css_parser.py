from morph.style.css_parser import CSSParser


def test_parse_empty():
    rules = CSSParser().parse_string("")
    assert rules == {}


def test_parse_simple_rule():
    css = """.button { background-color: #7c6af5; border-radius: 8px; }"""
    rules = CSSParser().parse_string(css)
    assert ".button" in rules
    assert rules[".button"]["background-color"] == "#7c6af5"


def test_parse_multiple_rules():
    css = """
.card { background-color: #1a1a2e; padding: 16px; }
.title { color: #ffffff; font-size: 24px; }
"""
    rules = CSSParser().parse_string(css)
    assert len(rules) == 2


def test_parse_string_vs_file(tmp_path):
    css = ".box { width: 100px; }"
    f   = tmp_path / "test.css"
    f.write_text(css)
    from_str  = CSSParser().parse_string(css)
    from_file = CSSParser().parse_file(str(f))
    assert from_str == from_file
