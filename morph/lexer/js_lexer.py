from morph.lexer.tokens import Token, TokenType


class JSLexer:
    """Tokenizes raw JavaScript source into a flat token stream."""

    def tokenize(self, source: str) -> list[Token]:
        # TODO: implement JS tokenization
        tokens: list[Token] = []
        tokens.append(Token(TokenType.EOF, "", 0, 0))
        return tokens
