"""Semantic linting for .mx (JSX/TSX) files.

The linter walks the tree-sitter AST directly (nodes carry line/col
positions) and applies the rules from ``morph.checker.registry``. Errors
block the dev/build pipeline; warnings are reported only. Rule behavior
can be tuned per project via ``morph.config.json``:

.. code-block:: json

    {
      "lint": {
        "disable": ["mx-list-key"],
        "severities": { "mx-tag": "warning" }
      }
    }
"""

from __future__ import annotations

import difflib
import os
import re
from pathlib import Path

import tree_sitter_css as tscss
import tree_sitter_typescript as tsts
from tree_sitter import Language, Node, Parser

from morph.checker.errors import LintError
from morph.checker import registry as R

_JS_LANG = Language(tsts.language_tsx())
_JS_PARSER = Parser(_JS_LANG)
_CSS_LANG = Language(tscss.language())
_CSS_PARSER = Parser(_CSS_LANG)

_JSX_ELEMENT_TYPES = ("jsx_element", "jsx_self_closing_element")
_EXPR_WRAPPER_TYPES = ("jsx_expression", "jsx_expression_container")

# ── mx-js-* : static knowledge of the TSX→C++ translator ─────────────
# What the translator (morph/js/codegen.py) actually supports, and which
# browser/JS globals have no native counterpart.

#: Globals that exist in browsers/JS but are NOT mapped by the translator.
#: value = what kind of API it belongs to (for the error message).
_UNSUPPORTED_GLOBALS: dict[str, str] = {
    "document": "DOM", "window": "browser window", "localStorage": "storage",
    "sessionStorage": "storage", "navigator": "browser", "location": "browser",
    "history": "browser", "screen": "browser", "alert": "browser dialog",
    "prompt": "browser dialog", "confirm": "browser dialog", "print": "browser",
    "open": "browser", "close": "browser", "focus": "browser", "blur": "browser",
    "requestAnimationFrame": "animation", "cancelAnimationFrame": "animation",
    "queueMicrotask": "scheduling", "structuredClone": "cloning",
    "atob": "encoding", "btoa": "encoding", "escape": "encoding",
    "unescape": "encoding", "encodeURIComponent": "encoding",
    "decodeURIComponent": "encoding", "encodeURI": "encoding",
    "decodeURI": "encoding", "globalThis": "global scope", "self": "browser",
    "top": "browser", "parent": "browser", "frames": "browser",
    "performance": "performance", "crypto": "crypto",
    "Math": "Math", "JSON": "JSON", "Date": "Date", "RegExp": "RegExp",
    "Promise": "Promise", "Proxy": "Proxy", "Reflect": "Reflect",
    "Symbol": "Symbol", "BigInt": "BigInt", "Map": "Map", "Set": "Set",
    "WeakMap": "WeakMap", "WeakSet": "WeakSet",
    "parseInt": "parseInt", "parseFloat": "parseFloat", "isNaN": "isNaN",
    "isFinite": "isFinite", "Infinity": "Infinity", "NaN": "NaN",
    "arguments": "arguments", "Event": "Event", "KeyboardEvent": "KeyboardEvent",
    "MouseEvent": "MouseEvent", "FocusEvent": "FocusEvent",
    "CustomEvent": "CustomEvent", "EventTarget": "EventTarget",
    "HTMLElement": "HTMLElement", "Element": "Element", "Node": "Node",
    "addEventListener": "addEventListener",
    "removeEventListener": "removeEventListener", "dispatchEvent": "dispatchEvent",
    "Object": "Object", "Array": "Array", "String": "String",
    "Number": "Number", "Boolean": "Boolean", "Function": "Function",
}

#: Bases where ANY member access is unsupported (value = label).
_MEMBER_BASES_UNSUPPORTED: dict[str, str] = {
    "Math": "the Math library is not available in the native runtime",
    "JSON": "the JSON library is not available in the native runtime",
    "Date": "the Date library is not available in the native runtime",
    "RegExp": "regular expressions are not available in the native runtime",
}

#: console methods the translator maps to std::println.
_CONSOLE_METHODS = frozenset({"log", "warn", "error", "info"})

#: JS statics the translator does not map (value = label).
_JS_STATIC_METHODS: dict[str, dict[str, str]] = {
    "Object": {
        "keys": "use a manual loop", "values": "use a manual loop",
        "entries": "use a manual loop", "assign": "merge fields manually",
        "create": "not available", "defineProperty": "not available",
        "freeze": "not available", "getPrototypeOf": "not available",
        "is": "use ===", "hasOwn": "use obj.has(key)",
    },
    "Array": {
        "from": "not available", "of": "not available", "isArray": "not available",
    },
    "String": {
        "fromCharCode": "not available", "fromCodePoint": "not available",
    },
    "Number": {
        "isInteger": "use Number.is_int()", "isSafeInteger": "not available",
        "isNaN": "use == NaN checks", "isFinite": "not available",
        "parseInt": "use parseInt", "parseFloat": "use parseFloat",
        "MAX_SAFE_INTEGER": "not available", "MIN_SAFE_INTEGER": "not available",
        "MAX_VALUE": "not available", "MIN_VALUE": "not available",
        "EPSILON": "not available", "POSITIVE_INFINITY": "not available",
        "NEGATIVE_INFINITY": "not available",
    },
}

#: Methods a morphState value (JsValue) actually implements.
_STATE_METHODS = frozenset({"length", "push", "pop", "get", "toString"})

#: Methods the runtime JsString implements (valid on JsString receivers).
_JSSTRING_METHODS = frozenset({
    "length", "empty", "toUpperCase", "toLowerCase", "substring", "slice",
    "charAt", "indexOf", "replace", "trim", "toString",
})

#: JS-only methods that exist on no runtime type — always an error when the
#: receiver is a state value or a string literal.
_JS_ONLY_METHODS = frozenset({
    "split", "includes", "startsWith", "endsWith", "match", "search", "repeat",
    "padStart", "padEnd", "charCodeAt", "concat", "localeCompare", "at",
    "map", "filter", "forEach", "join", "find", "findIndex", "reduce",
    "sort", "reverse", "shift", "unshift", "splice", "every", "some",
    "flat", "flatMap", "keys", "values", "entries", "toFixed",
    "toPrecision", "codePointAt", "isArray",
})

#: AST contexts that hold type names, not values — identifiers there are types.
_TYPE_NODES = frozenset({
    "type_annotation", "type_arguments", "generic_type", "type_identifier",
    "predefined_type", "satisfies_type", "interface_declaration",
    "type_alias_declaration", "enum_declaration", "property_signature",
    "method_signature", "function_signature", "extends_clause",
    "implements_clause",
})

#: Operators the translator comments out or emits verbatim → broken C++.
_UNSUPPORTED_OPS: dict[str, str] = {
    "**": "the exponent operator is not supported (use repeated multiplication)",
    "**=": "the exponent assignment is not supported",
    ">>>": "the unsigned shift is not supported",
    "typeof": "typeof is not supported (compare against undefined/null instead)",
    "void": "the void operator is not supported",
    "delete": "delete is not supported on native objects",
    "&&=": "&&= is not supported",
    "||=": "||= is not supported",
    "??=": "??= is not supported",
    "instanceof": "instanceof is not supported (use a type discriminant)",
    "in": "the in operator is not supported (use obj.has(key))",
}


# ── Generic AST helpers ──────────────────────────────────────────────

def _find_all(node: Node, node_type: str) -> list[Node]:
    out = []
    if node.type == node_type:
        out.append(node)
    for child in node.children:
        out.extend(_find_all(child, node_type))
    return out


def _text(node: Node) -> str:
    return node.text.decode() if node is not None else ""


def _pos(node: Node) -> tuple[int, int]:
    return node.start_point[0] + 1, node.start_point[1] + 1


def _first_child(node: Node, *types: str) -> Node | None:
    for child in node.children:
        if child.type in types:
            return child
    return None


def _named_children(node: Node) -> list[Node]:
    return [c for c in node.children if c.is_named]


def _opening_of(el: Node) -> Node:
    """Return the tag/attr-bearing node of a JSX element.

    jsx_element has a separate jsx_opening_element child; for
    jsx_self_closing_element the element node itself carries the tag.
    """
    for child in el.children:
        if child.type == "jsx_opening_element":
            return child
    return el


def _tag_of(el: Node) -> str:
    opening = _opening_of(el)
    tag = _first_child(opening, "identifier", "jsx_namespace_name", "member_expression")
    return _text(tag).strip() if tag else ""


def _attrs_of(el: Node) -> list[tuple[str, Node, Node | None]]:
    """Return [(name, attr_node, value_node)] for each jsx_attribute."""
    opening = _opening_of(el)
    out = []
    for child in opening.children:
        if child.type != "jsx_attribute":
            continue
        name = ""
        value: Node | None = None
        for part in child.children:
            if part.type == "property_identifier":
                name = _text(part)
            elif part.type in ("string",) + _EXPR_WRAPPER_TYPES:
                value = part
        out.append((name, child, value))
    return out


def _expr_inner(wrapper: Node) -> str:
    """Inner JS source of a { ... } jsx expression wrapper."""
    raw = _text(wrapper)
    if raw.startswith("{") and raw.endswith("}"):
        return raw[1:-1].strip()
    return raw.strip()


def _is_jsx_comment(wrapper: Node) -> bool:
    inner = _expr_inner(wrapper)
    return inner.startswith("/*") and "*/" in inner


def _extract_css_classes(css: str) -> set[str]:
    """Collect .class names defined in a stylesheet (selectors)."""
    if not css.strip():
        return set()
    classes: set[str] = set()
    try:
        tree = _CSS_PARSER.parse(css.encode("utf-8"))
        for cls in _find_all(tree.root_node, "class_name"):
            for child in cls.children:
                if child.type == "identifier":
                    classes.add(child.text.decode())
                    break
    except Exception:
        pass
    return classes


def _unwrap_jsx(node: Node) -> Node | None:
    if node.type in _JSX_ELEMENT_TYPES:
        return node
    if node.type == "parenthesized_expression":
        for child in node.children:
            got = _unwrap_jsx(child)
            if got is not None:
                return got
    return None


def _call_parts(call: Node) -> list[str]:
    """['morphState'] / ['CSS', 'load'] / ['items', 'map'] for a call_expression."""
    fn = _first_child(call, "identifier", "member_expression")
    if fn is None:
        return []
    if fn.type == "identifier":
        return [_text(fn)]
    return [c.text.decode() for c in fn.children
            if c.type in ("identifier", "property_identifier")]


def _args_of(call: Node) -> list[Node]:
    args_node = _first_child(call, "arguments")
    if args_node is None:
        return []
    return [c for c in args_node.children if c.is_named]


def _is_inside(node: Node, ancestor_type: str) -> bool:
    p = node.parent
    while p is not None:
        if p.type == ancestor_type:
            return True
        p = p.parent
    return False


def _inside_map_callback(node: Node) -> bool:
    """True when ``node`` sits inside a ``arr.map(cb => ...)`` arrow body."""
    p = node.parent
    while p is not None:
        if p.type == "arrow_function":
            call = p.parent
            while call is not None and call.type != "call_expression":
                call = call.parent
            if call is not None and _call_parts(call)[-1:] == ["map"]:
                return True
            return False
        p = p.parent
    return False


def _map_callbacks(root: Node) -> list[tuple[Node, Node, list[str]]]:
    """(map_call, arrow_callback, param_names) for every ``arr.map(cb)`` call."""
    out = []
    for call in _find_all(root, "call_expression"):
        parts = _call_parts(call)
        if not parts or parts[-1] != "map":
            continue
        arrow = None
        for arg in _args_of(call):
            if arg.type == "arrow_function":
                arrow = arg
                break
        if arrow is None:
            continue
        params: list[str] = []
        for child in arrow.children:
            if child.type == "identifier" and len(params) < 2:
                params.append(_text(child))
            elif child.type == "formal_parameters":
                for ident in _find_all(child, "identifier"):
                    if len(params) < 2:
                        params.append(_text(ident))
        out.append((call, arrow, params))
    return out


def _arrow_body_jsx(arrow: Node) -> Node | None:
    for child in arrow.children:
        if child.type in _JSX_ELEMENT_TYPES:
            return child
        if child.type == "parenthesized_expression":
            got = _unwrap_jsx(child)
            if got is not None:
                return got
        if child.type == "statement_block":
            for ret in _find_all(child, "return_statement"):
                for sub in ret.children:
                    got = _unwrap_jsx(sub)
                    if got is not None:
                        return got
    return None


# ── JS transpile check (mirrors IRBuilder._transpile_js_expr) ────────

def _transpile_expr(js_source: str, state_vars_map: dict[str, str]) -> str | None:
    """Return None on success, or the failure message when the expression
    cannot be compiled to C++ by the JS→C++ translator."""
    if not js_source.strip():
        return None
    try:
        from morph.js.ast_builder import TSAstBuilder
        from morph.js.codegen import TSToCppTranslator
        tree = _JS_PARSER.parse(js_source.encode("utf-8"))
        builder = TSAstBuilder()
        expr = builder.build_expression(tree.root_node)
        translator = TSToCppTranslator(indent_level=0,
                                       state_vars=state_vars_map or {})
        if (hasattr(expr, "statements") and expr.statements
                and hasattr(expr.statements[0], "expression")):
            inner = expr.statements[0].expression
            translator._translate_node(inner)
        else:
            if hasattr(expr, "statements") and not expr.statements:
                return None
            translator.translate(expr)
        return None
    except Exception as e:
        return f"{type(e).__name__}: {e}"


def _transpile_arrow(js_source: str, state_vars_map: dict[str, str],
                     event_handler: bool = True) -> str | None:
    """Compile an arrow-function body the way IRBuilder does.

    event_handler=True mirrors the JSX event path (builder.py:845-861);
    event_handler=False mirrors the morphEffect callback path
    (builder.py:381-399)."""
    try:
        from morph.js.ast_builder import TSAstBuilder
        from morph.js.codegen import TSToCppTranslator
        tree = _JS_PARSER.parse(js_source.encode("utf-8"))
        arrow_fn_node = tree.root_node.children[0].children[0]
        builder = TSAstBuilder()
        arrow_fn = builder.build_expression(arrow_fn_node)
        translator = TSToCppTranslator(indent_level=0,
                                       event_handler=event_handler,
                                       state_vars=state_vars_map or {})
        translator._fn_body_depth = 1
        translator.translate(arrow_fn)
        return None
    except Exception as e:
        return f"{type(e).__name__}: {e}"


def _transpile_statement(js_source: str, state_vars_map: dict[str, str]) -> str | None:
    """Compile a whole statement the way IRBuilder does for top-level
    functions, global variables and inner functions (builder.py:290-377)."""
    if not js_source.strip():
        return None
    try:
        from morph.js.ast_builder import TSAstBuilder
        from morph.js.codegen import TSToCppTranslator
        tree = _JS_PARSER.parse(js_source.encode("utf-8"))
        builder = TSAstBuilder()
        stmt = builder.build_statement(tree.root_node.children[0])
        translator = TSToCppTranslator(indent_level=0,
                                       state_vars=state_vars_map or {})
        translator.translate(stmt)
        return None
    except Exception as e:
        return f"{type(e).__name__}: {e}"


# ── Check context & main runner ──────────────────────────────────────

class CheckContext:
    def __init__(self, source: str, ast: Node, walked: dict,
                 file_path: str | None, config):
        self.source = source
        self.source_lines = source.split("\n")
        self.ast = ast
        self.walked = walked
        self.file_path = file_path
        self.config = config

        lint = getattr(config, "lint", None) if config is not None else None
        self.disabled: set[str] = set(getattr(lint, "disable", None) or [])
        self.severity_overrides: dict[str, str] = dict(
            getattr(lint, "severities", None) or {})

        # State getters of the exported component (for effect deps + transpile).
        getters: set[str] = set()
        state_map: dict[str, str] = {}
        for comp in walked.get("components", []):
            if not comp.get("exported", False):
                continue
            for sv in comp.get("state_vars", []):
                g, s = sv.get("getter", ""), sv.get("setter", "")
                if g:
                    getters.add(g)
                    state_map[g] = f"__st_{g}.get()"
                if s:
                    state_map[s] = f"__st_{s}.set"
        self.state_getters = getters
        self.state_vars_map = state_map

        # CSS classes defined in imported stylesheets — className tokens that
        # match these are custom CSS classes, not Tailwind utilities.
        self.css_classes: set[str] = set()
        base = Path(file_path).parent if file_path else Path(".")
        for imp in walked.get("imports", []):
            if imp.get("type") != "css_local":
                continue
            path = base / imp["path"]
            if not path.exists():
                continue
            try:
                css = path.read_text(encoding="utf-8")
            except OSError:
                continue
            self.css_classes |= _extract_css_classes(css)

    def err(self, code: str, message: str, node: Node,
            suggestion: str | None = None,
            file_path: str | None = None,
            source_lines: list[str] | None = None) -> LintError:
        line, col = _pos(node)
        return LintError(
            R.DEFAULT_SEVERITIES.get(code, "error"), code, message,
            file_path=file_path or self.file_path, line=line, col=col,
            suggestion=suggestion,
            source_lines=source_lines or self.source_lines)


class Checker:
    """Run all lint rules over a parsed .mx file."""

    def run(self, source: str, ast: Node, walked: dict,
            file_path: str | None = None, config=None) -> list[LintError]:
        ctx = CheckContext(source, ast, walked, file_path, config)
        errors: list[LintError] = []
        for fn in _RULES:
            try:
                errors.extend(fn(ctx) or [])
            except Exception:
                # A buggy rule must never take down the pipeline.
                continue

        kept: list[LintError] = []
        for e in errors:
            if e.code in ctx.disabled:
                continue
            if e.code in ctx.severity_overrides:
                e.severity = ctx.severity_overrides[e.code]
            kept.append(e)
        kept.sort(key=lambda e: (e.line, e.col,
                                 0 if e.severity == "error" else 1))
        return kept


# ── Rules ────────────────────────────────────────────────────────────

def _rule_export(ctx: CheckContext) -> list[LintError]:
    """mx-export: exactly one `export default function` component."""
    defaults = []
    for es in _find_all(ctx.ast, "export_statement"):
        fn = _first_child(es, "function_declaration")
        if fn is not None:
            defaults.append((es, fn))
    if not defaults:
        return [ctx.err("mx-export",
                        "No default exported component found — add "
                        "`export default function App() { ... }`",
                        ctx.ast)]
    out = []
    for es, fn in defaults[1:]:
        out.append(ctx.err("mx-export",
                           "Multiple default-exported components — keep "
                           "exactly one `export default function`",
                           es))
    return out


def _rule_component_name(ctx: CheckContext) -> list[LintError]:
    """mx-component-name: default export must be a named function."""
    out = []
    for es in _find_all(ctx.ast, "export_statement"):
        fn = _first_child(es, "function_declaration")
        if fn is None:
            continue
        if _first_child(fn, "identifier") is None:
            out.append(ctx.err(
                "mx-component-name",
                "`export default` must be a named function, e.g. "
                "`export default function App() { ... }`", fn))
    return out


def _has_windowconfig(ctx: CheckContext) -> Node | None:
    for es in _find_all(ctx.ast, "export_statement"):
        for decl in _find_all(es, "variable_declarator"):
            name = ""
            obj = None
            for child in decl.children:
                if child.type == "identifier":
                    name = _text(child)
                elif child.type == "object":
                    obj = child
            if name == "windowConfig" and obj is not None:
                return obj
    return None


def _has_morph_window(ctx: CheckContext) -> Node | None:
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        if _tag_of(el) == "morph-window":
            return el
    return None


def _rule_window_conflict(ctx: CheckContext) -> list[LintError]:
    """mx-window-conflict: windowConfig export + <morph-window> both present."""
    win_el = _has_morph_window(ctx)
    if win_el is not None and _has_windowconfig(ctx) is not None:
        return [ctx.err("mx-window-conflict",
                        "Use either `<morph-window>` or `export const "
                        "windowConfig`, not both (windowConfig takes "
                        "precedence)", win_el)]
    return []


def _rule_window_missing(ctx: CheckContext) -> list[LintError]:
    """mx-window-missing: component renders nothing without a window."""
    if _has_windowconfig(ctx) is not None or _has_morph_window(ctx) is not None:
        return []
    jsx_els = _find_all(ctx.ast, "jsx_element") + _find_all(
        ctx.ast, "jsx_self_closing_element")
    if not jsx_els:
        return []
    anchor = jsx_els[0]
    return [ctx.err("mx-window-missing",
                    "No window is created — render a `<morph-window>` root "
                    "or export `windowConfig` (title/width/height)",
                    anchor)]


def _rule_windowconfig(ctx: CheckContext) -> list[LintError]:
    """mx-windowconfig-key / mx-windowconfig-type."""
    obj = _has_windowconfig(ctx)
    if obj is None:
        return []
    out = []
    for pair in _find_all(obj, "pair"):
        key_node = _first_child(pair, "property_identifier")
        if key_node is None:
            continue
        key = _text(key_node)
        val = _first_child(pair, "string", "number")
        if key not in R.WINDOW_CONFIG_KEYS:
            out.append(ctx.err(
                "mx-windowconfig-key",
                f"Unknown windowConfig key '{key}'",
                key_node,
                suggestion="Allowed: " + ", ".join(sorted(R.WINDOW_CONFIG_KEYS))))
            continue
        if key == "title":
            if val is None or val.type != "string":
                out.append(ctx.err(
                    "mx-windowconfig-type",
                    f"windowConfig.title must be a string", key_node))
        elif key in R.WINDOW_SIZE_PROPS:
            if val is None or val.type != "number":
                out.append(ctx.err(
                    "mx-windowconfig-type",
                    f"windowConfig.{key} must be a number", key_node))
    return out


def _rule_window_props(ctx: CheckContext) -> list[LintError]:
    """mx-window-prop / mx-window-prop-type on <morph-window>."""
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        if _tag_of(el) != "morph-window":
            continue
        for name, attr_node, value in _attrs_of(el):
            if name not in R.WINDOW_PROPS:
                close = difflib.get_close_matches(name, R.WINDOW_PROPS, n=1)
                hint = f" Did you mean '{close[0]}'?" if close else ""
                out.append(ctx.err(
                    "mx-window-prop",
                    f"Unknown prop '{name}' on <morph-window>", attr_node,
                    suggestion="Allowed: " + ", ".join(sorted(R.WINDOW_PROPS)) + hint))
                continue
            if name in R.WINDOW_SIZE_PROPS and value is not None:
                inner = _first_child(value, "number", "string") if value.type in _EXPR_WRAPPER_TYPES else value
                is_num = inner is not None and (
                    inner.type == "number"
                    or (inner.type == "string" and _text(inner).strip("\"'").isdigit()))
                if not is_num:
                    out.append(ctx.err(
                        "mx-window-prop-type",
                        f"<morph-window> {name} must be a number", attr_node))
    return out


def _rule_tags(ctx: CheckContext) -> list[LintError]:
    """mx-tag / mx-tag-stub."""
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        tag = _tag_of(el)
        if not tag:
            continue
        if tag in R.STUB_TAGS:
            out.append(ctx.err(
                "mx-tag-stub",
                f"<{tag}> is registered but not fully implemented in the "
                "runtime yet", el))
            continue
        if tag in R.SUPPORTED_TAGS:
            continue
        if tag[0].isupper():
            suggestion = ("Custom components are not supported yet — use a "
                          "built-in HTML element instead")
        else:
            suggestion = ("Supported elements: div, span, p, h1-h6, button, "
                          "img, a, ul/ol/li, table, morph-window, ...")
        out.append(ctx.err("mx-tag",
                           f"Unknown element <{tag}>", el,
                           suggestion=suggestion))
    return out


def _allowed_props_for(tag: str) -> frozenset[str]:
    allowed = R.GLOBAL_PROPS | R.EVENT_PROPS | R.MORPH_ACTIONS
    return allowed | R.TAG_PROPS.get(tag, frozenset())


def _rule_props(ctx: CheckContext) -> list[LintError]:
    """mx-prop / mx-img-src / mx-dup-class / mx-key-misuse."""
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        tag = _tag_of(el)
        if not tag:
            continue
        allowed = _allowed_props_for(tag)
        names = [a[0] for a in _attrs_of(el)]
        for name, attr_node, value in _attrs_of(el):
            if name == "key":
                if not _inside_map_callback(attr_node):
                    out.append(ctx.err(
                        "mx-key-misuse",
                        "`key` is only meaningful on the root element of a "
                        ".map() list item", attr_node))
                continue
            if name in allowed:
                continue
            close = difflib.get_close_matches(name, allowed, n=1)
            suggestion = f" Did you mean '{close[0]}'?" if close else ""
            if name.startswith("on"):
                suggestion += (" Event props: onClick, onDoubleClick, "
                               "onMouseDown/Up/Enter/Leave, onKeyUp/Down")
            if name.startswith("morph-"):
                suggestion += (" morph-* props: morph-open, morph-close, "
                               "morph-navigate")
            out.append(ctx.err(
                "mx-prop",
                f"Prop '{name}' is not supported on <{tag}>", attr_node,
                suggestion=suggestion))
        if tag == "img" and "src" not in names:
            out.append(ctx.err("mx-img-src",
                               "<img> requires a `src` attribute", el))
        if "className" in names and "class" in names:
            out.append(ctx.err(
                "mx-dup-class",
                "Use `className` or `class`, not both", el))
    return out


def _rule_event_values(ctx: CheckContext) -> list[LintError]:
    """mx-event-value: handlers must be an arrow function or function name."""
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        for name, attr_node, value in _attrs_of(el):
            if name not in R.EVENT_PROPS or value is None:
                continue
            inner = _named_children(value) if value.type in _EXPR_WRAPPER_TYPES else [value]
            if any(c.type in ("arrow_function", "identifier") for c in inner):
                continue
            if _is_jsx_comment(value):
                continue
            out.append(ctx.err(
                "mx-event-value",
                f"{name} must be a function — e.g. "
                f"{name}={{\"() => ...\"}} or a named function reference",
                attr_node))
    return out


def _rule_morph_actions(ctx: CheckContext) -> list[LintError]:
    """mx-morph-action: morph-open/-close/-navigate need a static string."""
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        for name, attr_node, value in _attrs_of(el):
            if name not in R.MORPH_ACTIONS:
                continue
            if value is not None and value.type == "string":
                continue
            out.append(ctx.err(
                "mx-morph-action",
                f"{name} must be a static string, e.g. "
                f"{name}=\"https://example.com\"", attr_node))
    return out


def _style_attr(el: Node) -> tuple[str, Node, Node | None] | None:
    for name, attr_node, value in _attrs_of(el):
        if name == "style":
            return name, attr_node, value
    return None


def _rule_styles(ctx: CheckContext) -> list[LintError]:
    """mx-style-prop / mx-style-value on inline style={{...}} objects."""
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        got = _style_attr(el)
        if got is None:
            continue
        _, attr_node, value = got
        if value is None or value.type not in _EXPR_WRAPPER_TYPES:
            continue
        obj = _first_child(value, "object")
        if obj is None:
            continue
        for pair in _find_all(obj, "pair"):
            key_node = _first_child(pair, "property_identifier", "string")
            if key_node is None:
                continue
            key = _text(key_node).strip("\"'")
            css_key = re.sub(r"(?<!^)(?=[A-Z])", "-", key).lower()
            if css_key not in R.CSS_PROPERTIES:
                close = difflib.get_close_matches(css_key, R.CSS_PROPERTIES, n=1)
                hint = f" Did you mean '{close[0]}'?" if close else ""
                out.append(ctx.err(
                    "mx-style-prop",
                    f"Unsupported CSS property '{css_key}' in style", key_node,
                    suggestion=hint))
                continue
            val_node = _first_child(pair, "string")
            if val_node is not None and css_key in R.CSS_VALUE_SETS:
                val = _text(val_node).strip("\"'")
                if val not in R.CSS_VALUE_SETS[css_key]:
                    out.append(ctx.err(
                        "mx-style-value",
                        f"Invalid value '{val}' for '{css_key}' — allowed: "
                        + ", ".join(sorted(R.CSS_VALUE_SETS[css_key])),
                        val_node))
    return out


def _rule_tailwind(ctx: CheckContext) -> list[LintError]:
    """mx-tailwind-class: className tokens that resolve to nothing."""
    from morph.style.tailwind import TailwindResolver
    try:
        resolver = TailwindResolver(project_root=os.getcwd())
    except Exception:
        return []
    if getattr(resolver, "_mode", "static") != "static":
        return []  # full Tailwind installed — can't lint without running it
    out = []
    for el in _find_all(ctx.ast, "jsx_element") + _find_all(ctx.ast, "jsx_self_closing_element"):
        for name, attr_node, value in _attrs_of(el):
            if name not in ("className", "class") or value is None:
                continue
            if value.type != "string":
                continue
            classes = _text(value).strip("\"'").split()
            for cls in classes:
                if cls in ctx.css_classes:
                    continue  # custom CSS class defined in a stylesheet
                try:
                    if resolver._resolve_one(cls):
                        continue
                except Exception:
                    continue
                out.append(ctx.err(
                    "mx-tailwind-class",
                    f"Unknown Tailwind utility class '{cls}' — it will be "
                    "silently skipped", attr_node,
                    suggestion="Check the class name or use a CSS rule"))
    return out


def _rule_css_imports(ctx: CheckContext) -> list[LintError]:
    """mx-css-file-missing / mx-css-prop on imported stylesheets."""
    out = []
    base = Path(ctx.file_path).parent if ctx.file_path else Path(".")
    seen: set[Path] = set()
    for imp in ctx.walked.get("imports", []):
        if imp.get("type") not in ("css_local",):
            continue
        path = (base / imp["path"]).resolve()
        if not path.exists():
            out.append(LintError(
                R.DEFAULT_SEVERITIES["mx-css-file-missing"],
                "mx-css-file-missing",
                f"CSS file not found: {imp['path']}",
                file_path=ctx.file_path,
                line=1, col=1,
                source_lines=ctx.source_lines))
            continue
        if path in seen:
            continue
        seen.add(path)
        out.extend(_lint_css_file(ctx, path))
    return out


def _lint_css_file(ctx: CheckContext, path: Path) -> list[LintError]:
    try:
        css = path.read_text(encoding="utf-8")
    except OSError:
        return []
    lines = css.split("\n")
    tree = _CSS_PARSER.parse(css.encode("utf-8"))
    out = []
    for decl in _find_all(tree.root_node, "declaration"):
        prop_node = _first_child(decl, "property_name")
        if prop_node is None:
            continue
        prop = _text(prop_node).strip()
        if prop and prop not in R.CSS_PROPERTIES:
            line, col = _pos(prop_node)
            out.append(LintError(
                R.DEFAULT_SEVERITIES["mx-css-prop"], "mx-css-prop",
                f"Unsupported CSS property '{prop}'", file_path=str(path),
                line=line, col=col, source_lines=lines))
    return out


def _rule_imports(ctx: CheckContext) -> list[LintError]:
    """mx-import-morph / mx-import-missing / mx-import-type."""
    out = []
    base = Path(ctx.file_path).parent if ctx.file_path else Path(".")
    for imp_stmt in _find_all(ctx.ast, "import_statement"):
        source_node = _first_child(imp_stmt, "string")
        if source_node is None:
            continue
        source = _text(source_node).strip("\"'")
        kind = R.import_kind(source)
        if kind == "morph":
            for spec in _find_all(imp_stmt, "import_specifier"):
                ident = _first_child(spec, "identifier")
                if ident is None:
                    continue
                name = _text(ident)
                if name not in R.MORPH_BUILTINS:
                    out.append(ctx.err(
                        "mx-import-morph",
                        f"'{name}' is not exported by 'morph' — available: "
                        + ", ".join(sorted(R.MORPH_BUILTINS)),
                        ident))
        elif kind in ("css", "cpp"):
            if not (base / source).exists():
                out.append(ctx.err(
                    "mx-import-missing",
                    f"Imported file not found: {source}", source_node))
        elif kind == "other":
            out.append(ctx.err(
                "mx-import-type",
                f"Import '{source}' is not resolved by morph — only .css, "
                ".cpp files and 'morph' are supported", source_node))
    return out


def _rule_state_pattern(ctx: CheckContext) -> list[LintError]:
    """mx-state-pattern: morphState must be `const [getter, setter] = ...`."""
    out = []
    for call in _find_all(ctx.ast, "call_expression"):
        if _call_parts(call) != ["morphState"]:
            continue
        p = call.parent
        ok = False
        if p is not None and p.type == "variable_declarator":
            pattern = _first_child(p, "array_pattern")
            if pattern is not None:
                ids = [c for c in pattern.children if c.type == "identifier"]
                if len(ids) >= 2:
                    ok = True
        if not ok:
            out.append(ctx.err(
                "mx-state-pattern",
                "morphState must be destructured as "
                "`const [getter, setter] = morphState(initial)`", call))
    return out


def _rule_effect(ctx: CheckContext) -> list[LintError]:
    """mx-effect-cb / mx-effect-deps."""
    out = []
    for call in _find_all(ctx.ast, "call_expression"):
        if _call_parts(call) != ["morphEffect"]:
            continue
        args = _args_of(call)
        if not args:
            continue
        cb = args[0]
        if cb.type not in ("arrow_function", "function_expression"):
            out.append(ctx.err(
                "mx-effect-cb",
                "morphEffect's first argument must be a function, e.g. "
                "`morphEffect(() => { ... })`", cb))
        if len(args) > 1:
            deps_node = args[1]
            for ident in _find_all(deps_node, "identifier"):
                name = _text(ident)
                if name not in ctx.state_getters:
                    out.append(ctx.err(
                        "mx-effect-deps",
                        f"Effect dependency '{name}' is not a state variable "
                        "— the effect will not re-run on its changes", ident))
    return out


def _rule_list_key(ctx: CheckContext) -> list[LintError]:
    """mx-list-key: .map() item roots should carry a key prop."""
    out = []
    for call, arrow, _params in _map_callbacks(ctx.ast):
        body = _arrow_body_jsx(arrow)
        if body is None:
            continue
        names = [a[0] for a in _attrs_of(body)]
        if "key" not in names:
            out.append(ctx.err(
                "mx-list-key",
                "List item rendered with .map() has no `key` prop — "
                "add `key={...}` to the root element for stable re-renders",
                body))
    return out


def _rule_transpile(ctx: CheckContext) -> list[LintError]:
    """mx-transpile: JS that the C++ translator cannot compile."""
    out = []
    svm = ctx.state_vars_map

    def _fail(code: str, node: Node, js_source: str, msg: str) -> None:
        out.append(ctx.err(
            "mx-transpile",
            f"JS cannot be compiled to C++: {msg}",
            node, suggestion=f"Expression: {js_source[:80]}"))

    # ── JSX expression containers (text, style, class, conditionals) ──
    for wrapper in _find_all(ctx.ast, "jsx_expression") + _find_all(
            ctx.ast, "jsx_expression_container"):
        if _is_jsx_comment(wrapper):
            continue
        # Containers that embed JSX (conditionals / .map) are handled below.
        if _find_all(wrapper, "jsx_element") or _find_all(wrapper, "jsx_self_closing_element"):
            continue
        arrow = _first_child(wrapper, "arrow_function")
        if arrow is not None:
            msg = _transpile_arrow(_text(arrow), svm)
            if msg:
                _fail("mx-transpile", arrow, _text(arrow), msg)
            continue
        obj = _first_child(wrapper, "object")
        if obj is not None:
            for pair in _find_all(obj, "pair"):
                for expr_node in pair.children:
                    if expr_node.type in ("template_string", "identifier",
                                          "binary_expression", "call_expression",
                                          "member_expression", "unary_expression",
                                          "ternary_expression"):
                        msg = _transpile_expr(_text(expr_node), svm)
                        if msg:
                            _fail("mx-transpile", expr_node,
                                  _text(expr_node), msg)
            continue
        inner = _expr_inner(wrapper)
        if not inner:
            continue
        msg = _transpile_expr(inner, svm)
        if msg:
            _fail("mx-transpile", wrapper, inner, msg)

    # ── Conditional expressions ({cond && <JSX/>}, ternary) ──
    for wrapper in _find_all(ctx.ast, "jsx_expression") + _find_all(
            ctx.ast, "jsx_expression_container"):
        if _is_jsx_comment(wrapper):
            continue
        for expr in wrapper.children:
            if expr.type == "binary_expression":
                left = None
                for child in expr.children:
                    if child.type in ("&&", "||"):
                        break
                    if child.is_named and left is None:
                        left = child
                if left is not None:
                    msg = _transpile_expr(_text(left), svm)
                    if msg:
                        _fail("mx-transpile", left, _text(left), msg)
            elif expr.type == "ternary_expression":
                cond = None
                for child in expr.children:
                    if child.type in ("?", ":"):
                        break
                    if child.is_named and cond is None:
                        cond = child
                if cond is not None:
                    msg = _transpile_expr(_text(cond), svm)
                    if msg:
                        _fail("mx-transpile", cond, _text(cond), msg)

    # ── .map() array expressions and key props ──
    for call, arrow, params in _map_callbacks(ctx.ast):
        obj_node = _first_child(call, "member_expression")
        if obj_node is not None and obj_node.children:
            arr = obj_node.children[0]
            msg = _transpile_expr(_text(arr), svm)
            if msg:
                _fail("mx-transpile", arr, _text(arr), msg)
        item_map = dict(svm)
        if params:
            item_map[params[0]] = "__it"
        if len(params) > 1:
            item_map[params[1]] = "__index"
        body = _arrow_body_jsx(arrow)
        if body is not None:
            for name, _attr, value in _attrs_of(body):
                if name != "key" or value is None:
                    continue
                key_src = _expr_inner(value) if value.type in _EXPR_WRAPPER_TYPES else ""
                if not key_src:
                    continue
                msg = _transpile_expr(key_src, item_map)
                if msg:
                    _fail("mx-transpile", value, key_src, msg)

    # ── Top-level function declarations (builder.py:290-305) ──
    for fd in ctx.walked.get("function_declarations", []):
        src = fd.get("source", "")
        node = _find_node_text(ctx.ast, ("function_declaration",), src)
        if node is None or not src:
            continue
        msg = _transpile_statement(src, {})
        if msg:
            _fail("mx-transpile", node, src, msg)

    # ── Module-level variable declarations (builder.py:307-322) ──
    for gv in ctx.walked.get("global_vars", []):
        src = gv.get("source", "")
        node = _find_node_text(ctx.ast, ("lexical_declaration",), src)
        if node is None or not src:
            continue
        msg = _transpile_statement(src, {})
        if msg:
            _fail("mx-transpile", node, src, msg)

    # ── Component-level consts / inner functions / effects ──
    for comp in ctx.walked.get("components", []):
        if not comp.get("exported", False):
            continue
        block = _component_block(ctx, comp)
        if block is None:
            continue
        for cd in comp.get("consts", []):
            rhs = cd.get("rhs", "")
            decl = _find_declarator_by_name(block, cd.get("name", ""))
            if decl is None or not rhs:
                continue
            rhs_node = _find_child_by_text(decl, rhs)
            msg = _transpile_expr(rhs, svm)
            if msg:
                _fail("mx-transpile", rhs_node or decl, rhs, msg)
        for fn in comp.get("inner_functions", []):
            src = fn.get("source", "")
            node = _find_node_text(block, ("function_declaration",
                                           "lexical_declaration"), src)
            if node is None or not src:
                continue
            msg = _transpile_statement(src, svm)
            if msg:
                _fail("mx-transpile", node, src, msg)
        for ef in comp.get("morph_effects", []):
            cb = ef.get("callback", "")
            call = _find_effect_call(block, cb)
            if call is None or not cb:
                continue
            msg = _transpile_arrow(cb, svm, event_handler=False)
            if msg:
                _fail("mx-transpile", call, cb, msg)

    return out


def _find_node_text(root: Node, types: tuple[str, ...], text: str) -> Node | None:
    """First node of any of ``types`` whose exact text equals ``text``."""
    if not text:
        return None
    for t in types:
        for node in _find_all(root, t):
            if _text(node) == text:
                return node
    return None


def _component_block(ctx: CheckContext, comp: dict) -> Node | None:
    """statement_block of the (exported) component function by name."""
    name = comp.get("name", "")
    for node in _find_all(ctx.ast, "function_declaration"):
        if _first_child(node, "identifier") is None:
            continue
        if _text(_first_child(node, "identifier")) != name:
            continue
        block = _first_child(node, "statement_block")
        if block is not None:
            return block
    return None


def _find_declarator_by_name(block: Node, name: str) -> Node | None:
    for decl in _find_all(block, "variable_declarator"):
        ident = _first_child(decl, "identifier")
        if ident is not None and _text(ident) == name:
            return decl
    return None


def _find_child_by_text(node: Node, text: str) -> Node | None:
    for child in node.children:
        if child.is_named and _text(child) == text:
            return child
        got = _find_child_by_text(child, text)
        if got is not None:
            return got
    return None


def _find_effect_call(block: Node, callback: str) -> Node | None:
    """First morphEffect(...) call in ``block`` whose first arg equals callback."""
    for call in _find_all(block, "call_expression"):
        if _call_parts(call) != ["morphEffect"]:
            continue
        args = _args_of(call)
        if args and _text(args[0]) == callback:
            return args[0]
    return None


def _is_morphstate_pattern(node: Node) -> bool:
    """True if the pattern is part of `const [g, s] = morphState(...)` —
    the one destructuring the builder handles at IR level."""
    decl = node.parent
    if decl is None or decl.type != "variable_declarator":
        return False
    value = None
    for child in decl.children:
        if child.is_named and child != node:
            value = child
            break
    if value is None or value.type != "call_expression":
        return False
    fn = _first_child(value, "identifier")
    return fn is not None and _text(fn) == "morphState"


def _in_type_context(node: Node) -> bool:
    """True if the node sits inside a type position (annotations, generics…)."""
    cur = node.parent
    while cur is not None:
        if cur.type in _TYPE_NODES:
            return True
        cur = cur.parent
    return False


def _declared_names(ctx: CheckContext) -> set[str]:
    """Every identifier declared anywhere in the file (params, consts,
    functions, imports, state getters/setters, map-callback params…).

    Only *declaration* positions are collected — initializer expressions
    are skipped so `const rnd = Math.random()` does not "declare" Math.
    """
    names: set[str] = set()

    def _pattern_names(decl: Node) -> set[str]:
        got: set[str] = set()
        for idn in _find_all(decl, "identifier"):
            got.add(_text(idn))
        for idn in _find_all(decl, "shorthand_property_identifier_pattern"):
            got.add(_text(idn))
        return got

    for imp in _find_all(ctx.ast, "import_statement"):
        for idn in _find_all(imp, "identifier"):
            names.add(_text(idn))
    for vd in _find_all(ctx.ast, "variable_declarator"):
        if vd.children:
            names |= _pattern_names(vd.children[0])
    for fd in _find_all(ctx.ast, "function_declaration") + \
            _find_all(ctx.ast, "generator_function_declaration"):
        ident = _first_child(fd, "identifier")
        if ident is not None:
            names.add(_text(ident))
    for par in _find_all(ctx.ast, "required_parameter") + _find_all(
            ctx.ast, "optional_parameter"):
        for idn in _find_all(par, "identifier"):
            names.add(_text(idn))
    for cd in _find_all(ctx.ast, "class_declaration"):
        for idn in _find_all(cd, "identifier"):
            names.add(_text(idn))
        for md in _find_all(cd, "method_definition"):
            for idn in _find_all(md, "property_identifier"):
                names.add(_text(idn))
    names.update(ctx.state_vars_map)
    return names


def _member_base(node: Node) -> Node | None:
    """If node is the object (base) of a member_expression, return it."""
    p = node.parent
    if p is not None and p.type == "member_expression" and p.children and \
            p.children[0] == node:
        return node
    return None


def _inside_jsx_expr(node: Node) -> bool:
    """True if node is inside a JSX expression container — the .map() and
    conditionals there are handled by the IR builder, not the translator."""
    cur = node.parent
    while cur is not None:
        if cur.type in _EXPR_WRAPPER_TYPES:
            return True
        cur = cur.parent
    return False


def _rule_js_globals(ctx: CheckContext) -> list[LintError]:
    """mx-js-global: browser/JS globals with no native counterpart."""
    out: list[LintError] = []
    declared = _declared_names(ctx)
    for idn in _find_all(ctx.ast, "identifier"):
        name = _text(idn)
        if name not in _UNSUPPORTED_GLOBALS:
            continue
        if name in declared or _in_type_context(idn):
            continue
        if _member_base(idn) is not None and name in _MEMBER_BASES_UNSUPPORTED:
            continue  # reported by mx-js-member with a more specific message
        out.append(ctx.err(
            "mx-js-global",
            f"'{name}' ({_UNSUPPORTED_GLOBALS[name]}) is a browser/JS global "
            "not available in the native runtime",
            idn,
            suggestion="Available instead: morphState/morphEffect/morphLog/"
                       "morphEmit, CSS, fetch(), setTimeout/setInterval, "
                       "console.log/warn/error/info"))
    return out


def _rule_js_members(ctx: CheckContext) -> list[LintError]:
    """mx-js-member: member access on unsupported JS builtins."""
    out: list[LintError] = []
    declared = _declared_names(ctx)
    for mem in _find_all(ctx.ast, "member_expression"):
        if not mem.children:
            continue
        base = mem.children[0]
        if base.type != "identifier":
            continue
        base_name = _text(base)
        prop = mem.children[-1]
        prop_name = _text(prop) if prop.type in (
            "property_identifier", "identifier") else ""
        if base_name in declared:
            continue
        if base_name in _MEMBER_BASES_UNSUPPORTED:
            out.append(ctx.err(
                "mx-js-member",
                f"{base_name}.{prop_name or '…'} is not supported — "
                f"{_MEMBER_BASES_UNSUPPORTED[base_name]}",
                mem, suggestion=f"Expression: {_text(mem)[:80]}"))
        elif base_name == "console" and prop_name and \
                prop_name not in _CONSOLE_METHODS:
            out.append(ctx.err(
                "mx-js-member",
                f"console.{prop_name} is not supported — only "
                "console.log/warn/error/info map to std::println",
                mem, suggestion=f"Expression: {_text(mem)[:80]}"))
        elif base_name in _JS_STATIC_METHODS and prop_name and \
                prop_name in _JS_STATIC_METHODS[base_name]:
            hint = _JS_STATIC_METHODS[base_name][prop_name]
            out.append(ctx.err(
                "mx-js-member",
                f"{base_name}.{prop_name} is not available in the native runtime",
                mem, suggestion=f"Instead: {hint}"))
    return out


def _rule_js_methods(ctx: CheckContext) -> list[LintError]:
    """mx-js-method: methods the runtime types do not implement."""
    out: list[LintError] = []
    for call in _find_all(ctx.ast, "call_expression"):
        if _inside_jsx_expr(call):
            continue  # .map() etc. in JSX are handled by the IR builder
        callee = _first_child(call, "member_expression")
        if callee is None or not callee.children:
            continue
        base, prop = callee.children[0], callee.children[-1]
        if prop.type not in ("property_identifier", "identifier"):
            continue
        method = _text(prop)
        if base.type == "string":
            out.append(ctx.err(
                "mx-js-method",
                f"'{method}()' on a string literal is not supported in native — "
                "assign the string to a JsString variable first "
                f"(JsString supports: {', '.join(sorted(_JSSTRING_METHODS))})",
                call, suggestion=f"Expression: {_text(call)[:80]}"))
            continue
        if method not in _JS_ONLY_METHODS:
            continue
        if base.type == "identifier" and _text(base) in ctx.state_getters:
            out.append(ctx.err(
                "mx-js-method",
                f"'{method}()' is not implemented by state values — "
                f"state values support: {', '.join(sorted(_STATE_METHODS))}",
                call, suggestion=f"Expression: {_text(call)[:80]}"))
    return out


def _rule_js_ops(ctx: CheckContext) -> list[LintError]:
    """mx-js-op: operators the translator cannot emit."""
    out: list[LintError] = []
    for node in _find_all(ctx.ast, "binary_expression") + \
            _find_all(ctx.ast, "unary_expression") + \
            _find_all(ctx.ast, "assignment_expression"):
        for child in node.children:
            if child.type in _UNSUPPORTED_OPS:
                out.append(ctx.err(
                    "mx-js-op",
                    f"the '{child.type}' operator is not supported by the "
                    "C++ translator",
                    child, suggestion=_UNSUPPORTED_OPS[child.type]))
                break
    return out


def _rule_js_syntax(ctx: CheckContext) -> list[LintError]:
    """mx-js-syntax: JS constructs the translator cannot handle."""
    out: list[LintError] = []

    def _fail(node: Node, what: str, suggestion: str | None = None) -> None:
        out.append(ctx.err(
            "mx-js-syntax", f"{what} is not supported by the C++ translator",
            node, suggestion=suggestion))

    for node in _find_all(ctx.ast, "optional_chain"):
        _fail(node, "optional chaining ('?.')",
              "The null check is silently dropped — use an explicit "
              "if/ternary check instead")
    for node in _find_all(ctx.ast, "generator_function_declaration") + \
            _find_all(ctx.ast, "generator_function"):
        _fail(node, "generator functions (function*)")
    for node in _find_all(ctx.ast, "yield_expression"):
        _fail(node, "'yield'")
    for node in _find_all(ctx.ast, "for_in_statement") + \
            _find_all(ctx.ast, "for_of_statement"):
        _fail(node, "for...in / for...of loops",
              "Use a regular index-based for loop or .map() instead")
    for node in _find_all(ctx.ast, "debugger_statement"):
        _fail(node, "'debugger' statements")
    for node in _find_all(ctx.ast, "object_pattern") + \
            _find_all(ctx.ast, "array_pattern"):
        if _is_morphstate_pattern(node):
            continue
        _fail(node, "destructuring",
              "Declare each variable separately, e.g. "
              "const a = obj.a; const b = obj.b")
    for node in _find_all(ctx.ast, "spread_element"):
        parent = node.parent
        if parent is not None and parent.type == "arguments":
            _fail(node, "spread in call arguments",
                  "Pass the arguments explicitly")
        elif parent is not None and parent.type == "object":
            _fail(node, "object spread ('{...obj}')",
                  "Copy the fields explicitly")
    for node in _find_all(ctx.ast, "method_definition") + \
            _find_all(ctx.ast, "getter") + _find_all(ctx.ast, "setter"):
        parent = node.parent
        if parent is not None and parent.type == "object":
            _fail(node, "object literal methods/getters",
                  "Use a plain property with a function value, "
                  "e.g. { fn: () => … }")
    return out


_RULES = [
    _rule_export,
    _rule_component_name,
    _rule_window_conflict,
    _rule_window_missing,
    _rule_windowconfig,
    _rule_window_props,
    _rule_tags,
    _rule_props,
    _rule_event_values,
    _rule_morph_actions,
    _rule_styles,
    _rule_tailwind,
    _rule_css_imports,
    _rule_imports,
    _rule_state_pattern,
    _rule_effect,
    _rule_list_key,
    _rule_transpile,
    _rule_js_globals,
    _rule_js_members,
    _rule_js_methods,
    _rule_js_ops,
    _rule_js_syntax,
]