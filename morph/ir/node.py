from __future__ import annotations
from dataclasses import dataclass, field
from morph.ir.style import IRStyle
from morph.ir.event import IREvent


@dataclass
class IRNode:
    node_id: str
    node_type: str                              # div, button, text, ...
    style: IRStyle = field(default_factory=IRStyle)
    hover_style: IRStyle | None = None          # style applied on :hover
    active_style: IRStyle | None = None         # style applied on :active (pressed)
    children: list[IRNode] = field(default_factory=list)
    events: list[IREvent] = field(default_factory=list)
    text_content: str = ""
    attrs: dict[str, str] = field(default_factory=dict)
    parent_id: str | None = None
    raw_styles: dict[str, str] = field(default_factory=dict)
    transition_duration: float = 0.0   # 0 = no transition
    transition_easing: str = "ease-in-out"

    ancestor_hover_rules: list[tuple[str, IRStyle]] = field(default_factory=list)
    # Each tuple: (ancestor_tag, resolved IRStyle)

    ancestor_active_rules: list[tuple[str, IRStyle]] = field(default_factory=list)
    # Each tuple: (ancestor_tag, resolved IRStyle)

    # computed by LayoutEngine
    x: float = 0.0
    y: float = 0.0
    w: float = 0.0
    h: float = 0.0

    # ── Reactivity ────────────────────────────────────────────
    reactive_text: str = ""          # C++ expression for dynamic text (empty = static)
    condition_expr: str = ""         # C++ expression for conditional (empty = not conditional)
    then_nodes: list[IRNode] = field(default_factory=list)
    else_nodes: list[IRNode] = field(default_factory=list)

    # ── Reactive className / style ────────────────────────────
    reactive_class: str = ""         # C++ expression for dynamic className (empty = static)
    reactive_style: dict[str, str] = field(default_factory=dict)
    # CSS property name → C++ expression for dynamic inline style
    # e.g. {"width": "__st_count.get()"}
    class_conditional_effects: list[tuple[str, dict[str, str], dict[str, str]]] = field(default_factory=list)
    # [(condition_cpp, {css_prop: css_value_on}, {css_prop: css_value_off}), ...]
    # For each conditional class: if condition is true → apply on_styles, else → apply off_styles


@dataclass
class IRWindow:
    window_id: str
    title: str
    width: int
    height: int
    visible: bool = True
    modal: bool = False
    renderer: str = "flash"  # "flash" (default) | "forge"
    nodes: list[IRNode] = field(default_factory=list)
    startup_logs: list[str] = field(default_factory=list)
    premain_functions: list[str] = field(default_factory=list)
    extra_headers: list[str] = field(default_factory=list)
    state_vars: list[dict] = field(default_factory=list)  # morphState definitions
    effect_decls: list[dict] = field(default_factory=list)  # morphEffect declarations


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
