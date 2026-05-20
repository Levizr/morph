from morph.lexer.tokens import Token
from morph.style.sheet import StyleSheet


class CSSParser:
    """Parses a CSS token stream into a StyleSheet."""

    def parse(self, tokens: list[Token]) -> StyleSheet:
        # TODO: implement CSS parsing
        return StyleSheet()
