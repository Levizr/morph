from __future__ import annotations
from tree_sitter import Node


class JSXWalker:
    """
    Walks a tree-sitter AST from a .mx file.
    Extracts: imports, components, JSX trees, props, events.
    """

    def walk(self, root: Node) -> dict:
        return {
            "imports":    self._extract_imports(root),
            "components": self._extract_components(root),
        }

    # ── Imports ───────────────────────────────────────────────

    def _extract_imports(self, root: Node) -> list[dict]:
        imports = []
        for node in self._find_all(root, "import_declaration"):
            source = self._get_import_source(node)
            if not source:
                continue

            if source.startswith("http://") or source.startswith("https://"):
                imports.append({"type": "css_url",   "url":  source})
            elif source.endswith(".css"):
                imports.append({"type": "css_local", "path": source})
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

    # ── Components ────────────────────────────────────────────

    def _extract_components(self, root: Node) -> list[dict]:
        components = []

        # function App() { return (...) }
        for node in self._find_all(root, "function_declaration"):
            comp = self._parse_function_component(node)
            if comp:
                components.append(comp)

        # export default function App() { ... }
        for node in self._find_all(root, "export_statement"):
            for fn in self._find_all(node, "function_declaration"):
                comp = self._parse_function_component(fn)
                if comp:
                    comp["exported"] = True
                    components.append(comp)

        return components

    def _parse_function_component(self, node: Node) -> dict | None:
        name = ""
        params = []
        jsx_root = None

        for child in node.children:
            if child.type == "identifier":
                name = child.text.decode()
            elif child.type == "formal_parameters":
                params = self._extract_params(child)
            elif child.type == "statement_block":
                jsx_root = self._find_jsx_in_block(child)

        if not name or jsx_root is None:
            return None

        return {
            "name":     name,
            "params":   params,
            "jsx":      self._parse_jsx_element(jsx_root),
            "exported": False,
        }

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
                                  "parenthesized_expression"):
                    if child.type == "parenthesized_expression":
                        for sub in child.children:
                            if sub.type in ("jsx_element",
                                            "jsx_self_closing_element"):
                                return sub
                    return child
        return None

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
                elif child.type == "jsx_expression_container":
                    children.append({
                        "tag": "__expr__",
                        "text": child.text.decode(),
                    })
                elif child.type == "jsx_text":
                    text = child.text.decode().strip()
                    if text:
                        children.append({"tag": "__text__", "text": text})

            return {
                "tag":      self._get_jsx_tag(opening) if opening else "",
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
                    elif part.type == "jsx_expression_container":
                        value = self._unwrap_jsx_expr(part)

                if name:
                    props[name] = value

        return props

    def _unwrap_jsx_expr(self, node: Node):
        """
        Unwrap {value} from a jsx_expression_container.
        Returns dict for style={{}}, string for others.
        """
        for child in node.children:
            if child.type == "object":
                return self._parse_style_object(child)
            elif child.type in ("string", "number", "true", "false"):
                return child.text.decode().strip("'\"")
            elif child.type == "template_string":
                return child.text.decode()
            elif child.type == "identifier":
                return {"__ref__": child.text.decode()}
            elif child.type == "arrow_function":
                return {"__fn__": child.text.decode()}
        return node.text.decode()

    def _parse_style_object(self, node: Node) -> dict:
        """Parse {{ backgroundColor: '#fff', padding: 16 }}"""
        style = {}
        for child in node.children:
            if child.type == "pair":
                key = val = ""
                for part in child.children:
                    if part.type in ("property_identifier", "string"):
                        if not key:
                            key = part.text.decode().strip("'\"")
                    elif part.type in ("string", "number",
                                       "template_string", "identifier"):
                        val = part.text.decode().strip("'\"")
                if key and val:
                    # camelCase → kebab-case
                    style[self._camel_to_kebab(key)] = val
        return style

    def _camel_to_kebab(self, s: str) -> str:
        import re
        return re.sub(r"(?<!^)(?=[A-Z])", "-", s).lower()

    # ── Generic helpers ───────────────────────────────────────

    def _find_all(self, node: Node, node_type: str) -> list[Node]:
        results = []
        if node.type == node_type:
            results.append(node)
        for child in node.children:
            results.extend(self._find_all(child, node_type))
        return results
