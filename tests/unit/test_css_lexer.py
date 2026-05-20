from morph.lexer import CSSLexer
from morph.lexer.tokens import TokenType


def test_tokenize_returns_eof():
    tokens = CSSLexer().tokenize("div { color: red; }")
    assert tokens[-1].type == TokenType.EOF
