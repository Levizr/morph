from __future__ import annotations
from dataclasses import dataclass, field
from morph.ir.style import IRStyle
from morph.ir.event import IREvent


@dataclass
class IRNode:
    node_id: str
    node_type: str                              # div, button, text, ...
    style: IRStyle = field(default_factory=IRStyle)
    children: list[IRNode] = field(default_factory=list)
    events: list[IREvent] = field(default_factory=list)
    text_content: str = ""
    parent_id: str | None = None

    # computed by LayoutEngine
    x: float = 0.0
    y: float = 0.0
    w: float = 0.0
    h: float = 0.0


@dataclass
class IRWindow:
    window_id: str
    title: str
    width: int
    height: int
    visible: bool = True
    modal: bool = False
    nodes: list[IRNode] = field(default_factory=list)


@dataclass
class IRPage:
    page_id: str
    nodes: list[IRNode] = field(default_factory=list)


@dataclass
class IRViewport:
    viewport_id: str
    driver_header: str      # e.g. "cpp/scene_renderer.h"
    driver_class: str       # e.g. "SceneRenderer"
    style: IRStyle = field(default_factory=IRStyle)
    x: float = 0.0
    y: float = 0.0
    w: float = 0.0
    h: float = 0.0
