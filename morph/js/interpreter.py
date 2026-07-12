from morph.js.walker import ASTVisitor


class JSInterpreter(ASTVisitor):
    """Walks a JS AST and extracts Morph intents (create, mount, animate)."""

    def __init__(self):
        self.intents: list[dict] = []
        self.imports: dict[str, str] = {}  # local name → package

    def interpret(self, ast: dict) -> list[dict]:
        self.visit(ast)
        return self.intents

    def visit_ImportDeclaration(self, node: dict):
        pkg = node["source"]["value"]
        for spec in node.get("specifiers", []):
            local = spec["local"]["name"]
            self.imports[local] = pkg

    def visit_NewExpression(self, node: dict):
        print(node)
        # TODO: extract component instantiation
        pass

    def visit_CallExpression(self, node: dict):
        # TODO: extract .mount(), .on(), .call() etc.
        pass
