from __future__ import annotations

import re

from morph.ir.node import IRWindow, IRNode

# CSS property → (C++ style field name, value type)
_CSS_TO_STYLE_FIELD: dict[str, tuple[str, str]] = {
    "background-color": ("bgColor", "color"),
    "color": ("color", "color"),
    "border-color": ("borderColor", "color"),
    "scrollbar-track-color": ("scrollbarTrackColor", "color"),
    "scrollbar-thumb-color": ("scrollbarThumbColor", "color"),
    "width": ("explicitWidth", "float"),
    "min-width": ("minWidth", "float"),
    "max-width": ("maxWidth", "float"),
    "height": ("explicitHeight", "float"),
    "min-height": ("minHeight", "float"),
    "max-height": ("maxHeight", "float"),
    "border-radius": ("borderRadius", "float"),
    "font-size": ("fontSize", "float"),
    "gap": ("gap", "float"),
    "left": ("left", "float"),
    "right": ("right", "float"),
    "top": ("top", "float"),
    "bottom": ("bottom", "float"),
    "flex-grow": ("flexGrow", "float"),
    "flex-shrink": ("flexShrink", "float"),
    "scrollbar-width": ("scrollbarWidth", "float"),
    "scrollbar-border-radius": ("scrollbarBorderRadius", "float"),
    "border-width": ("borderWidth", "float"),
    "z-index": ("zIndex", "int"),
    "opacity": ("opacity", "float"),
    "font-weight": ("fontWeight", "string"),
    "text-align": ("textAlign", "string"),
    "display": ("display", "string"),
    "flex-direction": ("flexDirection", "string"),
    "flex-wrap": ("flexWrap", "string"),
    "justify-content": ("justifyContent", "string"),
    "align-items": ("alignItems", "string"),
    "cursor": ("cursor", "string"),
    "border-style": ("borderStyle", "string"),
    "box-sizing": ("boxSizing", "string"),
    "overflow": ("overflow", "string"),
    "position": ("position", "string"),
    "flex-basis": ("flexBasis", "string"),
    "transform": ("transform", "transform"),
}

# Default/clean values for each style field (used when resetting class-based styles)
_DEFAULT_STYLE_VALUES: dict[str, float | tuple[float, float, float, float] | str] = {
    "bgColor": (0.0, 0.0, 0.0, 0.0),
    "color": (0.0, 0.0, 0.0, 1.0),
    "borderColor": (0.0, 0.0, 0.0, 1.0),
    "scrollbarTrackColor": (0.85, 0.85, 0.85, 0.4),
    "scrollbarThumbColor": (0.5, 0.5, 0.5, 0.6),
    "explicitWidth": -1.0,
    "explicitHeight": -1.0,
    "minWidth": -1.0,
    "maxWidth": -1.0,
    "minHeight": -1.0,
    "maxHeight": -1.0,
    "borderRadius": 0.0,
    "fontSize": 16.0,
    "gap": 0.0,
    "left": 0.0,
    "right": 0.0,
    "top": 0.0,
    "bottom": 0.0,
    "flexGrow": 0.0,
    "flexShrink": 1.0,
    "scrollbarWidth": 8.0,
    "scrollbarBorderRadius": 4.0,
    "opacity": 1.0,
    "borderWidth": 0.0,
    "zIndex": 0,
    "fontWeight": "normal",
    "textAlign": "left",
    "display": "block",
    "flexDirection": "row",
    "flexWrap": "nowrap",
    "justifyContent": "flex-start",
    "alignItems": "stretch",
    "cursor": "default",
    "borderStyle": "none",
    "boxSizing": "content-box",
    "overflow": "visible",
    "position": "static",
    "flexBasis": "auto",
}


def _css_val_to_cpp_assignments(css_prop: str, css_val: str) -> list[str]:
    """Convert a CSS property + value to C++ style assignment lines."""
    field_info = _CSS_TO_STYLE_FIELD.get(css_prop)
    if field_info is None:
        return []
    field_name, val_type = field_info
    prefix = f"n->style.{field_name}"

    if val_type == "color":
        try:
            from morph.utils.color import parse_color
            r, g, b, a = parse_color(css_val)
            return [
                f"{prefix}[0] = {r:.4f}f;",
                f"{prefix}[1] = {g:.4f}f;",
                f"{prefix}[2] = {b:.4f}f;",
                f"{prefix}[3] = {a:.4f}f;",
            ]
        except Exception:
            return []
    elif val_type == "float":
        try:
            from morph.style.units import to_px
            val = to_px(css_val)
            return [f"{prefix} = {val}f;"]
        except Exception:
            return []
    elif val_type == "int":
        try:
            val = 0 if css_val.strip().lower() == "auto" else int(float(css_val))
            return [f"{prefix} = {val};", f"n->style.zIndexSet = true;"]
        except Exception:
            return []
    elif val_type == "string":
        return [f'{prefix} = "{css_val}";']
    elif val_type == "transform":
        # Literal CSS value → parse at runtime via the feature-gated parser.
        escaped = css_val.replace("\\", "\\\\").replace('"', '\\"')
        return [f"morph::setCssTransform(n->style, \"{escaped}\", n->w, n->h);"]
    return []


def _css_field_reset_assignments(field_name: str) -> list[str]:
    """Generate C++ assignments to reset a style field to its default."""
    if field_name == "transform":
        return ["morph::resetCssTransform(n->style);"]
    default = _DEFAULT_STYLE_VALUES.get(field_name)
    if default is None:
        return []
    prefix = f"n->style.{field_name}"
    if isinstance(default, tuple) and len(default) == 4:
        return [
            f"{prefix}[0] = {default[0]:.4f}f;",
            f"{prefix}[1] = {default[1]:.4f}f;",
            f"{prefix}[2] = {default[2]:.4f}f;",
            f"{prefix}[3] = {default[3]:.4f}f;",
        ]
    elif isinstance(default, float):
        return [f"{prefix} = {default}f;"]
    elif isinstance(default, int):
        lines = [f"{prefix} = {default};"]
        if field_name == "zIndex":
            lines.append("n->style.zIndexSet = false;")
        return lines
    elif isinstance(default, str):
        return [f'{prefix} = "{default}";']
    return []

_LOGIC_PREHEADER = """#include <cstdio>
#include <string>
#if __has_include(<print>)
#include <print>
#endif
#include "core/node.h"
#include "reactivity/signal.h"
#include "core/event.h"
#include "types/js_value.h"

// Node registry and signal store (included via -I<runtime>/dev)
#include "signal_store.h"
#include "node_registry.h"

// Runtime color parser for reactive style color expressions
namespace morph {
inline void setColor(float rgba[4], const std::string& c) {
    if (c.size() == 7 && c[0] == '#') {
        rgba[0] = std::stoi(c.substr(1,2), nullptr, 16) / 255.0f;
        rgba[1] = std::stoi(c.substr(3,2), nullptr, 16) / 255.0f;
        rgba[2] = std::stoi(c.substr(5,2), nullptr, 16) / 255.0f;
        rgba[3] = 1.0f;
    } else if (c.size() == 4 && c[0] == '#') {
        rgba[0] = std::stoi(c.substr(1,1) + c[1], nullptr, 16) / 255.0f;
        rgba[1] = std::stoi(c.substr(2,1) + c[2], nullptr, 16) / 255.0f;
        rgba[2] = std::stoi(c.substr(3,1) + c[3], nullptr, 16) / 255.0f;
        rgba[3] = 1.0f;
    }
}
}

"""

# Built-in headers already provided by _LOGIC_PREHEADER or the compiler include paths.
_BUILTIN_HEADERS = frozenset({
    "<cstdio>", "<print>", "<string>",
    '"core/node.h"', '"reactivity/signal.h"', '"core/event.h"', '"types/js_value.h"',
    '"signal_store.h"', '"node_registry.h"',
})

_LOGIC_FOOTER = """
void morph_logic_cleanup() {
    for (int i = 0; i < __effect_count; i++) {
        if (__effects[i]) {
            __effects[i]->cleanup();
            __effects[i] = nullptr;
        }
    }
    __effect_count = 0;
}

} // extern "C"
"""


def _effect_dep_exprs(deps: str, state_getters: set[str]) -> list[str] | None:
    """Return `__st_*.get()` expressions for deps if every dep is a known state var.

    Returns None when the effect can't be guarded (no deps / unknown deps).
    """
    if not deps or deps == "[]":
        return None
    ids = re.findall(r"[A-Za-z_$][A-Za-z0-9_$]*", deps)
    if not ids:
        return None
    exprs: list[str] = []
    for name in dict.fromkeys(ids):
        if name not in state_getters:
            return None
        exprs.append(f"__st_{name}.get()")
    return exprs


def _get_cpp_type(init: str) -> str:
    raw_init = init.strip()
    if raw_init.startswith("'") and raw_init.endswith("'"):
        return "std::string"
    if raw_init in ("true", "false"):
        return "bool"
    if raw_init.startswith('"'):
        return "std::string"
    if "." in raw_init:
        return "double"
    if raw_init.isdigit() or (raw_init.startswith("-") and raw_init[1:].isdigit()):
        return "int"
    return "auto"


def _clean_init(init: str) -> str:
    raw = init.strip()
    if raw.startswith("'") and raw.endswith("'"):
        return f'"{raw[1:-1]}"'
    return raw


def emit_logic(windows: list[IRWindow]) -> str:
    lines = [_LOGIC_PREHEADER]

    # Collect all state vars
    all_state_vars: list[dict] = []
    for w in windows:
        for sv in w.state_vars:
            name = sv.get("getter", "")
            if name and sv not in all_state_vars:
                all_state_vars.append(sv)
    state_getters: set[str] = {sv.get("getter", "") for sv in all_state_vars}

    # Extra headers required by transpiled code (task.h for coroutines, etc.)
    # These must go before extern "C" block because they may contain templates.
    extra_headers: set[str] = set()
    for w in windows:
        for h in w.extra_headers:
            if h not in _BUILTIN_HEADERS:
                for prefix in ('"../../morph/runtime/', '"../../morph/runtime/types/', '"../../morph/runtime/dev/'):
                    if h.startswith(prefix):
                        h = '"' + h[len(prefix):]
                        break
                extra_headers.add(h)
    for h in sorted(extra_headers):
        lines.append(f'#include {h}')

    # ── File-scope signal statics (persist across morph_logic_init calls) ──
    if all_state_vars:
        lines.append("")
        lines.append("// ── morphState signals ──")
        for sv in all_state_vars:
            name = sv.get("getter", "")
            if not name:
                continue
            init = _clean_init(sv.get("init", "0"))
            cpp_type = _get_cpp_type(sv.get("init", "0"))
            lines.append(f'static morph::Signal<{cpp_type}> __st_{name}({init});')

    # ── File-scope premain functions (static inline, same as build mode) ──
    seen_fns = set()
    for w in windows:
        for f in w.premain_functions:
            if f not in seen_fns:
                seen_fns.add(f)
                lines.append("")
                lines.append(f)

    # ── User morphEffect declarations ──
    all_effect_decls: list[dict] = []
    for w in windows:
        all_effect_decls.extend(w.effect_decls)

    # Effects with deps that map to known state vars can be guarded so they
    # skip their initial run when hot-reloaded without any dep changing.
    guarded_map: dict[int, list[str]] = {}
    for idx, ed in enumerate(all_effect_decls):
        exprs = _effect_dep_exprs(ed.get("deps", "").strip(), state_getters)
        if exprs:
            guarded_map[idx] = exprs

    lines.append('extern "C" {')
    lines.append("")
    lines.append("static int __effect_count = 0;")
    lines.append("")

    for i in range(len(guarded_map)):
        lines.append(f"static std::string __esig_{i};")
    if guarded_map:
        lines.append("")

    # ── morph_logic_rewire: wire events + (re)create effects ──
    lines.append("void morph_logic_rewire(::NodeRegistry& nodes, ::SignalStore& store) {")
    lines.append("    (void)store;")

    wired_events: set[tuple[str, str]] = set()
    for w in windows:
        for node in w.nodes:
            _emit_node_events(lines, node, wired_events, indent="    ")

    for w in windows:
        for node in w.nodes:
            _emit_node_effects(lines, node, indent="    ")

    gidx = 0
    for idx, ed in enumerate(all_effect_decls):
        deps = ed.get("deps", "").strip()
        if deps == "[]":
            continue  # run-once, executed in morph_logic_init
        cpp_lambda = ed["lambda"]
        if idx in guarded_map:
            sig_expr = " + \"|\" + ".join(
                f"morph::str({e})" for e in guarded_map[idx]
            )
            lines.append("    {")
            lines.append(f"        auto __ef = {cpp_lambda};")
            lines.append(f"        __effects[__effect_count++] = morph::create_effect([&, __ef]() {{")
            lines.append(f"            std::string __sig = {sig_expr};")
            lines.append(f"            if (__sig == __esig_{gidx}) return;")
            lines.append(f"            __esig_{gidx} = __sig;")
            lines.append("            __ef();")
            lines.append("        });")
            lines.append("    }")
            gidx += 1
        else:
            lines.append(f'    __effects[__effect_count++] = morph::create_effect({cpp_lambda});')

    lines.append("}")
    lines.append("")

    # ── morph_logic_init: sync signals from store, run-once effects ──
    lines.append("void morph_logic_init(::NodeRegistry& nodes, ::SignalStore& store) {")

    # Sync signal values from store (for hot-reload persistence)
    for sv in all_state_vars:
        name = sv.get("getter", "")
        if not name:
            continue
        init = _clean_init(sv.get("init", "0"))
        cpp_type = _get_cpp_type(sv.get("init", "0"))
        lines.append(f'    __st_{name}.set(store.get_or_create<{cpp_type}>("{name}", {init}).get());')

    if all_state_vars:
        lines.append("")

    # Run-once effects (no deps) — never re-run on in-place rewires
    for ed in all_effect_decls:
        if ed.get("deps", "").strip() != "[]":
            continue
        lines.append("    { // morphEffect (run once)")
        lines.append(f"        auto __ef_fn = {ed['lambda']};")
        lines.append("        __ef_fn();")
        lines.append("    }")

    lines.append("    morph_logic_rewire(nodes, store);")
    lines.append("}")
    lines.append(_LOGIC_FOOTER)

    # The effects array must hold every effect created in morph_logic_rewire.
    # Count the exact number of emission sites and size the array accordingly
    # so large apps (hundreds of reactive-style nodes) don't overflow a
    # fixed-size buffer.
    effect_count = sum(1 for ln in lines if "__effects[__effect_count++]" in ln)
    extern_idx = next(i for i, ln in enumerate(lines) if ln.strip() == 'extern "C" {')
    lines.insert(extern_idx + 1, f"static morph::EffectNode* __effects[{max(effect_count, 1)}];")

    return "\n".join(lines)


def _emit_node_events(lines: list[str], node: IRNode, wired_events: set[tuple[str, str]],
                      indent: str = "    ") -> None:
    node_id = node.node_id

    if node.node_type in ("__expr__", "__text__"):
        return

    if node.node_type == "__conditional__":
        for tn in node.then_nodes:
            _emit_node_events(lines, tn, wired_events, indent)
        for en in node.else_nodes:
            _emit_node_events(lines, en, wired_events, indent)
        return

    # Emit events for this node
    _EVENT_MEMBER_MAP = {
        "click": "onClick",
        "keyup": "onKeyUp",
        "keydown": "onKeyDown",
        "dblclick": "onDoubleClick",
        "mousedown": "onMouseDown",
        "mouseup": "onMouseUp",
        "mouseenter": "onMouseEnter",
        "mouseleave": "onMouseLeave",
    }

    for event in node.events:
        member = _EVENT_MEMBER_MAP.get(event.trigger, "onClick")
        wire_key = (node_id, member)
        if wire_key in wired_events:
            continue
        if event.action == "call":
            rhs = event.target
            lines.append(f'{indent}if (auto* n = nodes.get("{node_id}")) {{')
            lines.append(f'{indent}    n->{member} = {rhs};')
            lines.append(f'{indent}}}')
            wired_events.add(wire_key)
        elif event.action in ("open", "close", "navigate"):
            # Note: open/close/navigate require multi-window management (build mode only)
            # In dev mode, skip these actions silently.
            wired_events.add(wire_key)
        elif event.action == "log":
            escaped = event.target.replace('"', '\\"')
            lines.append(f'{indent}if (auto* n = nodes.get("{node_id}")) {{')
            lines.append(f'{indent}    n->{member} = [](JsObject) {{ fprintf(stderr, "{escaped}\\n"); }};')
            lines.append(f'{indent}}}')
            wired_events.add(wire_key)

    # Recurse children
    for child in node.children:
        _emit_node_events(lines, child, wired_events, indent)


def _emit_node_effects(lines: list[str], node: IRNode, indent: str = "    ") -> None:
    node_id = node.node_id

    if node.node_type == "__expr__":
        return

    if node.node_type == "__text__":
        if node.reactive_text:
            lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
            lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
            lines.append(f'{indent}    if (n) n->setText(morph::str({node.reactive_text}));')
            lines.append(f'{indent}}});')
        return

    if node.node_type == "__conditional__":
        then_id = node.then_nodes[0].node_id if node.then_nodes else ""
        else_id = node.else_nodes[0].node_id if node.else_nodes else ""
        lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
        lines.append(f'{indent}    auto* container = nodes.get("{node_id}");')
        lines.append(f'{indent}    if (!container) return;')
        if node.condition_expr:
            lines.append(f'{indent}    container->removeAllChildren();')
            lines.append(f'{indent}    if ({node.condition_expr}) {{')
            if then_id:
                lines.append(f'{indent}        auto* child = nodes.get("{then_id}");')
                lines.append(f'{indent}        if (child) container->addChild(child);')
            lines.append(f'{indent}    }} else {{')
            if else_id:
                lines.append(f'{indent}        auto* child = nodes.get("{else_id}");')
                lines.append(f'{indent}        if (child) container->addChild(child);')
            lines.append(f'{indent}    }}')
        lines.append(f'{indent}}});')
        for tn in node.then_nodes:
            _emit_node_effects(lines, tn, indent)
        for en in node.else_nodes:
            _emit_node_effects(lines, en, indent)
        return

    # ── Reactive style (inline style expressions) ──
    if node.reactive_style:
        for css_prop, cpp_expr in node.reactive_style.items():
            field_info = _CSS_TO_STYLE_FIELD.get(css_prop)
            if field_info is None:
                continue
            field_name, val_type = field_info
            if val_type == "float":
                lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
                lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
                lines.append(f'{indent}    if (!n) return;')
                lines.append(f'{indent}    n->interruptStateTransitions();')
                lines.append(f'{indent}    n->style.{field_name} = (float)({cpp_expr});')
                lines.append(f'{indent}    n->markDirty(LayoutDirty);')
                lines.append(f'{indent}    n->markDirty(PaintDirty);')
                lines.append(f'{indent}}});')
            elif val_type == "string":
                lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
                lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
                lines.append(f'{indent}    if (!n) return;')
                lines.append(f'{indent}    n->interruptStateTransitions();')
                lines.append(f'{indent}    n->style.{field_name} = morph::str({cpp_expr});')
                lines.append(f'{indent}    n->markDirty(PaintDirty);')
                lines.append(f'{indent}}});')
            elif val_type == "color":
                lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
                lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
                lines.append(f'{indent}    if (!n) return;')
                lines.append(f'{indent}    n->interruptStateTransitions();')
                lines.append(f'{indent}    morph::setColor(n->style.{field_name}, morph::str({cpp_expr}));')
                lines.append(f'{indent}    n->markDirty(PaintDirty);')
                lines.append(f'{indent}}});')
            elif val_type == "transform":
                lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
                lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
                lines.append(f'{indent}    if (!n) return;')
                lines.append(f'{indent}    n->interruptStateTransitions();')
                lines.append(f'{indent}    morph::setCssTransform(n->style, morph::str({cpp_expr}), n->w, n->h);')
                lines.append(f'{indent}    n->markDirty(PaintDirty);')
                lines.append(f'{indent}}});')

    # ── Propagate font-size to child TextNodes ──
    # Font-size set on a container node must also be applied to any
    # direct TextNode children so the text actually renders larger/smaller.
    if node.reactive_style:
        for css_prop, cpp_expr in node.reactive_style.items():
            if css_prop != "font-size":
                continue
            field_info = _CSS_TO_STYLE_FIELD.get(css_prop)
            if field_info is None:
                continue
            field_name, val_type = field_info
            if val_type != "float":
                continue
            for child in node.children:
                if child.node_type == "__text__":
                    lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
                    lines.append(f'{indent}    auto* n = nodes.get("{child.node_id}");')
                    lines.append(f'{indent}    if (!n) return;')
                    lines.append(f'{indent}    n->interruptStateTransitions();')
                    lines.append(f'{indent}    n->style.fontSize = (float)({cpp_expr});')
                    lines.append(f'{indent}    n->markDirty(LayoutDirty);')
                    lines.append(f'{indent}    n->markDirty(PaintDirty);')
                    lines.append(f'{indent}}});')

    # ── Reactive className (stores string on node) ──
    if node.reactive_class:
        lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
        lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
        lines.append(f'{indent}    if (!n) return;')
        lines.append(f'{indent}    n->setClassName({node.reactive_class});')
        lines.append(f'{indent}}});')

    # ── Conditional class style effects (direct condition → style, no string searching) ──
    for cond_cpp, on_styles, off_styles in node.class_conditional_effects:
        lines.append(f'{indent}__effects[__effect_count++] = morph::create_effect([&]() {{')
        lines.append(f'{indent}    auto* n = nodes.get("{node_id}");')
        lines.append(f'{indent}    if (!n) return;')
        lines.append(f'{indent}    n->interruptStateTransitions();')
        lines.append(f'{indent}    if ({cond_cpp}) {{')
        for css_prop, css_val in on_styles.items():
            for a in _css_val_to_cpp_assignments(css_prop, css_val):
                lines.append(f'{indent}        {a}')
        lines.append(f'{indent}    }} else {{')
        if off_styles:
            for css_prop, css_val in off_styles.items():
                for a in _css_val_to_cpp_assignments(css_prop, css_val):
                    lines.append(f'{indent}        {a}')
        else:
            # Reset affected fields to defaults
            reset_fields = set()
            for css_prop in on_styles:
                fi = _CSS_TO_STYLE_FIELD.get(css_prop)
                if fi:
                    reset_fields.add(fi[0])
            for fname in sorted(reset_fields):
                for a in _css_field_reset_assignments(fname):
                    lines.append(f'{indent}        {a}')
        lines.append(f'{indent}    }}')
        lines.append(f'{indent}    n->markDirty(PaintDirty);')
        lines.append(f'{indent}}});')

    # Recurse children
    for child in node.children:
        _emit_node_effects(lines, child, indent)
