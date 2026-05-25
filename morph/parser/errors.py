class MorphParseError(Exception):
    """Parse error with file location and source context for rich terminal display."""

    def __init__(self, message: str,
                 file_path: str | None = None,
                 line: int = 0,
                 col: int = 0,
                 source_lines: list[str] | None = None):
        self.file_path = file_path
        self.line = line
        self.col = col
        self.source_lines = source_lines
        super().__init__(message)


class UnexpectedTokenError(MorphParseError):
    def __init__(self, expected, got, line=0, col=0):
        super().__init__(
            f"Expected {expected}, got {got!r} at {line}:{col}"
        )


class MissingAttributeError(MorphParseError):
    def __init__(self, attr, tag):
        super().__init__(
            f"Missing required attribute '{attr}' on <{tag}>"
        )


class UnsupportedPropertyError(MorphParseError):
    def __init__(self, prop):
        super().__init__(f"Unsupported CSS property: '{prop}'")
