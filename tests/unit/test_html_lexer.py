from morph.lexer import HTMLLexer
from morph.lexer.tokens import TokenType


def test_tokenize_returns_eof():
    tokens = HTMLLexer().tokenize("<div></div>")
    assert tokens[-1].type == TokenType.EOF


def test_empty_source():
    tokens = HTMLLexer().tokenize("")
    assert len(tokens) >= 1
