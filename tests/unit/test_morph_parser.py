import pytest
from morph.parser.morph_parser import MorphParser
from morph.parser.errors import MorphParseError


def test_parse_valid_mx():
    source = """
export default function App() {
  return (
    <morph-window title="Test" width={800} height={600}>
      <div style={{ backgroundColor: '#000' }}>Hello</div>
    </morph-window>
  )
}
"""
    root = MorphParser().parse(source)
    assert root is not None


def test_parse_syntax_error():
    source = "export default function App( { return (<div) }"
    with pytest.raises(MorphParseError):
        MorphParser().parse(source)
