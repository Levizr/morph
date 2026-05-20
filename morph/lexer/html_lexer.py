from morph.lexer.tokens import Token, TokenType


class HTMLLexer:
    """Tokenizes raw HTML source into a flat token stream."""

    def tokenize(self, source: str) -> list[Token]:
        # TODO: implement HTML tokenization
        tokens: list[Token] = []
        tokens.append(Token(TokenType.EOF, "", 0, 0))
        return tokens
