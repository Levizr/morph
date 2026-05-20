class MorphParseError(Exception):
    """Base class for all Morph parse errors."""


class UnexpectedTokenError(MorphParseError):
    def __init__(self, expected, got, line=0, col=0):
        super().__init__(
            f"Expected {expected}, got {got!r} at {line}:{col}"
        )


class MissingAttributeError(MorphParseError):
    def __init__(self, attr, tag):
        super().__init__(f"Missing required attribute '{attr}' on <{tag}>")


class UnsupportedPropertyError(MorphParseError):
    def __init__(self, prop):
        super().__init__(f"Unsupported CSS property: '{prop}'")
