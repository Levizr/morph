from __future__ import annotations

from tree_sitter import Node

from morph.js.ast import (
    TSAwaitExpression,
    TSArrayLiteral,
    TSArrayType,
    TSArrowFunction,
    TSAssignmentExpression,
    TSBinaryExpression,
    TSBlockStatement,
    TSBreakStatement,
    TSCallExpression,
    TSCaseClause,
    TSCatchClause,
    TSClassDeclaration,
    TSConstructor,
    TSContinueStatement,
    TSDefaultClause,
    TSDoWhileStatement,
    TSExpressionStatement,
    TSFunctionExpression,
    TSObjectLiteral,
    TSObjectProperty,
    TSSpreadElement,
    TSForStatement,
    TSFunctionDeclaration,
    TSGenericType,
    TSIdentifier,
    TSIfStatement,
    TSInterfaceDeclaration,
    TSLiteral,
    TSMemberExpression,
    TSMethodDefinition,
    TSNewExpression,
    TSNode,
    TSParenthesizedExpression,
    TSProgram,
    TSPropertyDefinition,
    TSReturnStatement,
    TSSequenceExpression,
    TSSuperExpression,
    TSSwitchStatement,
    TSTernaryExpression,
    TSThisExpression,
    TSTemplateLiteral,
    TSThrowStatement,
    TSTryStatement,
    TSType,
    TSTypeParameter,
    TSUnaryExpression,
    TSUnionType,
    TSUpdateExpression,
    TSVariableDeclaration,
    TSWhileStatement,
    TSPredefinedType,
)

_NODE_TYPES = frozenset([
    "program",
    "function_declaration",
    "arrow_function",
    "lexical_declaration",
    "variable_declaration",
    "variable_declarator",
    "identifier",
    "number",
    "string",
    "template_string",
    "template_substitution",
    "call_expression",
    "await_expression",
    "member_expression",
    "subscript_expression",
    "binary_expression",
    "unary_expression",
    "update_expression",
    "assignment_expression",
    "parenthesized_expression",
    "statement_block",
    "expression_statement",
    "return_statement",
    "if_statement",
    "while_statement",
    "for_statement",
    "type_annotation",
    "predefined_type",
    "array_type",
    "generic_type",
    "union_type",
    "formal_parameters",
    "required_parameter",
    "optional_parameter",
    "true",
    "false",
    "null",
    "comment",
    "export_statement",
    # OOP
    "class_declaration",
    "interface_declaration",
    "method_definition",
    "public_field_definition",
    "constructor",
    "this",
    "super",
    "new_expression",
    "type_parameter",
    "type_parameters",
    "property_identifier",
    "property_signature",
    "method_signature",
    "accessibility_modifier",
    "extends_clause",
    "implements_clause",
    "heritage_clause",
    "abstract",
    "static",
    "readonly",
    "array",
    "undefined",
    "ternary_expression",
    "break_statement",
    "continue_statement",
    "do_statement",
    "switch_statement",
    "switch_case",
    "switch_default",
    "sequence_expression",
    "empty_statement",
    "try_statement",
    "catch_clause",
    "throw_statement",
    "abstract_class_declaration",
    "abstract_method_signature",
])


class TSAstBuilder:
    """Builds a TSAST from a tree-sitter TSX AST subtree."""

    def build(self, node: Node) -> TSProgram:
        assert node.type == "program", f"expected program node, got {node.type}"
        stmts = []
        for child in node.children:
            stmt = self._build_node(child)
            if stmt is not None:
                stmts.append(stmt)
        return TSProgram(statements=stmts)

    def build_expression(self, node: Node) -> TSNode | None:
        return self._build_node(node)

    def build_statement(self, node: Node) -> TSNode | None:
        return self._build_node(node)

    def _build_node(self, node: Node) -> TSNode | None:
        if node.type == "comment":
            return None

        method = getattr(self, f"_build_{node.type.replace('-', '_')}", None)
        if method is None:
            text = node.text.decode()
            if text.strip():
                import warnings
                warnings.warn(f"unhandled TS node type: {node.type} = {text!r}",
                              stacklevel=2)
            return None
        return method(node)

    def _build_empty_statement(self, node: Node) -> None:
        return None

    # ── Program ────────────────────────────────────────────────

    def _build_program(self, node: Node) -> TSProgram:
        stmts = []
        for child in node.children:
            stmt = self._build_node(child)
            if stmt is not None:
                stmts.append(stmt)
        return TSProgram(statements=stmts)

    # ── Declarations ───────────────────────────────────────────

    def _build_function_declaration(self, node: Node) -> TSFunctionDeclaration | None:
        name = ""
        params = []
        return_type = None
        body = None
        type_params = None
        is_async = False

        for child in node.children:
            if child.type == "async":
                is_async = True
            elif child.type == "identifier":
                name = child.text.decode()
            elif child.type == "formal_parameters":
                params = self._build_formal_parameters(child)
            elif child.type == "type_annotation":
                return_type = self._build_type_annotation(child)
            elif child.type == "statement_block":
                body = self._build_statement_block(child)
            elif child.type == "type_parameters":
                type_params = self._build_type_parameters(child)

        if not name or body is None:
            return None
        return TSFunctionDeclaration(
            name=name,
            params=params,
            return_type=return_type,
            body=body,
            type_parameters=type_params,
            is_async=is_async,
        )

    def _build_lexical_declaration(self, node: Node) -> TSNode | None:
        kind = ""
        declarators = []

        for child in node.children:
            if child.type in ("const", "let", "var"):
                kind = child.text.decode()
            elif child.type == "variable_declarator":
                d = self._build_variable_declarator(child)
                if d is not None:
                    declarators.append(d)

        if len(declarators) == 1:
            declarators[0].kind = kind
            return declarators[0]

        if declarators:
            from morph.js.ast import TSBlockStatement
            stmts = []
            for d in declarators:
                d.kind = kind
                stmts.append(
                    TSExpressionStatement(
                        expression=TSBinaryExpression(
                            operator="=",
                            left=TSIdentifier(name=d.name.name,
                                              type_annotation=d.type_annotation),
                            right=d.initializer,
                        )
                    ) if d.initializer else TSExpressionStatement(
                        expression=TSIdentifier(name=d.name.name,
                                                type_annotation=d.type_annotation)
                    )
                )
            return declarators[0]

        return None

    def _build_variable_declarator(self, node: Node) -> TSVariableDeclaration | None:
        name_node = None
        initializer = None
        type_ann = None
        found_equals = False

        for child in node.children:
            if child.type == "identifier":
                name_node = child
            elif child.type == "type_annotation":
                type_ann = self._build_type_annotation(child)
            elif child.type == "=":
                found_equals = True
            elif child.type == ":":
                continue
            elif child.type in ("required_parameter", "optional_parameter"):
                continue
            else:
                candidate = self._build_node(child)
                if candidate is not None:
                    if found_equals:
                        initializer = candidate
                    else:
                        initializer = candidate

        if name_node is None:
            return None

        raw_name = name_node.text.decode()
        return TSVariableDeclaration(
            kind="let",
            name=TSIdentifier(name=raw_name, type_annotation=type_ann),
            initializer=initializer,
            type_annotation=type_ann,
        )

    def _build_formal_parameters(self, node: Node) -> list[TSVariableDeclaration]:
        params = []
        for child in node.children:
            if child.type in ("required_parameter", "optional_parameter"):
                p = self._build_parameter(child)
                if p is not None:
                    params.append(p)
        return params

    def _build_parameter(self, node: Node) -> TSVariableDeclaration | None:
        name = ""
        type_ann = None
        default = None
        seen_equals = False

        for child in node.children:
            if child.type == "identifier":
                name = child.text.decode()
            elif child.type == "type_annotation":
                type_ann = self._build_type_annotation(child)
            elif child.type == "=":
                seen_equals = True
            elif child.type == "rest_pattern":
                for sub in child.children:
                    if sub.type == "identifier":
                        name = sub.text.decode()
            elif child.type != "?":
                if seen_equals:
                    candidate = self._build_node(child)
                    if candidate is not None:
                        default = candidate
                # else: skip — might be part of rest_pattern etc.

        if not name:
            return None

        return TSVariableDeclaration(
            kind="param",
            name=TSIdentifier(name=name, type_annotation=type_ann),
            initializer=default,
            type_annotation=type_ann,
        )

    # ── Arrow Functions ────────────────────────────────────────

    def _build_arrow_function(self, node: Node) -> TSArrowFunction:
        params = []
        return_type = None
        body = None
        is_async = False

        for child in node.children:
            if child.type == "async":
                is_async = True
            elif child.type == "formal_parameters":
                params = self._build_formal_parameters(child)
            elif child.type == "type_annotation":
                return_type = self._build_type_annotation(child)
            elif child.type == "statement_block":
                body = self._build_statement_block(child)
            elif child.type == "=>":
                pass
            else:
                expr = self._build_node(child)
                if expr is not None:
                    body = expr

        if body is None:
            body = TSBlockStatement(statements=[])

        return TSArrowFunction(
            params=params,
            return_type=return_type,
            body=body,
            is_async=is_async,
        )

    def _build_function_expression(self, node: Node) -> TSFunctionExpression:
        params = []
        return_type = None
        body = None
        is_async = False

        for child in node.children:
            if child.type == "async":
                is_async = True
            elif child.type == "formal_parameters":
                params = self._build_formal_parameters(child)
            elif child.type == "type_annotation":
                return_type = self._build_type_annotation(child)
            elif child.type == "statement_block":
                body = self._build_statement_block(child)
            elif child.type == "function":
                pass

        if body is None:
            body = TSBlockStatement(statements=[])

        return TSFunctionExpression(
            params=params,
            return_type=return_type,
            body=body,
            is_async=is_async,
        )

    # ── Statements ─────────────────────────────────────────────

    def _build_statement_block(self, node: Node) -> TSBlockStatement:
        stmts = []
        for child in node.children:
            if child.type in ("{", "}"):
                continue
            stmt = self._build_node(child)
            if stmt is not None:
                stmts.append(stmt)
        return TSBlockStatement(statements=stmts)

    def _build_expression_statement(self, node: Node) -> TSNode | None:
        for child in node.children:
            if child.type != ";":
                expr = self._build_node(child)
                if expr is not None:
                    return TSExpressionStatement(expression=expr)
        return None

    def _build_return_statement(self, node: Node) -> TSReturnStatement:
        arg = None
        for child in node.children:
            if child.type == "return":
                continue
            if child.type == ";":
                continue
            candidate = self._build_node(child)
            if candidate is not None:
                arg = candidate
        return TSReturnStatement(argument=arg)

    def _build_if_statement(self, node: Node) -> TSIfStatement:
        condition = None
        consequence = None
        alternate = None

        for child in node.children:
            if child.type == "parenthesized_expression":
                for sub in child.children:
                    if sub.type not in ("(", ")"):
                        cond = self._build_node(sub)
                        if cond is not None:
                            condition = cond
            elif child.type == "statement_block":
                if consequence is None:
                    consequence = self._build_statement_block(child)
                else:
                    alternate = self._build_statement_block(child)
            elif child.type == "else_clause":
                for sub in child.children:
                    if sub.type == "statement_block":
                        alternate = self._build_statement_block(sub)
                    elif sub.type == "if_statement":
                        alternate = self._build_if_statement(sub)
            elif child.type not in ("else", "if"):
                # Single-statement consequence (return, expression, etc.)
                stmt = self._build_node(child)
                if stmt is not None and consequence is None:
                    consequence = TSBlockStatement(statements=[stmt])

        if condition is None:
            condition = TSLiteral(value=True, raw="true")

        return TSIfStatement(
            condition=condition,
            consequence=consequence or TSBlockStatement(statements=[]),
            alternate=alternate,
        )

    def _build_augmented_assignment_expression(self, node: Node) -> TSAssignmentExpression:
        left = None
        operator = ""
        right = None

        for child in node.children:
            if child.type in ("+=", "-=", "*=", "/=", "%=",
                              "<<=", ">>=", ">>>=", "&=", "^=", "|=",
                              "**=", "&&=", "||=", "??="):
                operator = child.text.decode()
            elif left is None:
                left = self._build_node(child)
            else:
                right = self._build_node(child)

        return TSAssignmentExpression(
            operator=operator,
            left=left or TSIdentifier(name=""),
            right=right or TSLiteral(value=0, raw="0"),
        )

    def _build_while_statement(self, node: Node) -> TSWhileStatement:
        condition = None
        body = None
        for child in node.children:
            if child.type in ("while",):
                continue
            if child.type == "(":
                continue
            if child.type == ")":
                continue
            if condition is None:
                condition = self._build_node(child)
            else:
                body = self._build_node(child)
        return TSWhileStatement(
            condition=condition or TSLiteral(value=True, raw="true"),
            body=body or TSBlockStatement(statements=[]),
        )

    def _build_for_statement(self, node: Node) -> TSForStatement:
        init = None
        condition = None
        update = None
        body = None
        part = 0

        for child in node.children:
            if child.type in ("for", "(", ")", ";"):
                continue
            if part == 0:
                init = self._build_node(child)
                part = 1
            elif part == 1:
                condition = self._build_node(child)
                part = 2
            elif part == 2:
                if child.type == "statement_block":
                    body = self._build_statement_block(child)
                else:
                    update = self._build_node(child)
                    part = 3
            elif part == 3:
                if child.type == "statement_block":
                    body = self._build_statement_block(child)
                else:
                    body = self._build_node(child)

        return TSForStatement(
            init=init,
            condition=condition,
            update=update,
            body=body or TSBlockStatement(statements=[]),
        )

    def _build_break_statement(self, node: Node) -> TSBreakStatement:
        return TSBreakStatement()

    def _build_continue_statement(self, node: Node) -> TSContinueStatement:
        return TSContinueStatement()

    def _build_do_statement(self, node: Node) -> TSDoWhileStatement:
        body = None
        condition = None
        for child in node.children:
            if child.type in ("do", "while", ";", "(", ")"):
                continue
            if body is None:
                body = self._build_node(child)
            else:
                condition = self._build_node(child)
        return TSDoWhileStatement(
            body=body or TSBlockStatement(statements=[]),
            condition=condition or TSLiteral(value=True, raw="true"),
        )

    def _build_switch_statement(self, node: Node) -> TSSwitchStatement:
        discriminant = None
        cases = []
        for child in node.children:
            if child.type in ("switch", "(", ")"):
                continue
            if discriminant is None:
                discriminant = self._build_node(child)
            elif child.type == "switch_body":
                for sub in child.children:
                    if sub.type in ("{", "}"):
                        continue
                    c = self._build_node(sub)
                    if c is not None:
                        cases.append(c)
        return TSSwitchStatement(
            discriminant=discriminant or TSLiteral(value=0, raw="0"),
            cases=cases,
        )

    def _build_switch_case(self, node: Node) -> TSCaseClause:
        test = None
        consequence = []
        for child in node.children:
            if child.type in ("case", ":"):
                continue
            if test is None:
                test = self._build_node(child)
            else:
                stmt = self._build_node(child)
                if stmt is not None:
                    consequence.append(stmt)
        return TSCaseClause(
            test=test or TSLiteral(value=0, raw="0"),
            consequence=consequence,
        )

    def _build_switch_default(self, node: Node) -> TSDefaultClause:
        consequence = []
        for child in node.children:
            if child.type in ("default", ":"):
                continue
            stmt = self._build_node(child)
            if stmt is not None:
                consequence.append(stmt)
        return TSDefaultClause(consequence=consequence)

    def _build_abstract(self, node: Node) -> None:
        return None

    def _build_abstract_class_declaration(self, node: Node) -> TSClassDeclaration | None:
        return self._build_class_declaration(node)

    def _build_variable_declaration(self, node: Node) -> TSNode | None:
        # tree-sitter uses "variable_declaration" for var statements
        # Delegate to lexical_declaration handler which already handles "var"
        return self._build_lexical_declaration(node)

    def _build_sequence_expression(self, node: Node) -> TSSequenceExpression:
        exprs = []
        for child in node.children:
            if child.type == ",":
                continue
            e = self._build_node(child)
            if e is not None:
                exprs.append(e)
        return TSSequenceExpression(expressions=exprs)

    # ── Try / Catch / Throw ─────────────────────────────────────

    def _build_try_statement(self, node: Node) -> TSTryStatement:
        body = None
        handler = None
        finalizer = None
        for child in node.children:
            if child.type in ("try",):
                continue
            if child.type == "statement_block":
                if body is None:
                    body = self._build_statement_block(child)
            elif child.type == "catch_clause":
                handler = self._build_catch_clause(child)
            elif child.type == "finally_clause":
                for sub in child.children:
                    if sub.type == "statement_block":
                        finalizer = self._build_statement_block(sub)
        return TSTryStatement(
            body=body or TSBlockStatement(statements=[]),
            handler=handler,
            finalizer=finalizer,
        )

    def _build_catch_clause(self, node: Node) -> TSCatchClause:
        param = TSIdentifier(name="_e")
        body = TSBlockStatement(statements=[])
        for child in node.children:
            if child.type in ("catch", "(", ")"):
                continue
            if child.type == "identifier":
                param = TSIdentifier(name=child.text.decode())
            elif child.type == "statement_block":
                body = self._build_statement_block(child)
        return TSCatchClause(param=param, body=body)

    def _build_throw_statement(self, node: Node) -> TSThrowStatement:
        arg = None
        for child in node.children:
            if child.type == "throw":
                continue
            if child.type == ";":
                continue
            candidate = self._build_node(child)
            if candidate is not None:
                arg = candidate
        return TSThrowStatement(
            argument=arg or TSLiteral(value=None, raw="undefined"),
        )

    # ── Expressions ────────────────────────────────────────────

    def _build_identifier(self, node: Node) -> TSIdentifier:
        return TSIdentifier(name=node.text.decode())

    def _build_property_identifier(self, node: Node) -> TSIdentifier:
        return TSIdentifier(name=node.text.decode())

    def _build_number(self, node: Node) -> TSLiteral:
        text = node.text.decode()
        val: int | float
        if text.startswith("0x") or text.startswith("0X"):
            val = int(text, 16)
            return TSLiteral(value=val, raw=text)
        if text.startswith("0o") or text.startswith("0O"):
            val = int(text, 8)
            return TSLiteral(value=val, raw=text)
        if text.startswith("0b") or text.startswith("0B"):
            val = int(text, 2)
            return TSLiteral(value=val, raw=text)
        try:
            if "." in text or "e" in text or "E" in text:
                val = float(text)
            else:
                val = int(text)
        except (ValueError, OverflowError):
            val = float(text)
        return TSLiteral(value=val, raw=text)

    def _build_string(self, node: Node) -> TSLiteral:
        text = node.text.decode()
        val = text.strip("'\"")
        return TSLiteral(value=val, raw=text)

    def _build_true(self, node: Node) -> TSLiteral:
        return TSLiteral(value=True, raw="true")

    def _build_false(self, node: Node) -> TSLiteral:
        return TSLiteral(value=False, raw="false")

    def _build_null(self, node: Node) -> TSLiteral:
        return TSLiteral(value=None, raw="null")

    def _build_undefined(self, node: Node) -> TSLiteral:
        return TSLiteral(value=None, raw="undefined")

    def _build_template_string(self, node: Node) -> TSTemplateLiteral:
        parts: list[str | TSNode] = []
        for child in node.children:
            if child.type == "template_substitution":
                inner = None
                for sub in child.children:
                    if sub.type not in ("${", "}"):
                        inner = self._build_node(sub)
                parts.append(inner or TSIdentifier(name=""))
            elif child.type == "string_fragment":
                text = child.text.decode()
                parts.append(text)
        return TSTemplateLiteral(parts=parts)

    def _build_call_expression(self, node: Node) -> TSCallExpression:
        callee = None
        args = []
        type_args = None

        for child in node.children:
            if child.type == "arguments":
                for arg in child.children:
                    if arg.type in (",", "(", ")"):
                        continue
                    a = self._build_node(arg)
                    if a is not None:
                        args.append(a)
            elif child.type == "type_arguments":
                type_args = self._build_type_arguments(child)
            else:
                c = self._build_node(child)
                if c is not None:
                    callee = c

        return TSCallExpression(
            callee=callee or TSIdentifier(name=""),
            arguments=args,
            type_arguments=type_args,
        )

    def _build_await_expression(self, node: Node) -> TSAwaitExpression:
        argument = None
        for child in node.children:
            if child.type == "await":
                continue
            candidate = self._build_node(child)
            if candidate is not None:
                argument = candidate
        return TSAwaitExpression(
            argument=argument or TSIdentifier(name="undefined"),
        )

    def _build_member_expression(self, node: Node) -> TSMemberExpression:
        obj = None
        prop = None
        computed = False

        for child in node.children:
            if child.type in (".", "optional_chain"):
                continue
            if child.type == "[":
                computed = True
                continue
            if child.type == "]":
                continue
            if child.type == "property_identifier":
                prop = TSIdentifier(name=child.text.decode())
            else:
                if obj is None:
                    obj = self._build_node(child)
                else:
                    prop = self._build_node(child)

        return TSMemberExpression(
            object=obj or TSIdentifier(name=""),
            property=prop or TSIdentifier(name=""),
            computed=computed,
        )

    def _build_subscript_expression(self, node: Node) -> TSMemberExpression:
        obj = None
        prop = None
        for child in node.children:
            if child.type in ("[", "]"):
                continue
            if obj is None:
                obj = self._build_node(child)
            else:
                prop = self._build_node(child)
        return TSMemberExpression(
            object=obj or TSIdentifier(name=""),
            property=prop or TSIdentifier(name=""),
            computed=True,
        )

    def _build_binary_expression(self, node: Node) -> TSBinaryExpression:
        left = None
        operator = ""
        right = None

        for child in node.children:
            if child.type in ("+", "-", "*", "/", "%", "**",
                              "==", "!=", "===", "!==",
                              "<", ">", "<=", ">=",
                              "&&", "||", "??",
                              "&", "|", "^", "<<", ">>", ">>>",
                              "in", "instanceof"):
                operator = child.text.decode()
            elif left is None:
                left = self._build_node(child)
            else:
                right = self._build_node(child)

        return TSBinaryExpression(
            operator=operator,
            left=left or TSLiteral(value=0, raw="0"),
            right=right or TSLiteral(value=0, raw="0"),
        )

    def _build_unary_expression(self, node: Node) -> TSUnaryExpression:
        operator = ""
        argument = None
        prefix = True

        for child in node.children:
            if child.type in ("!", "~", "-", "+", "typeof", "void", "delete"):
                operator = child.text.decode()
            else:
                argument = self._build_node(child)

        return TSUnaryExpression(
            operator=operator,
            argument=argument or TSLiteral(value=0, raw="0"),
            prefix=prefix,
        )

    def _build_ternary_expression(self, node: Node) -> TSTernaryExpression:
        condition = None
        consequent = None
        alternate = None
        for child in node.children:
            if child.type == "?":
                continue
            if child.type == ":":
                continue
            if condition is None:
                condition = self._build_node(child)
            elif consequent is None:
                consequent = self._build_node(child)
            else:
                alternate = self._build_node(child)
        return TSTernaryExpression(
            condition=condition or TSLiteral(value=True, raw="true"),
            consequent=consequent or TSLiteral(value=0, raw="0"),
            alternate=alternate or TSLiteral(value=0, raw="0"),
        )

    def _build_update_expression(self, node: Node) -> TSUpdateExpression:
        operator = ""
        argument = None
        prefix = True

        children = list(node.children)
        if children:
            first = children[0]
            if first.type in ("++", "--"):
                operator = first.text.decode()
                prefix = True
                for child in children[1:]:
                    arg = self._build_node(child)
                    if arg is not None:
                        argument = arg
                        break
            else:
                prefix = False
                for child in children:
                    if child.type in ("++", "--"):
                        operator = child.text.decode()
                    elif argument is None:
                        argument = self._build_node(child)

        return TSUpdateExpression(
            operator=operator,
            argument=argument or TSIdentifier(name=""),
            prefix=prefix,
        )

    def _build_assignment_expression(self, node: Node) -> TSAssignmentExpression:
        left = None
        operator = ""
        right = None

        for child in node.children:
            if child.type in ("=", "+=", "-=", "*=", "/=", "%=",
                              "<<=", ">>=", ">>>=", "&=", "^=", "|=",
                              "**=", "&&=", "||=", "??="):
                operator = child.text.decode()
            elif left is None:
                left = self._build_node(child)
            else:
                right = self._build_node(child)

        return TSAssignmentExpression(
            operator=operator,
            left=left or TSIdentifier(name=""),
            right=right or TSLiteral(value=0, raw="0"),
        )

    def _build_array(self, node: Node) -> TSArrayLiteral:
        elements = []
        for child in node.children:
            if child.type in ("[", "]", ","):
                continue
            if child.type == "spread_element":
                inner = None
                for sub in child.children:
                    if sub.type in ("[", "]", ",", "..."):
                        continue
                    inner = self._build_node(sub)
                    break
                if inner is not None:
                    elements.append(TSSpreadElement(argument=inner))
                continue
            elem = self._build_node(child)
            if elem is not None:
                elements.append(elem)
        return TSArrayLiteral(elements=elements)

    def _build_object(self, node: Node) -> TSObjectLiteral:
        props = []
        for child in node.children:
            if child.type in ("{", "}", ","):
                continue
            if child.type == "shorthand_property_identifier":
                name = child.text.decode()
                props.append(TSObjectProperty(
                    key=name,
                    value=TSIdentifier(name=name),
                ))
                continue
            prop = self._build_node(child)
            if prop is not None:
                props.append(prop)
        return TSObjectLiteral(properties=props)

    def _build_pair(self, node: Node) -> TSObjectProperty | None:
        key = None
        value = None
        for child in node.children:
            if child.type == "property_identifier":
                key = child.text.decode()
            elif child.type == "string":
                text = child.text.decode().strip("\"'")
                if key is None:
                    key = text
                else:
                    val = self._build_node(child)
                    if val is not None:
                        value = val
            elif child.type == ":":
                continue
            else:
                val = self._build_node(child)
                if val is not None:
                    value = val
        if key is None:
            return None
        return TSObjectProperty(key=key, value=value or TSLiteral(value=None, raw="null"))

    def _build_parenthesized_expression(self, node: Node) -> TSParenthesizedExpression:
        expr = None
        for child in node.children:
            if child.type in ("(", ")"):
                continue
            e = self._build_node(child)
            if e is not None:
                expr = e
        return TSParenthesizedExpression(
            expression=expr or TSLiteral(value=0, raw="0"),
        )

    # ── OOP: this, super, new ────────────────────────────────

    def _build_this(self, node: Node) -> TSThisExpression:
        return TSThisExpression()

    def _build_super(self, node: Node) -> TSSuperExpression:
        return TSSuperExpression()

    def _build_new_expression(self, node: Node) -> TSNewExpression:
        callee = None
        args = []
        type_args = None

        for child in node.children:
            if child.type == "new":
                continue
            if child.type == "arguments":
                for arg in child.children:
                    if arg.type in (",", "(", ")"):
                        continue
                    a = self._build_node(arg)
                    if a is not None:
                        args.append(a)
            elif child.type == "type_arguments":
                type_args = self._build_type_arguments(child)
            else:
                c = self._build_node(child)
                if c is not None:
                    callee = c

        return TSNewExpression(
            callee=callee or TSIdentifier(name=""),
            arguments=args,
            type_arguments=type_args,
        )

    # ── OOP: Type Parameters ──────────────────────────────────

    def _build_type_parameters(self, node: Node) -> list[TSTypeParameter]:
        params = []
        for child in node.children:
            if child.type in ("<", ">", ","):
                continue
            p = self._build_node(child)
            if isinstance(p, TSTypeParameter):
                params.append(p)
        return params

    def _build_type_parameter(self, node: Node) -> TSTypeParameter:
        name = ""
        constraint = None

        for child in node.children:
            if child.type == "type_identifier":
                name = child.text.decode()
            elif child.type == "type_annotation":
                t = self._build_type_node(child)
                if t is not None:
                    constraint = t
            elif child.type == "constraint":
                for sub in child.children:
                    if sub.type != "extends":
                        t = self._build_type_node(sub)
                        if t is not None:
                            constraint = t

        return TSTypeParameter(
            name=name,
            constraint=constraint,
        )

    # ── OOP: Class Members ─────────────────────────────────────

    def _build_method_definition(self, node: Node) -> TSMethodDefinition | TSConstructor:
        name = ""
        params = []
        return_type = None
        body = None
        access = ""
        static_flag = False
        is_async = False

        for child in node.children:
            if child.type == "async":
                is_async = True
            elif child.type == "property_identifier":
                name = child.text.decode()
            elif child.type == "formal_parameters":
                params = self._build_formal_parameters(child)
            elif child.type == "type_annotation":
                return_type = self._build_type_annotation(child)
            elif child.type == "statement_block":
                body = self._build_statement_block(child)
            elif child.type == "accessibility_modifier":
                access = self._read_accessibility(child)
            elif child.type == "static":
                static_flag = True

        if name == "constructor":
            return TSConstructor(
                params=params,
                body=body or TSBlockStatement(statements=[]),
            )

        return TSMethodDefinition(
            name=name,
            params=params,
            return_type=return_type,
            body=body or TSBlockStatement(statements=[]),
            access_modifier=access,
            static=static_flag,
            is_async=is_async,
        )

    def _build_public_field_definition(self, node: Node) -> TSPropertyDefinition:
        name = ""
        type_ann = None
        initializer = None
        access = ""
        static_flag = False
        readonly_flag = False

        for child in node.children:
            if child.type == "property_identifier":
                name = child.text.decode()
            elif child.type == "type_annotation":
                type_ann = self._build_type_annotation(child)
            elif child.type == "accessibility_modifier":
                access = self._read_accessibility(child)
            elif child.type == "static":
                static_flag = True
            elif child.type == "readonly":
                readonly_flag = True
            elif child.type == "=":
                continue
            else:
                candidate = self._build_node(child)
                if candidate is not None:
                    initializer = candidate

        return TSPropertyDefinition(
            name=name,
            type_annotation=type_ann,
            initializer=initializer,
            access_modifier=access,
            static=static_flag,
            readonly=readonly_flag,
        )

    def _build_constructor(self, node: Node) -> TSConstructor:
        params = []
        body = None

        for child in node.children:
            if child.type == "formal_parameters":
                params = self._build_formal_parameters(child)
            elif child.type == "statement_block":
                body = self._build_statement_block(child)

        return TSConstructor(
            params=params,
            body=body or TSBlockStatement(statements=[]),
        )

    # ── OOP: Interface Members ─────────────────────────────────

    def _build_property_signature(self, node: Node) -> TSPropertyDefinition:
        name = ""
        type_ann = None

        for child in node.children:
            if child.type == "property_identifier":
                name = child.text.decode()
            elif child.type == "type_annotation":
                type_ann = self._build_type_annotation(child)

        return TSPropertyDefinition(
            name=name,
            type_annotation=type_ann,
            initializer=None,
            access_modifier="",
        )

    def _build_abstract_method_signature(self, node: Node) -> TSMethodDefinition:
        return self._build_method_signature(node)

    def _build_method_signature(self, node: Node) -> TSMethodDefinition:
        name = ""
        params = []
        return_type = None

        for child in node.children:
            if child.type == "property_identifier":
                name = child.text.decode()
            elif child.type == "formal_parameters":
                params = self._build_formal_parameters(child)
            elif child.type == "type_annotation":
                return_type = self._build_type_annotation(child)

        return TSMethodDefinition(
            name=name,
            params=params,
            return_type=return_type,
            body=TSBlockStatement(statements=[]),
        )

    # ── OOP: Class & Interface Declarations ──────────────────

    def _build_class_declaration(self, node: Node) -> TSClassDeclaration | None:
        name = ""
        members: list[TSPropertyDefinition | TSMethodDefinition | TSConstructor] = []
        type_params = None
        extends = None
        implements: list[str] = []

        for child in node.children:
            if child.type in ("identifier", "type_identifier"):
                name = child.text.decode()
            elif child.type == "type_parameters":
                type_params = self._build_type_parameters(child)
            elif child.type in ("class_heritage", "heritage"):
                for clause in child.children:
                    if clause.type == "extends_clause":
                        for sub in clause.children:
                            if sub.type in ("identifier", "type_identifier"):
                                extends = sub.text.decode()
                    elif clause.type == "implements_clause":
                        for sub in clause.children:
                            if sub.type in ("identifier", "type_identifier"):
                                implements.append(sub.text.decode())
                    elif clause.type == "type_identifier":
                        if extends is None:
                            extends = clause.text.decode()
                        else:
                            implements.append(clause.text.decode())
            elif child.type == "class_body":
                for sub in child.children:
                    if sub.type in ("{", "}", ";"):
                        continue
                    member = self._build_node(sub)
                    if isinstance(member, (TSPropertyDefinition, TSMethodDefinition, TSConstructor)):
                        members.append(member)

        if not name:
            return None
        return TSClassDeclaration(
            name=name,
            members=members,
            type_parameters=type_params,
            extends=extends,
            implements=implements,
        )

    def _build_interface_declaration(self, node: Node) -> TSInterfaceDeclaration | None:
        name = ""
        members: list[TSPropertyDefinition | TSMethodDefinition] = []
        type_params = None
        extends: list[str] = []

        for child in node.children:
            if child.type in ("identifier", "type_identifier"):
                name = child.text.decode()
            elif child.type == "type_parameters":
                type_params = self._build_type_parameters(child)
            elif child.type in ("class_heritage", "heritage"):
                for clause in child.children:
                    if clause.type in ("extends_clause",):
                        for sub in clause.children:
                            if sub.type in ("identifier", "type_identifier"):
                                extends.append(sub.text.decode())
                    elif clause.type == "type_identifier":
                        extends.append(clause.text.decode())
            elif child.type == "interface_body":
                for sub in child.children:
                    if sub.type in ("{", "}", ";"):
                        continue
                    member = self._build_node(sub)
                    if isinstance(member, (TSPropertyDefinition, TSMethodDefinition)):
                        members.append(member)

        if not name:
            return None
        return TSInterfaceDeclaration(
            name=name,
            members=members,
            type_parameters=type_params,
            extends=extends,
        )

    # ── OOP: Helpers ───────────────────────────────────────────

    def _read_accessibility(self, node: Node) -> str:
        for child in node.children:
            return child.text.decode()
        return ""

    # ── Types ──────────────────────────────────────────────────

    def _build_type_annotation(self, node: Node) -> TSType | None:
        for child in node.children:
            if child.type == ":":
                continue
            t = self._build_type_node(child)
            if t is not None:
                return t
        return None

    def _build_type_node(self, node: Node) -> TSType | None:
        method = getattr(self, f"_build_type_{node.type.replace('-', '_')}", None)
        if method:
            return method(node)
        return TSPredefinedType(raw=node.text.decode())

    def _build_type_predefined_type(self, node: Node) -> TSPredefinedType:
        return TSPredefinedType(raw=node.text.decode())

    def _build_type_array_type(self, node: Node) -> TSArrayType:
        element = TSPredefinedType(raw="any")
        for child in node.children:
            if child.type != "[" and child.type != "]":
                t = self._build_type_node(child)
                if t is not None:
                    element = t
        return TSArrayType(raw=f"{element.raw}[]", element_type=element)

    def _build_type_generic_type(self, node: Node) -> TSGenericType:
        name = ""
        type_args = []

        for child in node.children:
            if child.type == "type_identifier":
                name = child.text.decode()
            elif child.type == "type_arguments":
                for arg in child.children:
                    if arg.type not in ("<", ">", ","):
                        t = self._build_type_node(arg)
                        if t is not None:
                            type_args.append(t)

        return TSGenericType(
            raw=f"{name}<{', '.join(t.raw for t in type_args)}>",
            name=name,
            type_args=type_args,
        )

    def _build_type_union_type(self, node: Node) -> TSUnionType:
        types = []
        for child in node.children:
            if child.type == "|":
                continue
            t = self._build_type_node(child)
            if t is not None:
                types.append(t)
        return TSUnionType(
            raw=" | ".join(t.raw for t in types),
            types=types,
        )

    def _build_type_identifier(self, node: Node) -> TSPredefinedType:
        return TSPredefinedType(raw=node.text.decode())

    def _build_type_arguments(self, node: Node) -> list[TSType]:
        args = []
        for child in node.children:
            if child.type not in ("<", ">", ","):
                t = self._build_type_node(child)
                if t is not None:
                    args.append(t)
        return args

    # ── Export / re-exports ────────────────────────────────────

    def _build_export_statement(self, node: Node) -> TSNode | None:
        for child in node.children:
            if child.type != "export":
                stmt = self._build_node(child)
                if stmt is not None:
                    return stmt
        return None
