from __future__ import annotations
from dataclasses import dataclass, field


@dataclass
class DOMNode:
    """Base DOM node."""
    parent: DOMNode | None = field(default=None, repr=False)
    children: list[DOMNode] = field(default_factory=list, repr=False)


@dataclass
class TextNode(DOMNode):
    content: str = ""


@dataclass
class ElementNode(DOMNode):
    tag: str = ""
    attrs: dict[str, str] = field(default_factory=dict)
    # resolved after StyleResolver runs
    computed_style: dict[str, str] = field(default_factory=dict)

    @property
    def id(self) -> str | None:
        return self.attrs.get("id")

    @property
    def classes(self) -> list[str]:
        return self.attrs.get("class", "").split()
