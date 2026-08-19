"""Lint diagnostic with severity, rule code, and source location."""

from __future__ import annotations


class LintError:
    """A single lint finding in a .mx file.

    severity: "error" (blocks build/dev reload) or "warning" (reported only).
    code:     stable rule id, e.g. "mx-tag", usable in morph.config.json
              lint.disable / lint.severities.
    """

    __slots__ = ("severity", "code", "message", "suggestion",
                 "file_path", "line", "col", "source_lines")

    def __init__(self, severity: str, code: str, message: str,
                 file_path: str | None = None, line: int = 0, col: int = 0,
                 suggestion: str | None = None,
                 source_lines: list[str] | None = None):
        self.severity = severity
        self.code = code
        self.message = message
        self.suggestion = suggestion
        self.file_path = file_path
        self.line = line
        self.col = col
        self.source_lines = source_lines

    def __str__(self) -> str:
        return f"{self.code}: {self.message}"

    def __repr__(self) -> str:
        return (f"LintError({self.severity!r}, {self.code!r}, "
                f"{self.message!r}, {self.line}:{self.col})")