class ASTVisitor:
    """Base visitor for walking a JS AST (esprima-style dicts)."""

    def visit(self, node: dict):
        kind = node.get("type", "")
        method = getattr(self, f"visit_{kind}", self.generic_visit)
        return method(node)

    def generic_visit(self, node: dict):
        for val in node.values():
            if isinstance(val, dict) and "type" in val:
                self.visit(val)
            elif isinstance(val, list):
                for item in val:
                    if isinstance(item, dict) and "type" in item:
                        self.visit(item)
