from morph.lexer.tokens import Token, TokenType

class CSSLexer:
    """Tokenizes raw CSS source into a flat token stream."""
    
    def tokenize(self, source: str) -> list[Token]:
        # TODO: implement CSS tokenization
        tokens: list[Token] = []
        tokens.append(Token(TokenType.EOF, "", 0, 0))
        return tokens
