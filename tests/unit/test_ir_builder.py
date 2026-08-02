from morph.ir.builder import IRBuilder
from morph.ir.style import IRStyle
from morph.style.tailwind import TailwindResolver


def test_empty_dom_returns_empty_ir():
    ir = IRBuilder().build({}, {}, TailwindResolver(project_root="."))
    assert ir == []


def test_ancestor_hover_rule_created():
    """h1:hover button should create an ancestor_hover_rule on button nodes."""
    walked = {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                        "tag": "div",
                        "props": {},
                        "children": [{
                            "tag": "h1",
                            "props": {},
                            "children": [{
                                "tag": "button",
                                "props": {"className": "btn"},
                                "children": [],
                            }],
                        }],
                    }],
            },
        }],
    }
    css_rules = {
        "h1:hover button": {"color": "#ff0000"},
        "div > p": {"margin": "10px"},  # non-hover rule
        "button:hover": {"background-color": "#00ff00"},  # self-hover
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(walked, css_rules, tw)
    assert len(windows) == 1
    # Find the button node
    def find_button(nodes):
        for n in nodes:
            if n.node_type == "button":
                return n
            found = find_button(n.children)
            if found:
                return found
        return None
    button = find_button(windows[0].nodes)
    assert button is not None, "Button node should exist"
    # Button should have an ancestor_hover_rule for h1
    ancestor_rules = button.ancestor_hover_rules
    assert len(ancestor_rules) == 1, f"Expected 1 ancestor hover rule, got {len(ancestor_rules)}"
    tag, style = ancestor_rules[0]
    assert tag == "h1", f"Expected ancestor_tag='h1', got '{tag}'"
    assert style.color == (1.0, 0.0, 0.0, 1.0), f"Expected red color, got {style.color}"
    # Button should also have its own hover_style from button:hover
    assert button.hover_style is not None, "Button should have self-hover style"
    # Non-hover rule div > p should not affect button
    assert button.node_type == "button"


def test_self_active_style_created():
    """button:active should create an active_style on button nodes."""
    walked = {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                    "tag": "button",
                    "props": {"className": "btn"},
                    "children": [],
                }],
            },
        }],
    }
    css_rules = {
        "button:active": {"background-color": "#ff0000"},
        "button:hover": {"background-color": "#00ff00"},
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(walked, css_rules, tw)
    button = windows[0].nodes[0]
    assert button.active_style is not None, "Button should have self-active style"
    assert button.active_style.bg_color == (1.0, 0.0, 0.0, 1.0), \
        f"Expected red active bg, got {button.active_style.bg_color}"
    # Hover should still be independent
    assert button.hover_style is not None, "Button should have self-hover style"
    assert button.hover_style.bg_color == (0.0, 1.0, 0.0, 1.0), \
        f"Expected green hover bg, got {button.hover_style.bg_color}"


def test_ancestor_active_rule_created():
    """h1:active button should create an ancestor_active_rule on button nodes."""
    walked = {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                        "tag": "div",
                        "props": {},
                        "children": [{
                            "tag": "h1",
                            "props": {},
                            "children": [{
                                "tag": "button",
                                "props": {"className": "btn"},
                                "children": [],
                            }],
                        }],
                    }],
            },
        }],
    }
    css_rules = {
        "h1:active button": {"color": "#0000ff"},
        "button:active": {"background-color": "#00ff00"},
        "div > p": {"margin": "10px"},  # non-active rule
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(walked, css_rules, tw)
    def find_button(nodes):
        for n in nodes:
            if n.node_type == "button":
                return n
            found = find_button(n.children)
            if found:
                return found
        return None
    button = find_button(windows[0].nodes)
    assert button is not None, "Button node should exist"
    ancestor_rules = button.ancestor_active_rules
    assert len(ancestor_rules) == 1, f"Expected 1 ancestor active rule, got {len(ancestor_rules)}"
    tag, style = ancestor_rules[0]
    assert tag == "h1", f"Expected ancestor_tag='h1', got '{tag}'"
    assert style.color == (0.0, 0.0, 1.0, 1.0), f"Expected blue color, got {style.color}"
    assert button.active_style is not None, "Button should have self-active style"
    # No ancestor hover rules should be created for the :active selector
    assert button.ancestor_hover_rules == [], "No ancestor hover rules expected"


def test_combined_hover_active_rule():
    """.btn:hover:active should populate both hover_style and active_style."""
    walked = {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                    "tag": "button",
                    "props": {"className": "btn"},
                    "children": [],
                }],
            },
        }],
    }
    css_rules = {
        ".btn:hover:active": {"background-color": "#ff00ff"},
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(walked, css_rules, tw)
    button = windows[0].nodes[0]
    assert button.hover_style is not None, "Expected hover_style from :hover:active"
    assert button.active_style is not None, "Expected active_style from :hover:active"
    assert button.active_style.bg_color == (1.0, 0.0, 1.0, 1.0), \
        f"Expected magenta bg, got {button.active_style.bg_color}"


def test_active_style_serialization_roundtrip():
    """active_style / ancestor_active_rules survive IR JSON serialization."""
    from morph.ir.serializer import IRSerializer
    from morph.ir.style import IRStyle
    from morph.ir.node import IRNode, IRWindow
    node = IRNode(
        node_id="b1",
        node_type="button",
        style=IRStyle(bg_color=(0.1, 0.1, 0.1, 1.0)),
        active_style=IRStyle(bg_color=(1.0, 0.0, 0.0, 1.0)),
        ancestor_active_rules=[("h1", IRStyle(color=(0.0, 0.0, 1.0, 1.0)))],
    )
    win = IRWindow(window_id="w1", title="T", width=400, height=300, nodes=[node])
    data = IRSerializer().to_dict([win])
    n = data["windows"][0]["nodes"][0]
    assert n["active_style"]["bg_color"] == [1.0, 0.0, 0.0, 1.0]
    rules = n["ancestor_active_rules"]
    assert len(rules) == 1 and rules[0]["ancestor_tag"] == "h1"
    assert rules[0]["style"]["color"] == [0.0, 0.0, 1.0, 1.0]


def _walked_button():
    return {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                    "tag": "button",
                    "props": {},
                    "children": [{"tag": "__text__", "props": {}, "text": "click"}],
                }],
            },
        }],
    }


def test_button_ua_defaults_applied():
    """Plain buttons get browser-like base + hover + active defaults."""
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(_walked_button(), {}, tw)
    btn = windows[0].nodes[0]
    assert btn.node_type == "button"
    # Base appearance
    assert btn.style.bg_color == (0xef / 255, 0xef / 255, 0xef / 255, 1.0)
    assert btn.style.border_width == 1.0
    assert btn.style.border_color == (0x76 / 255, 0x76 / 255, 0x76 / 255, 1.0)
    assert btn.style.border_radius == 4.0
    assert btn.style.padding == (1.0, 6.0, 1.0, 6.0)  # 1px 6px shorthand
    # Default hover + active
    assert btn.hover_style is not None, "Button should have UA default hover style"
    assert btn.hover_style.bg_color == (0xE6 / 255, 0xE6 / 255, 0xE6 / 255, 1.0)
    assert btn.active_style is not None, "Button should have UA default active style"
    assert btn.active_style.bg_color == (0xD4 / 255, 0xD4 / 255, 0xD4 / 255, 1.0)
    assert btn.active_style.border_color == (0x5A / 255, 0x5A / 255, 0x5A / 255, 1.0)


def test_button_ua_defaults_overridden_by_user_css():
    """User CSS (base, :hover, :active) overrides UA button defaults per-property."""
    css_rules = {
        "button": {"background-color": "#111111", "border-radius": "0px"},
        "button:hover": {"background-color": "#222222"},
        "button:active": {"background-color": "#333333", "color": "#ffffff"},
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(_walked_button(), css_rules, tw)
    btn = windows[0].nodes[0]
    # Base overridden
    assert btn.style.bg_color == (0x11 / 255, 0x11 / 255, 0x11 / 255, 1.0)
    assert btn.style.border_radius == 0.0
    # Unoverridden UA base props remain
    assert btn.style.border_width == 1.0
    # Hover: user bg wins, UA border/other props untouched
    assert btn.hover_style.bg_color == (0x22 / 255, 0x22 / 255, 0x22 / 255, 1.0)
    # Active: user bg + color win
    assert btn.active_style.bg_color == (0x33 / 255, 0x33 / 255, 0x33 / 255, 1.0)
    assert btn.active_style.color == (1.0, 1.0, 1.0, 1.0)
    # UA active border-color still applies since user didn't set it
    assert btn.active_style.border_color == (0x5A / 255, 0x5A / 255, 0x5A / 255, 1.0)


def test_non_button_no_ua_state_defaults():
    """Non-button tags don't get UA hover/active defaults."""
    walked = {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                    "tag": "div",
                    "props": {},
                    "children": [],
                }],
            },
        }],
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(walked, {}, tw)
    div = windows[0].nodes[0]
    assert div.hover_style is None
    assert div.active_style is None


def test_z_index_parsed_from_css():
    """z-index CSS values land on IRStyle.z_index (None for auto)."""
    walked = {
        "components": [{
            "exported": True,
            "jsx": {
                "tag": "morph-window",
                "props": {"title": "Test", "width": 400, "height": 300},
                "children": [{
                    "tag": "div",
                    "props": {"className": "overlay"},
                    "children": [{
                        "tag": "div",
                        "props": {"className": "behind"},
                        "children": [],
                    }],
                }],
            },
        }],
    }
    css_rules = {
        ".overlay": {"z-index": "10", "position": "absolute"},
        ".behind": {"z-index": "-2"},
        ".auto": {"z-index": "auto"},
    }
    tw = TailwindResolver(project_root=".")
    windows = IRBuilder().build(walked, css_rules, tw)
    overlay = windows[0].nodes[0]
    behind = overlay.children[0]
    assert overlay.style.z_index == 10
    assert overlay.style.position == "absolute"
    assert behind.style.z_index == -2
    # `auto` resolves to None (no explicit z-index)
    auto_style = IRStyle()
    assert auto_style.z_index is None


def test_z_index_serialization_roundtrip():
    """z_index survives IR JSON serialization, including None (auto)."""
    from morph.ir.serializer import IRSerializer
    from morph.ir.style import IRStyle
    from morph.ir.node import IRNode, IRWindow
    node = IRNode(
        node_id="n1",
        node_type="div",
        style=IRStyle(position="absolute", z_index=42),
        hover_style=IRStyle(z_index=100),
    )
    win = IRWindow(window_id="w1", title="T", width=400, height=300, nodes=[node])
    data = IRSerializer().to_dict([win])
    n = data["windows"][0]["nodes"][0]
    assert n["style"]["z_index"] == 42
    assert n["style"]["position"] == "absolute"
    assert n["hover_style"]["z_index"] == 100
    # auto → None survives round-trip
    auto = IRNode(node_id="n2", node_type="div", style=IRStyle(z_index=None))
    data2 = IRSerializer().to_dict([IRWindow(window_id="w2", title="T", width=1, height=1, nodes=[auto])])
    assert data2["windows"][0]["nodes"][0]["style"]["z_index"] is None
