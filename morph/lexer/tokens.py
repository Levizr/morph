from dataclasses import dataclass
from enum import Enum, auto


class TokenType(Enum):
    # HTML
    TAG_OPEN      = auto()
    TAG_CLOSE     = auto()
    TAG_SELF_CLOSE= auto()
    ATTRIBUTE     = auto()
    TEXT          = auto()
    COMMENT       = auto()
    DOCTYPE       = auto()

    # CSS
    SELECTOR      = auto()
    PROPERTY      = auto()
    VALUE         = auto()
    BRACE_OPEN    = auto()
    BRACE_CLOSE   = auto()
    COLON         = auto()
    SEMICOLON     = auto()
    AT_RULE       = auto()

    # JS
    KEYWORD       = auto()
    IDENTIFIER    = auto()
    STRING        = auto()
    NUMBER        = auto()
    OPERATOR      = auto()
    PUNCTUATION   = auto()

    # Shared
    WHITESPACE    = auto()
    NEWLINE       = auto()
    EOF           = auto()
    UNKNOWN       = auto()


@dataclass
class Token: 
    type: TokenType
    value: str
    line: int = 0
    col: int = 0

    def __repr__(self):
        return f"Token({self.type.name}, {self.value!r}, {self.line}:{self.col})"
