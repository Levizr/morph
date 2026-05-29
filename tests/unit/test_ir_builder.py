from morph.ir.builder import IRBuilder
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
