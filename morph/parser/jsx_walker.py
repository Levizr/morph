from __future__ import annotations
from tree_sitter import Node


class JSXWalker:
    """
    Walks a tree-sitter AST from a .mx file.
    Extracts: imports, components, JSX trees, props, events.
    """

    def walk(self, root: Node) -> dict:
        imports = self._extract_imports(root)
        imports.extend(self._extract_css_load_calls(root))
        components = self._extract_components(root)
        return {
            "imports":      imports,
            "components":   components,
            "windowConfig": self._extract_window_config(root),
            "function_declarations": self._extract_function_declarations(root, components),
            "global_vars":  self._extract_global_vars(root),
        }

    # ── Imports ───────────────────────────────────────────────

    _CPP_EXTS = (".cpp", ".cc", ".cxx", ".c++", ".h", ".hpp", ".hxx")

    def _extract_imports(self, root: Node) -> list[dict]:
        imports = []
        for node in self._find_all(root, "import_statement"):
            source = self._get_import_source(node)
            if not source:
                continue

            if source.startswith("http://") or source.startswith("https://"):
                imports.append({"type": "css_url",   "url":  source, "style": "import"})
            elif source.endswith(".css"):
                imports.append({"type": "css_local", "path": source, "style": "import"})
            elif source.lower().endswith(self._CPP_EXTS):
                # import { foo, bar } from './native.cpp'  or  import './utils.cpp'
                # The .cpp is #included into the generated TU (single-translation
                # unit = maximum inlining / dead-code elimination). Named
                # specifiers are validated at compile time by C++ itself.
                specifiers = self._get_import_specifiers(node)
                imports.append({
                    "type":       "cpp_local",
                    "path":       source,
                    "specifiers": specifiers,
                    "style":      "import",
                })
            else:
                specifiers = self._get_import_specifiers(node)
                imports.append({
                    "type":       "component",
                    "path":       source,
                    "specifiers": specifiers,
                })
        return imports

    def _get_import_source(self, node: Node) -> str:
        for child in node.children:
            if child.type == "string":
                # strip quotes
                return child.text.decode().strip("'\"")
        return ""

    def _get_import_specifiers(self, node: Node) -> list[str]:
        specifiers = []
        for child in node.children:
            if child.type == "import_clause":
                for sub in self._find_all(child, "identifier"):
                    specifiers.append(sub.text.decode())
        return specifiers

    def _extract_css_load_calls(self, root: Node) -> list[dict]:
        entries = []
        for node in self._find_all(root, "call_expression"):
            fn_parts, args = self._parse_call_expr(node)
            if fn_parts != ["CSS", "load"]:
                continue
            if args is None:
                continue
            for child in args.children:
                if child.type == "string":
                    source = child.text.decode().strip("'\"")
                    if source.startswith("http://") or source.startswith("https://"):
                        entries.append({"type": "css_url", "url": source, "style": "css_load"})
                    else:
                        entries.append({"type": "css_local", "path": source, "style": "css_load"})
                    break
        return entries

    def _extract_window_config(self, root: Node) -> dict | None:
        for node in self._find_all(root, "export_statement"):
            for decl in self._find_all(node, "variable_declarator"):
                name = ""
                obj_node = None
                for child in decl.children:
                    if child.type == "identifier":
                        name = child.text.decode()
                    elif child.type == "object":
                        obj_node = child
                if name == "windowConfig" and obj_node is not None:
                    config = {}
                    for pair in self._find_all(obj_node, "pair"):
                        key = ""
                        val = None
                        for part in pair.children:
                            if part.type == "property_identifier":
                                key = part.text.decode()
                            elif part.type == "string":
                                val = part.text.decode().strip("'\"")
                            elif part.type == "number":
                                try:
                                    val = int(part.text.decode())
                                except ValueError:
                                    val = float(part.text.decode())
                        if key and val is not None:
                            config[key] = val
                    return config
        return None

    # ── Components ────────────────────────────────────────────

    def _extract_components(self, root: Node) -> list[dict]:
        components = []

        # export default function App() { ... }
        seen = set()
        for node in self._find_all(root, "export_statement"):
            for fn in self._find_all(node, "function_declaration"):
                comp = self._parse_function_component(fn)
                if comp:
                    comp["exported"] = True
                    components.append(comp)
                    seen.add(id(fn))

        # Non-exported function App() { return (...) }
        for node in self._find_all(root, "function_declaration"):
            if id(node) in seen:
                continue
            comp = self._parse_function_component(node)
            if comp:
                components.append(comp)
        return components

    def _parse_function_component(self, node: Node) -> dict | None:
        name = ""
        params = []
        jsx_root = None
        block_node = None

        for child in node.children:
            if child.type == "identifier":
                name = child.text.decode()
            elif child.type == "formal_parameters":
                params = self._extract_params(child)
            elif child.type == "statement_block":
                block_node = child
                jsx_root = self._find_jsx_in_block(child)

        if not name or jsx_root is None:
            return None

        body_logs = self._extract_body_logs(block_node) if block_node else []
        state_vars = self._extract_state_vars(block_node) if block_node else []
        inner_funcs = self._extract_inner_functions(block_node) if block_node else []
        effects = self._extract_morph_effects(block_node) if block_node else []
        consts = self._extract_component_consts(block_node) if block_node else []

        return {
            "name":           name,
            "params":         params,
            "jsx":            self._parse_jsx_element(jsx_root),
            "body_logs":      body_logs,
            "state_vars":     state_vars,
            "inner_functions": inner_funcs,
            "morph_effects":  effects,
            "consts":         consts,
            "exported":       False,
        }

    def _extract_body_logs(self, block_node: Node) -> list[str]:
        """Find console.log('...') calls before the return statement,
        including inside JSX expressions in the return value."""
        logs = []
        for child in block_node.children:
            if child.type == "return_statement":
                for expr in self._find_all(child, "jsx_expression"):
                    for node in self._find_all(expr, "call_expression"):
                        if self._is_inside(node, "arrow_function"):
                            continue
                        fn_parts, args = self._parse_call_expr(node)
                        if fn_parts == ["console", "log"] and args:
                            for arg in args.children:
                                if arg.type == "string":
                                    logs.append(arg.text.decode().strip("'\""))
                                    break
                break
            for node in self._find_all(child, "call_expression"):
                if self._is_inside(node, "arrow_function"):
                    continue
                fn_parts, args = self._parse_call_expr(node)
                if fn_parts == ["console", "log"] and args:
                    for arg in args.children:
                        if arg.type == "string":
                            logs.append(arg.text.decode().strip("'\""))
                            break
        return logs

    def _extract_state_vars(self, block_node: Node) -> list[dict]:
        """Find morphState() calls in function body outside return/arrow fn."""
        vars = []
        for child in block_node.children:
            if child.type in ("lexical_declaration", "variable_declaration"):
                for decl in child.children:
                    if decl.type != "variable_declarator":
                        continue
                    value_node = None
                    pattern_nodes = []
                    for c in decl.children:
                        if c.type == "array_pattern":
                            pattern_nodes = [p for p in c.children if p.type == "identifier"]
                        elif c.type == "call_expression":
                            value_node = c
                    if not value_node or not pattern_nodes:
                        continue
                    fn_parts, _ = self._parse_call_expr(value_node)
                    if fn_parts == ["morphState"]:
                        getter = pattern_nodes[0].text.decode() if len(pattern_nodes) > 0 else ""
                        setter = pattern_nodes[1].text.decode() if len(pattern_nodes) > 1 else ""
                        init_val = self._extract_init_arg(value_node) or "0"
                        vars.append({
                            "getter": getter,
                            "setter": setter,
                            "init": init_val,
                            "raw": child.text.decode(),
                        })
        return vars

    @staticmethod
    def _extract_init_arg(call_node: Node) -> str | None:
        for child in call_node.children:
            if child.type == "arguments":
                for arg in child.children:
                    if arg.type == "number":
                        return arg.text.decode()
                    if arg.type == "string":
                        return arg.text.decode()
                    if arg.type == "true":
                        return "true"
                    if arg.type == "false":
                        return "false"
                    if arg.type == "identifier":
                        return arg.text.decode()
                    if arg.type in ("array", "object"):
                        return arg.text.decode()
        return None

    def _extract_morph_effects(self, block_node: Node) -> list[dict]:
        """Find morphEffect(fn, [deps]) calls in function body."""
        effects = []
        for call in self._find_all(block_node, "call_expression"):
            if self._is_inside(call, "arrow_function"):
                continue
            if self._is_inside(call, "return_statement"):
                continue
            fn_parts, args_node = self._parse_call_expr(call)
            if fn_parts == ["morphEffect"]:
                if args_node is None:
                    continue
                arg_nodes = [c for c in args_node.children
                             if c.type not in (",", "(", ")")]
                if not arg_nodes:
                    continue
                effect = {"callback": arg_nodes[0].text.decode()}
                if len(arg_nodes) > 1:
                    effect["deps"] = arg_nodes[1].text.decode()
                else:
                    effect["deps"] = ""
                effects.append(effect)
        return effects

    def _extract_global_vars(self, root: Node) -> list[dict]:
        """Find module-level variable declarations (let, const) that are
        NOT inside a component and NOT morphState/morphEffect calls."""
        vars = []
        for decl in self._find_all(root, "lexical_declaration"):
            if self._is_inside(decl, "function_declaration"):
                continue
            if self._is_inside(decl, "export_statement"):
                continue
            # Skip morphState declarations
            is_morph_state = False
            for call in self._find_all(decl, "call_expression"):
                fn_parts, _ = self._parse_call_expr(call)
                if fn_parts == ["morphState"]:
                    is_morph_state = True
                    break
            if is_morph_state:
                continue
            vars.append({"source": decl.text.decode()})
        return vars

    def _is_inside(self, node: Node, ancestor_type: str) -> bool:
        p = node.parent
        while p:
            if p.type == ancestor_type:
                return True
            p = p.parent
        return False

    @staticmethod
    def _parse_call_expr(node: Node) -> tuple[list[str], Node | None]:
        fn_parts = []
        args = None
        for child in node.children:
            if child.type == "member_expression":
                parts = []
                for p in child.children:
                    if p.type in ("identifier", "property_identifier"):
                        parts.append(p.text.decode())
                fn_parts = parts
            elif child.type == "identifier":
                fn_parts = [child.text.decode()]
            elif child.type == "arguments":
                args = child
        return fn_parts, args

    # ── Params (props destructuring) ──────────────────────────

    def _extract_params(self, params_node: Node) -> list[str]:
        params = []
        for child in params_node.children:
            if child.type == "object_pattern":
                # { label, color = '#fff', onClick }
                for item in child.children:
                    if item.type == "shorthand_property_identifier_pattern":
                        params.append(item.text.decode())
                    elif item.type == "assignment_pattern":
                        # color = '#fff'
                        for sub in item.children:
                            if sub.type == "identifier":
                                params.append(sub.text.decode())
                                break
        return params

    # ── JSX ───────────────────────────────────────────────────

    def _find_jsx_in_block(self, block: Node) -> Node | None:
        for node in self._find_all(block, "return_statement"):
            for child in node.children:
                if child.type in ("jsx_element", "jsx_self_closing_element",
                                  "jsx_fragment", "parenthesized_expression"):
                    if child.type == "parenthesized_expression":
                        for sub in child.children:
                            if sub.type in ("jsx_element",
                                            "jsx_self_closing_element",
                                            "jsx_fragment"):
                                return sub
                    return child
        return None

    def _parse_conditional_expr(self, container: Node) -> dict | None:
        """Parse a jsx_expression_container that contains JSX (&& or ternary).
        Returns a __conditional__ node or None."""
        for expr in container.children:
            if expr.type == "binary_expression":
                # count > 5 && <p>Big!</p>
                return self._extract_binary_conditional(expr)
            if expr.type == "ternary_expression":
                # loading ? <Spinner/> : <Content/>
                return self._extract_ternary_conditional(expr)
        return None

    def _parse_map_expression(self, container: Node) -> dict | None:
        """Parse a jsx_expression_container that is a `.map()` list rendering:
        ``{items.map(item => <JSX key={item.id}/>)}``.

        Returns a __list__ node:
          array_expr     — JS source of the array expression (e.g. ``items``)
          item_param     — callback first param name (``item``)
          index_param    — callback second param name (``i``) or ""
          key_expr       — JS source of the root element's `key` prop ("" = index fallback)
          item_template  — parsed JSX of the callback body (key prop removed)

        Returns None when the container is not a map-with-JSX expression
        (caller falls back to a plain __expr__).
        """
        for expr in container.children:
            if expr.type != "call_expression":
                continue
            array_src, prop_name, arrow = self._extract_map_call(expr)
            if array_src is None or arrow is None:
                continue

            # ── Callback params (1 or 2) ──
            params: list[str] = []
            for child in arrow.children:
                if child.type == "identifier" and len(params) < 2:
                    # Single unparenthesized param: item => <JSX/>
                    params.append(child.text.decode())
                elif child.type == "formal_parameters":
                    for sub in self._find_all(child, "required_parameter"):
                        for ident in sub.children:
                            if ident.type == "identifier":
                                params.append(ident.text.decode())
                                break
                    for sub in self._find_all(child, "optional_parameter"):
                        for ident in sub.children:
                            if ident.type == "identifier":
                                params.append(ident.text.decode())
                                break

            # ── Callback body must yield JSX ──
            body = self._arrow_body_jsx(arrow)
            if body is None:
                continue

            template = self._parse_jsx_element(body)
            if not isinstance(template, dict) or not template.get("tag"):
                continue

            # ── `key` prop → key expression (removed from rendered props) ──
            key_expr = ""
            if isinstance(template.get("props"), dict) and template.get("tag") != "__fragment__":
                key_val = template["props"].pop("key", None)
                if key_val is not None:
                    if isinstance(key_val, dict):
                        key_expr = (key_val.get("__ref__") or key_val.get("__expr__")
                                    or key_val.get("__template__") or "")
                    else:
                        key_expr = str(key_val)

            return {
                "tag": "__list__",
                "array_expr": array_src,
                "item_param": params[0] if params else "item",
                "index_param": params[1] if len(params) > 1 else "",
                "key_expr": key_expr,
                "item_template": template,
            }
        return None

    def _extract_map_call(self, node: Node) -> tuple[str | None, str | None, Node | None]:
        """From a call_expression like ``items.map(item => <JSX/>)`` return
        (array_source, method_name, arrow_function_node). None when the call
        is not a member `.map(...)` with an arrow function argument."""
        array_src: str | None = None
        method: str | None = None
        arrow: Node | None = None
        for child in node.children:
            if child.type == "member_expression":
                parts = [c for c in child.children
                         if c.type in ("identifier", "property_identifier")]
                if len(parts) < 2 or parts[-1].text.decode() != "map":
                    return None, None, None
                obj_node = child.children[0]
                if obj_node.type in ("identifier", "member_expression",
                                     "call_expression", "parenthesized_expression"):
                    array_src = obj_node.text.decode()
                method = "map"
            elif child.type == "arguments":
                for arg in child.children:
                    if arg.type == "arrow_function":
                        arrow = arg
                        break
        if array_src is None or arrow is None:
            return None, None, None
        return array_src, method, arrow

    def _arrow_body_jsx(self, arrow: Node) -> Node | None:
        """Extract the JSX body of a map callback arrow function.

        Supports expression bodies (``item => <div/>``), parenthesized bodies,
        fragments, and block bodies with a single ``return <JSX/>``.
        """
        for child in arrow.children:
            if child.type in ("jsx_element", "jsx_self_closing_element", "jsx_fragment"):
                return child
            if child.type == "parenthesized_expression":
                jsx = self._unwrap_jsx(child)
                if jsx is not None:
                    return jsx
            if child.type == "statement_block":
                for stmt in self._find_all(child, "return_statement"):
                    for sub in stmt.children:
                        jsx = self._unwrap_jsx(sub)
                        if jsx is not None:
                            return jsx
        return None

    @staticmethod
    def _unwrap_jsx(node: Node) -> Node | None:
        """If node is or wraps a jsx_element/jsx_self_closing_element/jsx_fragment, return it."""
        if node.type in ("jsx_element", "jsx_self_closing_element", "jsx_fragment"):
            return node
        if node.type == "parenthesized_expression":
            for child in node.children:
                result = JSXWalker._unwrap_jsx(child)
                if result is not None:
                    return result
        return None

    def _extract_binary_conditional(self, node: Node) -> dict | None:
        """Parse `left && <JSX>` into a conditional node."""
        left = None
        right = None
        op = None
        for child in node.children:
            if child.type in ("binary_expression", "identifier",
                              "call_expression", "member_expression",
                              "number", "string", "true", "false",
                              "unary_expression", "parenthesized_expression"):
                if left is None:
                    left = child
                else:
                    right = child
            elif child.type in ("jsx_element", "jsx_self_closing_element"):
                right = child
            elif child.type == "&&" or (hasattr(child, 'type') and child.type == "&&"):
                op = "&&"
            elif child.type.startswith("operator_"):
                op = child.text.decode()
            elif child.is_named and child.type not in ("comment",):
                if left is None:
                    left = child
                else:
                    right = child
        if left is None or right is None:
            return None
        if op not in ("&&", "||"):
            return None
        # The right side must contain JSX (possibly parenthesized)
        jsx_node = self._unwrap_jsx(right)
        if jsx_node is None:
            return None
        cond_source = left.text.decode()
        then_jsx = self._parse_jsx_element(jsx_node)
        if then_jsx is None:
            return None
        return {
            "tag": "__conditional__",
            "condition": cond_source,
            "then_branch": [then_jsx] if isinstance(then_jsx, dict) else then_jsx,
            "else_branch": [],
        }

    def _extract_ternary_conditional(self, node: Node) -> dict | None:
        """Parse `cond ? <A/> : <B/>` into a conditional node."""
        condition = None
        consequent = None
        alternate = None
        for child in node.children:
            if child.type in ("binary_expression", "identifier",
                              "call_expression", "member_expression",
                              "number", "string", "true", "false",
                              "unary_expression", "parenthesized_expression"):
                if condition is None:
                    condition = child
                elif consequent is None:
                    consequent = child
                else:
                    alternate = child
            elif child.type in ("jsx_element", "jsx_self_closing_element"):
                if consequent is None:
                    consequent = child
                else:
                    alternate = child
        if condition is None:
            return None
        cond_source = condition.text.decode()
        then_node = self._unwrap_jsx(consequent) if consequent else None
        else_node = self._unwrap_jsx(alternate) if alternate else None
        then_jsx = self._parse_jsx_element(then_node) if then_node else None
        else_jsx = self._parse_jsx_element(else_node) if else_node else None
        if then_jsx is None and else_jsx is None:
            return None
        result = {"tag": "__conditional__", "condition": cond_source,
                  "then_branch": [], "else_branch": []}
        if then_jsx:
            result["then_branch"] = [then_jsx] if isinstance(then_jsx, dict) else then_jsx
        if else_jsx:
            result["else_branch"] = [else_jsx] if isinstance(else_jsx, dict) else else_jsx
        return result

    def _parse_jsx_element(self, node: Node) -> dict:
        if node is None:
            return {}

        if node.type == "jsx_self_closing_element":
            return {
                "tag":      self._get_jsx_tag(node),
                "props":    self._get_jsx_props(node),
                "children": [],
                "self_closing": True,
            }

        if node.type == "jsx_element":
            opening = None
            children = []
            for child in node.children:
                if child.type == "jsx_opening_element":
                    opening = child
                elif child.type in ("jsx_element", "jsx_self_closing_element"):
                    children.append(self._parse_jsx_element(child))
                elif child.type in ("jsx_expression", "jsx_expression_container"):
                    # Skip JSX comments: {/* ... */}
                    raw = child.text.decode()
                    inner = raw[1:-1].strip() if raw.startswith("{") and raw.endswith("}") else raw
                    if inner.startswith("/*") and "*/" in inner:
                        continue
                    # Check for conditional rendering (JSX inside expression)
                    cond = self._parse_conditional_expr(child)
                    if cond:
                        children.append(cond)
                    else:
                        # Check for list rendering: {items.map(item => <JSX/>)}
                        lst = self._parse_map_expression(child)
                        if lst:
                            children.append(lst)
                        else:
                            children.append({
                                "tag": "__expr__",
                                "text": inner,
                            })
                elif child.type == "jsx_text":
                    text = _normalize_jsx_text(child.text.decode())
                    if text is not None:
                        children.append({"tag": "__text__", "text": text})
                elif child.type == "ERROR":
                    text = child.text.decode().strip()
                    if text:
                        children.append({"tag": "__text__", "text": text})

            tag = self._get_jsx_tag(opening) if opening else ""
            if tag == "":
                return {
                    "tag":      "__fragment__",
                    "props":    {},
                    "children": children,
                    "self_closing": False,
                }
            return {
                "tag":      tag,
                "props":    self._get_jsx_props(opening) if opening else {},
                "children": children,
                "self_closing": False,
            }
        return {}

    def _get_jsx_tag(self, node: Node) -> str:
        if not node:
            return ""
        for child in node.children:
            if child.type in ("identifier", "jsx_namespace_name",
                              "member_expression"):
                return child.text.decode()
        return ""

    def _get_jsx_props(self, node: Node) -> dict:
        props = {}
        if not node:
            return props

        for child in node.children:
            if child.type == "jsx_attribute":
                name  = ""
                value = None

                for part in child.children:
                    if part.type == "property_identifier":
                        name = part.text.decode()
                    elif part.type == "string":
                        value = part.text.decode().strip("'\"")
                    elif part.type in ("jsx_expression", "jsx_expression_container"):
                        value = self._unwrap_jsx_expr(part)

                if name:
                    props[name] = value
        return props

    def _unwrap_jsx_expr(self, node: Node):
        """
        Unwrap {value} from a jsx_expression_container.
        Returns dict for style={{}}, string for static values,
        or dict with __template__ / __ref__ / __expr__ for dynamic expressions.
        """
        for child in node.children:
            if child.type == "object":
                return self._parse_style_object(child)
            elif child.type in ("string", "number", "true", "false"):
                return child.text.decode().strip("'\"")
            elif child.type == "template_string":
                return {"__template__": child.text.decode()}
            elif child.type == "identifier":
                return {"__ref__": child.text.decode()}
            elif child.type == "arrow_function":
                return {"__fn__": child.text.decode()}
        # Fallthrough — complex expression, mark so builder can transpile
        raw = node.text.decode().strip()
        if raw.startswith('{') and raw.endswith('}'):
            raw = raw[1:-1].strip()
        return {"__expr__": raw}

    def _parse_style_object(self, node: Node) -> dict:
        """Parse {{ backgroundColor: '#fff', padding: 16 }}
        Returns dict where expression values are wrapped in {"__expr__": "..."}
        so the IR builder can transpile them to C++."""
        style = {}
        for child in node.children:
            if child.type == "pair":
                key = val = ""
                val_is_expr = False
                for part in child.children:
                    if part.type == "property_identifier":
                        if not key:
                            key = part.text.decode().strip("'\"")
                    elif part.type == "string":
                        if not key:
                            key = part.text.decode().strip("'\"")
                        else:
                            val = part.text.decode().strip("'\"")
                    elif part.type == "number":
                        val = part.text.decode()
                    elif part.type == "unary_expression":
                        # "-1" → signed numeric literal (static value), not a
                        # dynamic expression. Any non-numeric operand (e.g.
                        # -flag) falls through to the expression branch.
                        sign = ""
                        num = ""
                        for sub in part.children:
                            if sub.type in ("-", "+"):
                                sign = sub.text.decode()
                            elif sub.type == "number":
                                num = sub.text.decode()
                        if num:
                            val = sign + num
                        else:
                            val = part.text.decode()
                            val_is_expr = True
                    elif part.type == "template_string":
                        val = part.text.decode()
                        val_is_expr = True
                    elif part.type == "identifier":
                        val = part.text.decode()
                        val_is_expr = True
                    elif part.is_named:
                        # Any other named node — treat as expression
                        val = part.text.decode()
                        val_is_expr = True
                if key and val:
                    css_key = self._camel_to_kebab(key)
                    if val_is_expr:
                        style[css_key] = {"__expr__": val}
                    else:
                        style[css_key] = val
        return style

    def _camel_to_kebab(self, s: str) -> str:
        import re
        return re.sub(r"(?<!^)(?=[A-Z])", "-", s).lower()

    def _extract_function_declarations(self, root: Node, components: list[dict]) -> list[dict]:
        """Extract top-level function declarations that are NOT components
        and NOT inside a component body (those are handled per-component)."""
        # Collect all tree-sitter node ids that are inside a component body
        comp_body_ids = set()
        for comp in components:
            inner = comp.get("inner_functions", [])
            for fn in inner:
                comp_body_ids.add(fn["id"])

        comp_names = {c["name"] for c in components}
        funcs = []
        for node in self._find_all(root, "function_declaration"):
            if id(node) in comp_body_ids:
                continue  # handled inside its component
            name = ""
            for child in node.children:
                if child.type == "identifier":
                    name = child.text.decode()
                    break
            if name and name not in comp_names:
                funcs.append({"name": name, "source": node.text.decode()})
        return funcs

    def _extract_inner_functions(self, block_node: Node) -> list[dict]:
        """Extract function declarations inside a component body.

        Supports both:
          function foo() { ... }
          const foo = () => { ... }  / const foo = function() { ... }
        """
        funcs = []
        for child in block_node.children:
            if child.type == "function_declaration":
                name = ""
                for c in child.children:
                    if c.type == "identifier":
                        name = c.text.decode()
                        break
                if name:
                    funcs.append({
                        "id": id(child),
                        "name": name,
                        "source": child.text.decode(),
                    })
            elif child.type == "lexical_declaration":
                for decl in child.children:
                    if decl.type == "variable_declarator":
                        var_name = ""
                        has_fn = False
                        for c in decl.children:
                            if c.type == "identifier":
                                var_name = c.text.decode()
                            elif c.type in ("arrow_function", "function_expression"):
                                has_fn = True
                        if var_name and has_fn:
                            funcs.append({
                                "id": id(decl),
                                "name": var_name,
                                "source": child.text.decode(),
                            })
        return funcs

    def _extract_component_consts(self, block_node: Node) -> list[dict]:
        """Extract simple component-level const declarations.

        Handles `const showQrImage = qrPath !== "" && qrFailed === 0`
        (non-morphState, non-arrow-function values). These are emitted as
        file-scope lambdas so they stay reactive when state changes.
        """
        consts = []
        for child in block_node.children:
            if child.type != "lexical_declaration":
                continue
            for decl in child.children:
                if decl.type != "variable_declarator":
                    continue
                var_name = ""
                value_node = None
                is_state = False
                for c in decl.children:
                    if c.type == "identifier":
                        var_name = c.text.decode()
                    elif c.type in ("arrow_function", "function_expression"):
                        var_name = ""  # handled by inner_functions
                    elif c.type == "array_pattern":
                        var_name = ""  # morphState destructuring — handled separately
                        is_state = True
                    elif c.type == "call_expression":
                        fn_parts, _ = self._parse_call_expr(c)
                        if fn_parts == ["morphState"]:
                            is_state = True
                            var_name = ""
                        elif value_node is None:
                            value_node = c
                    elif c.type not in ("=", "const", "let", "var"):
                        if value_node is None:
                            value_node = c
                if var_name and value_node is not None and not is_state:
                    consts.append({
                        "name": var_name,
                        "rhs": value_node.text.decode(),
                    })
        return consts

    # ── Generic helpers ───────────────────────────────────────

    def _find_all(self, node: Node, node_type: str) -> list[Node]:
        results = []
        if node.type == node_type:
            results.append(node)
        for child in node.children:
            results.extend(self._find_all(child, node_type))
        return results


def _normalize_jsx_text(raw: str) -> str | None:
    """Normalize JSX text whitespace the way browsers/React do.

    Rules (per the JSX spec):
      - Whitespace at the beginning/end of each line is removed.
      - Whitespace-only multi-line text (newlines between elements) collapses
        to a single space — the layout engine renders it as a gap between
        inline elements and drops it between blocks.
      - Newlines between text segments collapse to a single space.
      - Single-line text is kept verbatim (its spaces are intentional):
        'a, ' before an element stays 'a, ', and a lone space between
        elements ('a <b/> c') is preserved.
    Returns None when the text is whitespace-only across lines (dropped).
    """
    if "\n" not in raw:
        return raw

    lines = [ln.strip() for ln in raw.split("\n")]
    first = last = -1
    for i, ln in enumerate(lines):
        if ln:
            if first < 0:
                first = i
            last = i
    if first < 0:
        # Whitespace-only across lines (e.g. newline between elements):
        # collapse to a single space — the layout engine turns it into a gap
        # between inline elements (browser-like) and drops it between blocks.
        return " "

    parts = []
    for i in range(first, last + 1):
        if lines[i]:
            parts.append(lines[i])
    return " ".join(parts)
