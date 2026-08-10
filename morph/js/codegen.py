from __future__ import annotations

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
    TSObjectLiteral,
    TSObjectProperty,
    TSForStatement,
    TSFunctionDeclaration,
    TSFunctionExpression,
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

_CPP_NATIVE_TYPES: dict[str, str] = {
    "int":      "int",
    "int32":    "int32_t",
    "int64":    "int64_t",
    "uint":     "unsigned int",
    "uint32":   "uint32_t",
    "uint64":   "uint64_t",
    "float":    "float",
    "double":   "double",
    "bool":     "bool",
    "char":     "char",
    "size_t":   "size_t",
    "byte":     "uint8_t",
}

_PRIMITIVE_MAP: dict[str, str] = {
    "string": "JsString",
    "number": "JsNumber",
    "boolean": "JsBoolean",
    "void": "void",
    "any": "JsValue",
    "never": "void",
    "bigint": "int64_t",
    "symbol": "void*",
    "object": "JsObject",
    "undefined": "JsUndefined",
    "null": "JsNull",
}

_CONTEXTUAL_TYPES: dict[str, str] = {
    "Number": "JsNumber",
    "String": "JsString",
    "Boolean": "JsBoolean",
    "HTMLElement": "MorphNode*",
    "Element": "MorphNode*",
    "Node": "MorphNode*",
    "MouseEvent": "MorphEvent*",
    "Event": "MorphEvent*",
    "KeyboardEvent": "MorphEvent*",
    "FocusEvent": "MorphEvent*",
    "Promise": "auto",
}

_TYPE_TO_HEADER: dict[str, str] = {
    "std::string":      "<string>",
    "std::string_view": "<string_view>",
    "std::vector":      "<vector>",
    "std::optional":    "<optional>",
    "std::variant":     "<variant>",
    "std::format":      "<format>",
    "std::println":     "<print>",
    "std::shared_ptr":  "<memory>",
    "std::make_shared": "<memory>",
    "JsNumber":         "\"../../morph/runtime/types/js_types.h\"",
    "JsString":         "\"../../morph/runtime/types/js_types.h\"",
    "JsBoolean":        "\"../../morph/runtime/types/js_types.h\"",
    "JsArray":          "\"../../morph/runtime/types/js_types.h\"",
    "JsObject":         "\"../../morph/runtime/types/js_types.h\"",
    "JsValue":          "\"../../morph/runtime/types/js_types.h\"",
    "JsUndefined":      "\"../../morph/runtime/types/js_types.h\"",
    "JsNull":           "\"../../morph/runtime/types/js_types.h\"",
    "morph::Result":    "\"../../morph/runtime/reactivity/promise.h\"",
    "morph::Task":      "\"../../morph/runtime/reactivity/task.h\"",
    "morph::net":       "\"../../morph/runtime/net/net.h\"",
    "morph::net::Response": "\"../../morph/runtime/net/net.h\"",
}

_STRING_TYPES = frozenset(["JsString", "const JsString", "std::string", "const std::string", "std::string_view", "const char*"])

_CLASS_NAMES: set[str] = set()
_INTERFACE_PROPS: dict[str, dict[str, str]] = {}


def register_class_name(name: str) -> None:
    _CLASS_NAMES.add(name)


def _resolve_type(ts_type: TSType | None, context_hint: str = "auto",
                  template_params: frozenset[str] | None = None,
                  wrap_shared: bool = False) -> str:
    if ts_type is None:
        return context_hint
    if isinstance(ts_type, TSPredefinedType):
        raw = ts_type.raw
        # Check native escape hatch first (int, float, etc.)
        native = _CPP_NATIVE_TYPES.get(raw)
        if native is not None:
            return native
        mapped = _PRIMITIVE_MAP.get(raw, _CONTEXTUAL_TYPES.get(raw))
        if mapped is not None:
            return mapped
        if wrap_shared and (raw in _CLASS_NAMES or (template_params and raw in template_params)):
            return f"std::shared_ptr<{raw}>"
        return raw
    def _is_plain_cpp_type(name: str) -> bool:
        return name.isidentifier() or name.startswith("std::")

    if isinstance(ts_type, TSArrayType):
        elem = _resolve_type(ts_type.element_type, "auto", template_params, wrap_shared=True)
        # If element is a Js runtime type, use JsArray for JS-like reference semantics
        if elem.startswith("Js") or not _is_plain_cpp_type(elem):
            return "JsArray"
        return f"std::vector<{elem}>"
    if isinstance(ts_type, TSGenericType):
        name = ts_type.name
        if name in ("Array", "ReadonlyArray"):
            elem = _resolve_type(ts_type.type_args[0], "auto", template_params, wrap_shared=True) if ts_type.type_args else "auto"
            if elem.startswith("Js") or not _is_plain_cpp_type(elem):
                return "JsArray"
            return f"std::vector<{elem}>"
        if name in ("Promise", "Readonly"):
            return _resolve_type(ts_type.type_args[0], "auto", template_params) if ts_type.type_args else "auto"
        if name == "Record":
            return "std::unordered_map<std::string, auto>"
        if name == "Partial":
            return "auto"
        return name
    if isinstance(ts_type, TSUnionType):
        return "auto"
    return ts_type.raw


def _is_string_type(cpp_type: str) -> bool:
    base = cpp_type.removeprefix("const ").removesuffix("&").strip()
    return base in _STRING_TYPES or base == "std::string"


def _param_type(cpp_type: str) -> str:
    if cpp_type == "JsString" or cpp_type == "const JsString":
        return "const JsString&"
    if cpp_type == "std::string" or cpp_type == "const std::string":
        return "std::string_view"
    if _is_string_type(cpp_type):
        return f"const {cpp_type}&"
    return cpp_type


def _headers_for(cpp_type: str) -> list[str]:
    headers = []
    for prefix, header in _TYPE_TO_HEADER.items():
        if prefix in cpp_type:
            headers.append(header)
    return headers


_INDENT = "    "


class TSToCppTranslator:
    """Translates TypeScript AST → C++ code with automatic #include detection."""

    def __init__(self, indent_level: int = 1, event_handler: bool = False,
                 state_vars: dict[str, str] | None = None):
        self._indent = _INDENT * indent_level
        self._needed: set[str] = set()
        self._template_params: frozenset[str] = frozenset()
        self._is_interface_method = False
        self._interface_name: str | None = None
        self._class_name: str | None = None
        self._shared_ptr_vars: set[str] = set()
        self._var_types: dict[str, str] = {}
        self._fn_expr_depth: int = 0
        self._fn_body_depth: int = 0
        self._event_handler = event_handler
        # Map of JS identifier → C++ expression for state variables
        # e.g. {"count": "__st_count.get()", "setCount": "__st_count.set"}
        self._state_vars: dict[str, str] = state_vars or {}
        # Set to True when a while(true) loop is encountered within a function
        self._has_infinite_loop: bool = False
        # Depth of enclosing `async` functions; >0 makes `return` → `co_return`
        self._is_async_fn: int = 0
        # Async main runs inside a void morph::Task; drop returned values
        self._drop_return_value: bool = False
    

    def _need(self, cpp_type: str) -> None:
        for h in _headers_for(cpp_type):
            self._needed.add(h)

    def generate_includes(self) -> str:
        if not self._needed:
            return ""
        return "\n".join(f"#include {h}" for h in sorted(self._needed))

    def translate(self, node: TSNode) -> str:
        if isinstance(node, TSProgram):
            return self._translate_program(node)
        return self._translate_node(node)

    def _sub(self) -> TSToCppTranslator:
        sub = TSToCppTranslator(self._indent.count(_INDENT) + 1, event_handler=self._event_handler,
                                state_vars=self._state_vars)
        sub._template_params = self._template_params
        sub._is_interface_method = self._is_interface_method
        sub._interface_name = self._interface_name
        sub._class_name = self._class_name
        sub._shared_ptr_vars = self._shared_ptr_vars
        sub._var_types = self._var_types
        sub._fn_expr_depth = self._fn_expr_depth
        sub._fn_body_depth = self._fn_body_depth
        sub._has_infinite_loop = self._has_infinite_loop
        sub._is_async_fn = self._is_async_fn
        sub._drop_return_value = self._drop_return_value
        return sub

    def _merge(self, child: TSToCppTranslator) -> None:
        self._needed |= child._needed
        self._has_infinite_loop = self._has_infinite_loop or child._has_infinite_loop
        self._is_async_fn = max(self._is_async_fn, child._is_async_fn)
        self._drop_return_value = self._drop_return_value or child._drop_return_value

    def _translate_node(self, node: TSNode) -> str:
        if isinstance(node, TSBlockStatement):
            return self._block(node)
        if isinstance(node, TSExpressionStatement):
            return self._expression_statement(node)
        if isinstance(node, TSReturnStatement):
            return self._return_statement(node)
        if isinstance(node, TSVariableDeclaration):
            return self._variable_declaration(node)
        if isinstance(node, TSFunctionDeclaration):
            return self._function_declaration(node)
        if isinstance(node, TSArrowFunction):
            return self._arrow_function(node)
        if isinstance(node, TSIfStatement):
            return self._if_statement(node)
        if isinstance(node, TSWhileStatement):
            return self._while_statement(node)
        if isinstance(node, TSForStatement):
            return self._for_statement(node)
        if isinstance(node, TSBinaryExpression):
            return self._binary_expression(node)
        if isinstance(node, TSUnaryExpression):
            return self._unary_expression(node)
        if isinstance(node, TSUpdateExpression):
            return self._update_expression(node)
        if isinstance(node, TSAssignmentExpression):
            return self._assignment_expression(node)
        if isinstance(node, TSCallExpression):
            return self._call_expression(node)
        if isinstance(node, TSAwaitExpression):
            return self._await_expression(node)
        if isinstance(node, TSMemberExpression):
            return self._member_expression(node)
        if isinstance(node, TSIdentifier):
            if node.name in self._state_vars:
                return self._state_vars[node.name]
            return node.name
        if isinstance(node, TSLiteral):
            return self._literal(node)
        if isinstance(node, TSTemplateLiteral):
            return self._template_literal(node)
        if isinstance(node, TSParenthesizedExpression):
            return f"({self._translate_node(node.expression)})"
        if isinstance(node, TSProgram):
            return self._translate_program(node)
        # OOP nodes
        if isinstance(node, TSThisExpression):
            return "_jsThis" if self._fn_expr_depth > 0 else "this"
        if isinstance(node, TSFunctionExpression):
            return self._function_expression(node)
        if isinstance(node, TSSuperExpression):
            return "super"
        if isinstance(node, TSNewExpression):
            return self._new_expression(node)
        if isinstance(node, TSPropertyDefinition):
            return self._property_definition(node)
        if isinstance(node, TSMethodDefinition):
            return self._method_definition(node)
        if isinstance(node, TSConstructor):
            return self._constructor_definition(node)
        if isinstance(node, TSClassDeclaration):
            return self._class_declaration(node)
        if isinstance(node, TSInterfaceDeclaration):
            return self._interface_declaration(node)
        if isinstance(node, TSTypeParameter):
            return self._type_parameter(node)
        if isinstance(node, TSArrayLiteral):
            return self._array_literal(node)
        if isinstance(node, TSObjectLiteral):
            return self._object_literal(node)
        if isinstance(node, TSTernaryExpression):
            return self._ternary_expression(node)
        if isinstance(node, TSBreakStatement):
            return "break;"
        if isinstance(node, TSContinueStatement):
            return "continue;"
        if isinstance(node, TSDoWhileStatement):
            return self._do_while_statement(node)
        if isinstance(node, TSSwitchStatement):
            return self._switch_statement(node)
        if isinstance(node, TSSequenceExpression):
            return self._sequence_expression(node)
        if isinstance(node, TSTryStatement):
            return self._try_statement(node)
        if isinstance(node, TSThrowStatement):
            return self._throw_statement(node)
        return f"/* unhandled: {type(node).__name__} */"

    def _is_main_call(self, stmt: TSNode) -> bool:
        """Check if a statement is a call to main() at file scope."""
        if not isinstance(stmt, TSExpressionStatement):
            return False
        expr = stmt.expression
        if not isinstance(expr, TSCallExpression):
            return False
        if not isinstance(expr.callee, TSIdentifier):
            return False
        return expr.callee.name == "main"

    def _translate_program(self, node: TSProgram) -> str:
        lines = []
        for stmt in node.statements:
            if self._is_main_call(stmt):
                continue
            code = self._translate_node(stmt)
            if code:
                lines.append(code)
        body = "\n".join(lines)
        if "<print>" in self._needed:
            self._needed.add("<iostream>")
            io_init = (
                '__attribute__((constructor)) static void _morph_io_init() {\n'
                '    std::ios_base::sync_with_stdio(false);\n'
                '    std::cin.tie(NULL);\n'
                '}'
            )
            body = f"{io_init}\n\n{body}"
        self._needed.add('"../../morph/runtime/types/js_types.h"')
        includes = self.generate_includes()
        if includes:
            return f"{includes}\n\n{body}"
        return body

    # ── Statements ─────────────────────────────────────────────

    def _block(self, node: TSBlockStatement) -> str:
        if not node.statements:
            return "{}"
        lines = ["{"]
        sub = self._sub()
        for stmt in node.statements:
            code = sub._translate_node(stmt)
            if code:
                lines.append(code)
        self._merge(sub)
        lines.append(f"{self._indent}}}")
        return "\n".join(lines)

    def _expression_statement(self, node: TSExpressionStatement) -> str:
        expr = self._translate_node(node.expression)
        return f"{self._indent}{expr};"

    def _return_statement(self, node: TSReturnStatement) -> str:
        if self._drop_return_value:
            return f"{self._indent}co_return;"
        kw = "co_return" if self._is_async_fn > 0 else "return"
        if node.argument is None:
            return f"{self._indent}{kw};"
        expr = self._translate_node(node.argument)
        return f"{self._indent}{kw} {expr};"

    def _infer_type_from_init(self, node: TSNode) -> str | None:
        if isinstance(node, TSLiteral):
            if isinstance(node.value, str):
                return "JsString"
            if isinstance(node.value, bool):
                return "JsBoolean"
            if isinstance(node.value, int | float):
                return "JsNumber"
        if isinstance(node, TSObjectLiteral):
            return "JsObject"
        if isinstance(node, TSArrayLiteral):
            return "JsArray"
        if isinstance(node, TSNewExpression):
            return "auto"
        return None

    def _variable_declaration(self, node: TSVariableDeclaration) -> str:
        cpp_type = _resolve_type(node.type_annotation, template_params=self._template_params)
        if cpp_type == "auto" and node.initializer is not None:
            inferred = self._infer_type_from_init(node.initializer)
            if inferred is not None:
                cpp_type = inferred
        self._need(cpp_type)
        name = node.name.name

        self._var_types[name] = cpp_type

        # JS const allows mutation, only add C++ const for primitives
        if node.kind == "const" and cpp_type in ("JsNumber", "JsBoolean", "JsString", "JsUndefined", "JsNull"):
            cpp_type = f"const {cpp_type}" if not cpp_type.startswith("const ") else cpp_type

        prefix = "static " if self._indent == "" else ""
        if node.initializer is not None:
            init = self._translate_node(node.initializer)
            if isinstance(node.initializer, TSNewExpression):
                self._shared_ptr_vars.add(name)
            return f"{self._indent}{prefix}{cpp_type} {name} = {init};"
        return f"{self._indent}{prefix}{cpp_type} {name}{{}};"

    @staticmethod
    def _has_return(body: TSNode) -> bool:
        if isinstance(body, TSReturnStatement):
            return True
        if isinstance(body, TSBlockStatement):
            return any(TSToCppTranslator._has_return(s) for s in body.statements)
        return False

    def _function_declaration(self, node: TSFunctionDeclaration) -> str:
        self._has_infinite_loop = False  # reset for this function

        is_async = node.is_async
        if node.return_type is None:
            if node.name == "main":
                ret = "int"
            elif is_async:
                ret = "JsValue"
            elif self._has_return(node.body):
                ret = "auto"
            else:
                ret = "void"
        else:
            ret = _resolve_type(node.return_type, template_params=self._template_params)

        params = self._format_params(node.params)

        if node.name == "main" and is_async:
            return self._async_main(node, ret, params)

        if is_async:
            ret = self._async_result_type(ret)
        self._need(ret)
        self._fn_body_depth += 1
        if is_async:
            self._is_async_fn += 1
        body = self._translate_node(node.body)
        if is_async:
            self._is_async_fn -= 1
        self._fn_body_depth -= 1
        # After body translation, check for infinite loops
        if self._has_infinite_loop and node.name != "main" and not is_async:
            ret = "morph::Task"
            self._needed.add('"../../morph/runtime/reactivity/task.h"')

        tpl = ""
        if node.type_parameters:
            tpl = f"template <{', '.join(self._translate_node(tp) for tp in node.type_parameters)}>\n"

        header = f'{ret} {node.name}({params})'
        if node.name != "main":
            header = f'static inline\n{header}'
        lines = [f"{tpl}{header}"]
        lines.append(body)
        return "\n".join(lines)

    def _async_main(self, node: TSFunctionDeclaration, ret: str, params: str) -> str:
        """Generate an `int main()` that runs an async JS main as a coroutine."""
        self._needed.add('"../../morph/runtime/reactivity/task.h"')
        self._is_async_fn += 1
        self._drop_return_value = True
        body = self._translate_node(node.body).rstrip()
        self._drop_return_value = False
        self._is_async_fn -= 1
        if body.endswith("}"):
            body = body[:-1].rstrip()
        body += "\nco_return;"
        body += "\n}"
        return (
            "int main() {\n"
            f"    morph::Task _main_task = [&]() -> morph::Task {{\n"
            f"{body}\n"
            "    }();\n"
            "    while (!_main_task.done()) {\n"
            "        morph::process_tasks();\n"
            "    }\n"
            "    return 0;\n"
            "}"
        )

    def _async_result_type(self, resolved: str) -> str:
        """Map a resolved C++ return type to a coroutine return type for async fns."""
        if resolved in ("void", "auto"):
            return "morph::Task"
        if resolved.startswith("morph::Result") or resolved.startswith("morph::Task"):
            return resolved
        return f"morph::Result<{resolved}>"

    def _function_expression(self, node: TSFunctionExpression) -> str:
        self._fn_expr_depth += 1
        is_async = node.is_async
        ret = "JsValue"
        if is_async:
            ret = self._async_result_type(_resolve_type(node.return_type, "JsValue",
                                                        template_params=self._template_params))
            self._need(ret)
        if isinstance(node.body, TSBlockStatement):
            if is_async:
                self._is_async_fn += 1
                body = self._translate_node(node.body)
                self._is_async_fn -= 1
            else:
                body = self._translate_node(node.body)
        else:
            expr = self._translate_node(node.body)
            kw = "co_return" if is_async else "return"
            body = f"{{ {kw} {expr}; }}"
        self._fn_expr_depth -= 1
        self._need("JsValue")
        if is_async:
            return f"+[](JsValue _jsThis) -> {ret} {body}"
        return f"+[](JsValue _jsThis) -> JsValue {body}"

    def _arrow_function(self, node: TSArrowFunction) -> str:
        capture = "[&]" if self._fn_body_depth > 0 else "[]"

        if self._event_handler:
            self._need("JsObject")
            if node.params:
                param_strs = []
                for p in node.params:
                    self._var_types[p.name.name] = "JsObject"
                    param_strs.append(f"JsObject {p.name.name}")
                params = ", ".join(param_strs)
            else:
                params = "JsObject"
            ret = "void"
            if isinstance(node.body, TSBlockStatement):
                body = self._translate_node(node.body)
                return f"{capture}({params}) -> void {body}"
            expr = self._translate_node(node.body)
            return f"{capture}({params}) -> void {{ {expr}; }}"

        ret = _resolve_type(node.return_type, "auto", template_params=self._template_params)
        is_async = node.is_async
        if is_async:
            ret = self._async_result_type(ret)
        self._need(ret)
        params = self._format_params(node.params)
        if isinstance(node.body, TSBlockStatement):
            if is_async:
                self._is_async_fn += 1
                body = self._translate_node(node.body)
                self._is_async_fn -= 1
            else:
                body = self._translate_node(node.body)
            return f"{capture}({params}) -> {ret} {body}"
        expr = self._translate_node(node.body)
        kw = "co_return" if is_async else "return"
        return f"{capture}({params}) -> {ret} {{ {kw} {expr}; }}"

    def _if_statement(self, node: TSIfStatement) -> str:
        cond = self._translate_node(node.condition)
        cons = self._translate_node(node.consequence)

        if isinstance(node.consequence, TSBlockStatement):
            result = f"{self._indent}if ({cond}) {cons}"
        else:
            bi = _INDENT * (self._indent.count(_INDENT) + 1)
            result = f"{self._indent}if ({cond}) {{\n{bi}{cons};\n{self._indent}}}"

        if node.alternate is not None:
            alt = self._translate_node(node.alternate)
            if isinstance(node.alternate, (TSIfStatement, TSBlockStatement)):
                result += f" else {alt}"
            else:
                bi = _INDENT * (self._indent.count(_INDENT) + 1)
                result += f" else {{\n{bi}{alt};\n{self._indent}}}"
        return result

    def _while_statement(self, node: TSWhileStatement) -> str:
        cond = self._translate_node(node.condition)
        body = self._translate_node(node.body)

        # Detect infinite loops: while(true), while(1), while((true)), etc.
        is_infinite = cond.strip().strip("()") in ("true", "1")

        if is_infinite and self._fn_body_depth > 0:
            self._has_infinite_loop = True
            self._needed.add('"../../morph/runtime/reactivity/task.h"')

            bi = _INDENT * (self._indent.count(_INDENT) + 1)
            if isinstance(node.body, TSBlockStatement):
                body_stripped = body.rstrip()
                if body_stripped.endswith("}"):
                    body = (body_stripped[:-1].rstrip() + "\n" + bi
                            + "co_await morph::next_frame();\n" + self._indent + "}")
                return f"{self._indent}while ({cond}) {body}"
            else:
                return (f"{self._indent}while ({cond}) {{\n{bi}{body};\n{bi}"
                        f"co_await morph::next_frame();\n{self._indent}}}")

        # Normal while (not infinite)
        if isinstance(node.body, TSBlockStatement):
            return f"{self._indent}while ({cond}) {body}"
        bi = _INDENT * (self._indent.count(_INDENT) + 1)
        return f"{self._indent}while ({cond}) {{\n{bi}{body};\n{self._indent}}}"

    def _do_while_statement(self, node: TSDoWhileStatement) -> str:
        body = self._translate_node(node.body)
        cond = self._translate_node(node.condition)
        return f"{self._indent}do {body} while ({cond});"

    def _switch_statement(self, node: TSSwitchStatement) -> str:
        disc = self._translate_node(node.discriminant)
        disc = f"({disc}).as_int()"
        lines = [f"{self._indent}switch ({disc}) {{"]
        bi = _INDENT * (self._indent.count(_INDENT) + 1)
        for case_node in node.cases:
            if isinstance(case_node, TSCaseClause):
                test = self._translate_node(case_node.test)
                lines.append(f"{bi}case {test}:")
                for stmt in case_node.consequence:
                    c = self._translate_node(stmt)
                    lines.append(f"{bi}    {c}")
            elif isinstance(case_node, TSDefaultClause):
                lines.append(f"{bi}default:")
                for stmt in case_node.consequence:
                    c = self._translate_node(stmt)
                    lines.append(f"{bi}    {c}")
        lines.append(f"{self._indent}}}")
        return "\n".join(lines)

    def _try_statement(self, node: TSTryStatement) -> str:
        bi = _INDENT * (self._indent.count(_INDENT) + 1)
        lines: list[str] = []

        # try body — strip trailing "}" so we can append catch blocks
        raw_body = self._translate_node(node.body)
        body_stripped = raw_body.rstrip()
        if body_stripped.endswith("}"):
            body_stripped = body_stripped[:-1]

        lines.append(f"{self._indent}try {body_stripped}")

        # catch clause
        if node.handler is not None:
            param = node.handler.param.name
            self._var_types[param] = "JsValue"
            h = self._translate_node(node.handler.body)
            h_lines = h.split("\n")
            lines.append(f"{self._indent}}} catch (JsValue& {param}) {{")
            for l in h_lines[1:-1]:
                lines.append(l)
            if node.finalizer is not None:
                f = self._translate_node(node.finalizer)
                f_lines = f.split("\n")
                for l in f_lines[1:-1]:
                    lines.append(l)
            lines.append(f"{self._indent}}}")

        # catch-all for exception path (ensures finally runs on any throw)
        if node.finalizer is not None:
            f = self._translate_node(node.finalizer)
            f_lines = f.split("\n")
            if node.handler is not None:
                # Replace the last closing '}' with '} catch (...) {'
                if lines and lines[-1].strip() == "}":
                    lines[-1] = f"{self._indent}}} catch (...) {{"
                else:
                    lines.append(f"{self._indent}}} catch (...) {{")
            else:
                lines.append(f"{self._indent}}} catch (...) {{")
            for l in f_lines[1:-1]:
                lines.append(l)
            lines.append(f"{bi}throw;")
            for l in f_lines[1:-1]:
                lines.append(l)
            lines.append(f"{self._indent}}}")
            # Normal exit path — inner lines of finally block at current indent
            for l in f_lines[1:-1]:
                stripped = l.strip()
                if stripped:
                    lines.append(f"{self._indent}{stripped}")

        # Close try if no catches
        if node.handler is None and node.finalizer is None:
            lines.append(f"{self._indent}}}")

        return "\n".join(lines)

    def _throw_statement(self, node: TSThrowStatement) -> str:
        arg = self._translate_node(node.argument)
        self._need("JsValue")
        return f"{self._indent}throw JsValue({arg});"

    def _for_statement(self, node: TSForStatement) -> str:
        init = self._translate_node(node.init) if node.init else ""
        cond = self._translate_node(node.condition) if node.condition else ""
        update = self._translate_node(node.update) if node.update else ""
        body = self._translate_node(node.body)

        init_clean = init.strip()
        if init_clean.endswith(";"):
            init_clean = init_clean[:-1]

        # Detect infinite for(;;)
        is_infinite = cond.strip() == "" and init_clean == "" and update.strip() == ""

        if is_infinite and self._fn_body_depth > 0:
            self._has_infinite_loop = True
            self._needed.add('"../../morph/runtime/reactivity/task.h"')
            bi = _INDENT * (self._indent.count(_INDENT) + 1)
            if isinstance(node.body, TSBlockStatement):
                body_stripped = body.rstrip()
                if body_stripped.endswith("}"):
                    body = (body_stripped[:-1].rstrip() + "\n" + bi
                            + "co_await morph::next_frame();\n" + self._indent + "}")
                return f"{self._indent}for ({init_clean}; {cond}; {update}) {body}"
            else:
                return (f"{self._indent}for ({init_clean}; {cond}; {update}) {{\n{bi}{body};\n{bi}"
                        f"co_await morph::next_frame();\n{self._indent}}}")

        if isinstance(node.body, TSBlockStatement):
            return f"{self._indent}for ({init_clean}; {cond}; {update}) {body}"
        bi = _INDENT * (self._indent.count(_INDENT) + 1)
        return f"{self._indent}for ({init_clean} {cond}; {update}) {{\n{bi}{body};\n{self._indent}}}"

    # ── Expressions ───────────────────────────────────────────

    def _is_number_literal(self, node: TSNode) -> bool:
        return isinstance(node, TSLiteral) and isinstance(node.value, (int, float))

    def _is_string_literal(self, node: TSNode) -> bool:
        return isinstance(node, TSLiteral) and isinstance(node.value, str)

    def _binary_expression(self, node: TSBinaryExpression) -> str:
        left = self._translate_node(node.left)
        right = self._translate_node(node.right)
        op = {"===": "==", "!==": "!=",
              "**": "/* pow */", ">>>": "/* >>> */"}.get(node.operator, node.operator)

        left_is_str_lit = self._is_string_literal(node.left)
        right_is_str_lit = self._is_string_literal(node.right)
        left_is_num_lit = self._is_number_literal(node.left)
        right_is_num_lit = self._is_number_literal(node.right)

        # Prevent C++ pointer arithmetic: wrap string literals in JsString
        if left_is_str_lit:
            left = f"JsString({left})"
        if right_is_str_lit:
            right = f"JsString({right})"

        # JS string concat: prevent int + char* ambiguity
        if op == "+":
            if left_is_num_lit and right_is_str_lit:
                left = f"JsNumber({left})"
            elif right_is_num_lit and left_is_str_lit:
                right = f"JsNumber({right})"

        if node.operator == "??":
            return f"(JsValue({left}).is_undefined() || JsValue({left}).is_null() ? JsValue({right}) : JsValue({left}))"

        return f"{left} {op} {right}"

    def _unary_expression(self, node: TSUnaryExpression) -> str:
        arg = self._translate_node(node.argument)
        op = {"typeof": "/* typeof */", "void": "/* void */",
              "delete": "/* delete */"}.get(node.operator, node.operator)
        if node.prefix:
            return f"{op}{arg}"
        return f"{arg}{op}"

    def _ternary_expression(self, node: TSTernaryExpression) -> str:
        cond = self._translate_node(node.condition)
        cons = self._translate_node(node.consequent)
        alt = self._translate_node(node.alternate)
        return f"({cond} ? {cons} : {alt})"

    def _sequence_expression(self, node: TSSequenceExpression) -> str:
        return ", ".join(self._translate_node(e) for e in node.expressions)

    def _update_expression(self, node: TSUpdateExpression) -> str:
        arg = self._translate_node(node.argument)
        if isinstance(node.argument, TSIdentifier):
            arg = node.argument.name
        if node.prefix:
            return f"{node.operator}{arg}"
        return f"{arg}{node.operator}"

    def _assignment_expression(self, node: TSAssignmentExpression) -> str:
        left = self._translate_node(node.left)
        right = self._translate_node(node.right)
        op = {"&&=": "/* &&= */", "||=": "/* ||= */",
              "??=": "/* ??= */"}.get(node.operator, node.operator)

        # Detect circular reference: obj.self = obj  or  obj["self"] = obj
        if op == "=" and isinstance(node.right, TSIdentifier):
            rhs_name = node.right.name
            lhs_base = self._get_member_base(node.left)
            if lhs_base is not None and lhs_base == rhs_name:
                t = self._var_types.get(rhs_name)
                if t in ("JsObject", "JsArray"):
                    raise ValueError(
                        f"circular reference detected: "
                        f"'{rhs_name}' assigned to itself via property access — "
                        f"this would cause a memory leak"
                    )

        return f"({left} {op} {right})"

    @staticmethod
    def _get_member_base(node: TSNode) -> str | None:
        if isinstance(node, TSIdentifier):
            return node.name
        if isinstance(node, TSMemberExpression):
            return TSToCppTranslator._get_member_base(node.object)
        return None

    def _call_expression(self, node: TSCallExpression) -> str:
        callee = self._translate_node(node.callee)

        # JS fetch() → morph::net::fetch() — awaitable HTTP GET
        if (isinstance(node.callee, TSIdentifier)
                and node.callee.name == "fetch"):
            self._needed.add('"../../morph/runtime/net/net.h"')
            args = [self._translate_node(a) for a in node.arguments]
            return f"morph::net::fetch({', '.join(args)})"

        # Detect console.log
        if (isinstance(node.callee, TSMemberExpression)
                and isinstance(node.callee.object, TSIdentifier)
                and node.callee.object.name == "console"
                and isinstance(node.callee.property, TSIdentifier)
                and not node.callee.computed
                and node.callee.property.name in ("log", "warn", "error", "info")):
            self._need("std::println")
            fmt_chars: list[str] = []
            val_args: list[str] = []
            for a in node.arguments:
                if isinstance(a, TSTemplateLiteral):
                    fstr, targs = self._extract_template_parts(a)
                    fmt_chars.append(fstr)
                    val_args.extend(targs)
                elif isinstance(a, TSLiteral) and isinstance(a.value, str):
                    fmt_chars.append(a.value.replace("{", "{{").replace("}", "}}"))
                else:
                    fmt_chars.append("{}")
                    val_args.append(self._translate_node(a))
            fmt_str = " ".join(fmt_chars)
            if not val_args:
                return f'std::println("{{}}", "{fmt_str}")'
            return f'std::println("{fmt_str}", {", ".join(val_args)})'

        # Detect .push(item) → .push(item)
        if (isinstance(node.callee, TSMemberExpression)
                and isinstance(node.callee.property, TSIdentifier)
                and node.callee.property.name == "push"
                and len(node.arguments) == 1):
            obj = self._translate_node(node.callee.object)
            arg = self._translate_node(node.arguments[0])
            return f"{obj}.push({arg})"

        # ── JS built-in timers: setTimeout / setInterval / clearTimeout / clearInterval ──
        if (isinstance(node.callee, TSIdentifier)
                and node.callee.name in ("setTimeout", "setInterval", "clearTimeout", "clearInterval")):
            self._needed.add('"../../morph/runtime/reactivity/task.h"')
            if node.callee.name == "clearTimeout" or node.callee.name == "clearInterval":
                args = [self._translate_node(a) for a in node.arguments]
                return f"morph::clear_timer({', '.join(args)})"
            else:
                # First arg: function, second arg: delay in ms
                fn_translated = self._translate_node(node.arguments[0])
                delay = self._translate_node(node.arguments[1]) if len(node.arguments) > 1 else "0"
                cpp_fn = f"morph::{('set_timeout' if node.callee.name == 'setTimeout' else 'set_interval')}"
                return f"{cpp_fn}(std::function<void()>({fn_translated}), {delay})"

        # State var getter: elapsed() → __st_elapsed.get() (no extra parens)
        # State var setter: setElapsed(5) → __st_elapsed.set(5)
        if isinstance(node.callee, TSIdentifier) and node.callee.name in self._state_vars:
            callee = self._state_vars[node.callee.name]
            args = [self._translate_node(a) for a in node.arguments]
            if args:
                return f"{callee}({', '.join(args)})"
            return callee

        args = [self._translate_node(a) for a in node.arguments]
        # super(args) in constructor context
        if isinstance(node.callee, TSSuperExpression):
            return f"super({', '.join(args)})"
        # Method call: obj.method(args)
        if isinstance(node.callee, TSMemberExpression) and not node.callee.computed:
            this_obj = self._translate_node(node.callee.object)
            # C++ class instance (shared_ptr) — no this injection
            if self._looks_like_shared_ptr(this_obj):
                return f"{callee}({', '.join(args)})"
            # Non-JsObject types (JsString, JsNumber, JsValue, etc.) use direct C++ methods
            if not self._is_js_object_type(node.callee.object):
                return f"{callee}({', '.join(args)})"
            # JsObject function property — pass obj as this: obj["method"](obj, args)
            return f"{callee}({this_obj}{', ' + ', '.join(args) if args else ''})"
        return f"{callee}({', '.join(args)})"

    def _await_expression(self, node: TSAwaitExpression) -> str:
        arg = self._translate_node(node.argument)
        return f"co_await {arg}"

    def _is_js_object_type(self, obj_node: TSNode) -> bool:
        if isinstance(obj_node, TSIdentifier):
            return self._var_types.get(obj_node.name) == "JsObject"
        if isinstance(obj_node, TSMemberExpression):
            return True
        return False

    def _looks_like_shared_ptr(self, expr: str) -> bool:
        if expr in self._shared_ptr_vars:
            return True
        if expr.endswith("]"):
            return True
        return False

    def _member_expression(self, node: TSMemberExpression) -> str:
        obj = self._translate_node(node.object)
        prop = self._translate_node(node.property)

        # this.prop → _jsThis["prop"] in function expression, this->prop otherwise
        if isinstance(node.object, TSThisExpression):
            if self._fn_expr_depth > 0:
                return f'_jsThis["{prop}"]'
            return f"this->{prop}"

        # super.prop → Base::prop
        if isinstance(node.object, TSSuperExpression):
            if self._class_name:
                return f"{self._class_name}::{prop}"
            return f"/* super */{prop}"

        # .length → .size()
        if (isinstance(node.property, TSIdentifier)
                and node.property.name == "length"
                and not node.computed):
            return f"{obj}.length()"

        if node.computed:
            return f"{obj}[{prop}]"

        # Resolve non-computed property access to ["key"] for JsObject/JsValue
        if self._should_use_bracket_access(node.object, obj):
            return f'{obj}["{prop}"]'

        if self._looks_like_shared_ptr(obj):
            return f"{obj}->{prop}"

        # Static class member: ClassName.member → ClassName::member
        if isinstance(node.object, TSIdentifier) and node.object.name in _CLASS_NAMES:
            return f"{obj}::{prop}"

        return f"{obj}.{prop}"

    def _should_use_bracket_access(self, obj_node: TSNode, obj_str: str) -> bool:
        if isinstance(obj_node, TSIdentifier):
            return self._var_types.get(obj_node.name) in ("JsObject", "JsValue")
        if isinstance(obj_node, TSMemberExpression):
            return True
        return False

    def _literal(self, node: TSLiteral) -> str:
        if node.value is None:
            if node.raw == "undefined":
                return "JsUndefined{}"
            return "JsNull{}"
        if isinstance(node.value, bool):
            return "true" if node.value else "false"
        if isinstance(node.value, str):
            return f'"{node.value}"'
        if isinstance(node.value, float):
            v = node.value
            if v == int(v):
                return f"{int(v)}.0"
            return f"{v}"
        if isinstance(node.value, int):
            # If the integer doesn't fit in int64, emit as string literal
            # so C++ doesn't truncate it during parsing
            if node.value > 2**63 - 1 or node.value < -(2**63):
                return f'JsNumber("{node.value}")'
            return str(node.value)
        return str(node.value)

    def _extract_template_parts(self, node: TSTemplateLiteral) -> tuple[str, list[str]]:
        string_parts: list[str] = []
        expr_parts: list[str] = []
        for part in node.parts:
            if isinstance(part, str):
                string_parts.append(part)
            else:
                expr_parts.append(self._translate_node(part))

        fmt_str = ""
        args: list[str] = []
        si, ei = 0, 0
        while si < len(string_parts) or ei < len(expr_parts):
            if si < len(string_parts):
                fmt_str += string_parts[si].replace("{", "{{").replace("}", "}}")
                si += 1
            if ei < len(expr_parts):
                fmt_str += "{}"
                args.append(expr_parts[ei])
                ei += 1
        return fmt_str.replace("\\", "\\\\").replace('"', '\\"'), args

    def _template_literal(self, node: TSTemplateLiteral) -> str:
        fmt_str, args = self._extract_template_parts(node)
        if not args:
            return f'"{fmt_str}"'
        self._need("std::format")
        return f'std::format("{fmt_str}", {", ".join(args)})'

    # ── Helpers: type wrapping ─────────────────────────────────

    def _wrap_type(self, cpp_type: str) -> str:
        if cpp_type in self._template_params:
            self._need("std::shared_ptr")
            return f"std::shared_ptr<{cpp_type}>"
        return cpp_type

    def _wrap_element_type(self, cpp_type: str) -> str:
        """Wrap array element type in shared_ptr if it's a class/template type."""
        if cpp_type not in ("void", "bool", "auto") and not cpp_type.startswith("std::"):
            self._need("std::shared_ptr")
            return f"std::shared_ptr<{cpp_type}>"
        if cpp_type in self._template_params:
            self._need("std::shared_ptr")
            return f"std::shared_ptr<{cpp_type}>"
        return cpp_type

    # ── OOP: this, super, new ──────────────────────────────────

    def _new_expression(self, node: TSNewExpression) -> str:
        callee = self._translate_node(node.callee)
        args = [self._translate_node(a) for a in node.arguments]

        # Built-in Error constructor — creates a JsObject with name/message
        if callee == "Error":
            msg_arg = args[0] if args else 'JsString("")'
            self._need("JsObject")
            return f'JsObject{{{{"name", JsString("Error")}}, {{"message", {msg_arg}}}}}'

        if node.type_arguments:
            type_args = ", ".join(
                _resolve_type(t, template_params=self._template_params)
                for t in node.type_arguments
            )
            full_type = f"{callee}<{type_args}>"
        else:
            full_type = callee

        self._need("std::make_shared")
        return f"std::make_shared<{full_type}>({', '.join(args)})"

    # ── OOP: Array Literal ─────────────────────────────────────

    def _array_literal(self, node: TSArrayLiteral) -> str:
        if not node.elements:
            return "JsArray{}"
        elems = [self._translate_node(e) for e in node.elements]
        return "JsArray{" + ", ".join(elems) + "}"

    # ── OOP: Object Literal ────────────────────────────────────

    def _object_literal(self, node: TSObjectLiteral) -> str:
        if not node.properties:
            return "JsObject{}"
        pairs = []
        for prop in node.properties:
            val = self._translate_node(prop.value)
            pairs.append(f'{{"{prop.key}", {val}}}')
        return "JsObject{" + ", ".join(pairs) + "}"

    # ── OOP: Type Parameters ───────────────────────────────────

    def _type_parameter(self, node: TSTypeParameter) -> str:
        if node.constraint:
            return f"typename {node.name}"
        return f"typename {node.name}"

    # ── OOP: Property Definition ───────────────────────────────

    def _property_definition(self, node: TSPropertyDefinition) -> str:
        wrap = bool(self._template_params)
        cpp_type = _resolve_type(node.type_annotation, "auto",
                                 template_params=self._template_params,
                                 wrap_shared=wrap)
        cpp_type = self._wrap_type(cpp_type)
        self._need(cpp_type)
        prefix = "static inline " if node.static else ""
        if node.initializer is not None:
            init = self._translate_node(node.initializer)
            return f"{prefix}{cpp_type} {node.name} = {init};"
        return f"{prefix}{cpp_type} {node.name};"

    # ── OOP: Method Definition ─────────────────────────────────

    def _method_definition(self, node: TSMethodDefinition) -> str:
        wrap = bool(self._template_params)
        static_prefix = "static inline " if node.static else ""
        is_async = node.is_async
        if node.return_type is None:
            if is_async:
                ret = "JsValue"
            elif TSToCppTranslator._has_return(node.body):
                ret = "auto"
            else:
                ret = "void"
        else:
            ret = _resolve_type(node.return_type, "void", template_params=self._template_params, wrap_shared=wrap)
        if is_async:
            ret = self._async_result_type(ret)
        ret = self._wrap_type(ret)
        self._need(ret)
        params = self._format_params(node.params, wrap_shared=wrap)
        if is_async:
            self._is_async_fn += 1
            body = self._translate_node(node.body)
            self._is_async_fn -= 1
        else:
            body = self._translate_node(node.body)

        header = f'{static_prefix}{ret} {node.name}({params})'

        if self._is_interface_method:
            header += " = 0"
            return f"{header};"

        return f"{header} {body}"

    # ── OOP: Constructor Definition ────────────────────────────

    def _constructor_definition(self, node: TSConstructor,
                                 base_name: str | None = None,
                                 param_names: set[str] | None = None) -> str:
        class_name = self._class_name or ""
        params = self._format_constructor_params(node.params)
        param_names = param_names or {p.name.name for p in node.params}

        init_entries: list[str] = []
        body_stmts: list[TSNode] = []
        super_call_seen = False

        for stmt in node.body.statements:
            if not super_call_seen:
                super_info = self._detect_super_call(stmt)
                if super_info is not None:
                    super_call_seen = True
                    base_super_args = []
                    for a in super_info:
                        base_super_args.append(self._translate_super_arg(a, param_names))
                    base = base_name or "Base"
                    init_entries.append(f"{base}({', '.join(base_super_args)})")
                    continue
            body_stmts.append(stmt)

        remaining_stmts: list[TSNode] = []
        for stmt in body_stmts:
            assignment = self._detect_this_assignment(stmt)
            if assignment is not None:
                prop_name, rhs_node = assignment
                if isinstance(rhs_node, TSIdentifier) and rhs_node.name in param_names:
                    rhs_str = f"p_{rhs_node.name}"
                else:
                    rhs_str = self._translate_node(rhs_node)
                init_entries.append(f"{prop_name}(std::move({rhs_str}))")
            else:
                remaining_stmts.append(stmt)

        init_str = ""
        if init_entries:
            init_str = f"\n{_INDENT * (self._indent.count(_INDENT) + 1)}: {', '.join(init_entries)}"

        body_lines = []
        sub = self._sub()
        for stmt in remaining_stmts:
            code = sub._translate_node(stmt)
            if code:
                body_lines.append(code)
        self._merge(sub)

        if body_lines:
            body = "{\n" + "\n".join(body_lines) + f"\n{self._indent}}}"
        else:
            body = "{}"

        return f"{class_name}({params}){init_str} {body}"

    def _translate_super_arg(self, arg_node: TSNode, param_names: set[str]) -> str:
        if isinstance(arg_node, TSIdentifier) and arg_node.name in param_names:
            return f"std::move(p_{arg_node.name})"
        return self._translate_node(arg_node)

    def _detect_super_call(self, stmt: TSNode) -> list[TSNode] | None:
        """Detect super(args) call in constructor. Returns arg nodes or None."""
        if not isinstance(stmt, TSExpressionStatement):
            return None
        expr = stmt.expression
        if not isinstance(expr, TSCallExpression):
            return None
        if not isinstance(expr.callee, TSSuperExpression):
            return None
        return expr.arguments

    def _detect_this_assignment(self, stmt: TSNode) -> tuple[str, TSNode] | None:
        """Detect this.x = val in constructor body. Returns (property_name, rhs_node) or None."""
        if not isinstance(stmt, TSExpressionStatement):
            return None
        expr = stmt.expression
        if not isinstance(expr, TSAssignmentExpression):
            return None
        if expr.operator != "=":
            return None
        left = expr.left
        if not isinstance(left, TSMemberExpression):
            return None
        if not isinstance(left.object, TSThisExpression):
            return None
        if not isinstance(left.property, TSIdentifier):
            return None
        return (left.property.name, expr.right)

    def _format_constructor_params(self, params: list[TSVariableDeclaration]) -> str:
        """Format constructor params with p_ prefix to avoid shadowing."""
        parts = []
        for p in params:
            cpp_type = _resolve_type(p.type_annotation, "auto", template_params=self._template_params)
            self._need(cpp_type)
            param_type = _param_type(cpp_type)
            if param_type == "std::string_view":
                self._need("std::string_view")
            parts.append(f"{param_type} p_{p.name.name}")
        return ", ".join(parts)

    # ── OOP: Class Declaration ─────────────────────────────────

    def _class_declaration(self, node: TSClassDeclaration) -> str:
        lines: list[str] = []
        old_tp = self._template_params
        if node.name:
            register_class_name(node.name)

        # Template header
        if node.type_parameters:
            tp_names: list[str] = []
            tp_decls: list[str] = []
            for tp in node.type_parameters:
                tp_names.append(tp.name)
                tp_decls.append(self._translate_node(tp))
            self._template_params = self._template_params | frozenset(tp_names)
            lines.append(f"template <{', '.join(tp_decls)}>")

        # Class header with inheritance
        bases = []
        if node.extends:
            bases.append(f"public {node.extends}")
        for iface in node.implements:
            bases.append(f"public {iface}")
        heritage = f" : {', '.join(bases)}" if bases else ""
        lines.append(f"class {node.name}{heritage} {{")

        # Organize members by access
        access_order = ["public", "private", "protected"]
        access_sections: dict[str, list] = {a: [] for a in access_order}

        for m in node.members:
            acc = m.access_modifier if hasattr(m, 'access_modifier') and m.access_modifier else "public"
            if acc not in access_sections:
                acc = "public"
            access_sections[acc].append(m)

        old_class_name = self._class_name
        self._class_name = node.name

        first = True
        for access in access_order:
            members = access_sections[access]
            if not members:
                continue
            if not first:
                lines.append("")
            first = False
            lines.append(f"{access}:" if access != "public" else "public:")
            sub = self._sub()
            sub._template_params = self._template_params
            sub._class_name = node.name
            for m in members:
                if isinstance(m, TSConstructor):
                    param_names = {p.name.name for p in m.params}
                    code = sub._constructor_definition(
                        m, base_name=node.extends, param_names=param_names)
                else:
                    code = sub._translate_node(m)
                if code:
                    lines.append(f"{_INDENT}{code}")
            self._merge(sub)

        # Generate override methods for implemented interfaces
        if node.extends or node.implements:
            all_bases = [node.extends] if node.extends else []
            all_bases += list(node.implements)
            for base_name in all_bases:
                if base_name and base_name in _INTERFACE_PROPS:
                    for prop_name, prop_type in _INTERFACE_PROPS[base_name].items():
                        getter = f"get{prop_name[0].upper() + prop_name[1:]}"
                        override_code = f"{prop_type} {getter}() const override {{ return {prop_name}; }}"
                        lines.append(f"{_INDENT}{override_code}")

        lines.append("};")
        self._class_name = old_class_name
        self._template_params = old_tp
        return "\n".join(lines)

    # ── OOP: Interface Declaration ─────────────────────────────

    def _interface_declaration(self, node: TSInterfaceDeclaration) -> str:
        lines: list[str] = []
        if node.name:
            register_class_name(node.name)

        old_tp = self._template_params
        if node.type_parameters:
            tp_names: list[str] = []
            tp_decls: list[str] = []
            for tp in node.type_parameters:
                tp_names.append(tp.name)
                tp_decls.append(self._translate_node(tp))
            self._template_params = self._template_params | frozenset(tp_names)
            lines.append(f"template <{', '.join(tp_decls)}>")

        lines.append(f"class {node.name} {{")

        sub = self._sub()
        sub._template_params = self._template_params
        sub._interface_name = node.name

        # Register interface properties for override generation
        if node.name:
            _INTERFACE_PROPS[node.name] = {}

        prop_methods: list[str] = []
        for m in node.members:
            if isinstance(m, TSPropertyDefinition):
                cpp_type = _resolve_type(m.type_annotation, template_params=self._template_params)
                getter_name = f"get{m.name[0].upper() + m.name[1:]}"
                prop_methods.append(f"virtual {cpp_type} {getter_name}() const = 0;")
                if node.name:
                    _INTERFACE_PROPS[node.name][m.name] = cpp_type
            elif isinstance(m, TSMethodDefinition):
                ret = _resolve_type(m.return_type, "void", template_params=self._template_params)
                params = sub._format_params(m.params)
                prop_methods.append(f"virtual {ret} {m.name}({params}) = 0;")

        self._merge(sub)

        lines.append("public:")
        for pm in prop_methods:
            lines.append(f"{_INDENT}{pm}")
        lines.append(f"{_INDENT}virtual ~{node.name}() = default;")
        lines.append("};")

        self._template_params = old_tp
        return "\n".join(lines)

    # ── Helpers ────────────────────────────────────────────────

    def _format_params(self, params: list[TSVariableDeclaration],
                       wrap_shared: bool = False) -> str:
        parts = []
        for p in params:
            cpp_type = _resolve_type(p.type_annotation, "auto",
                                     template_params=self._template_params,
                                     wrap_shared=wrap_shared)
            cpp_type = self._wrap_type(cpp_type)
            self._need(cpp_type)
            param_type = _param_type(cpp_type)
            if param_type == "std::string_view":
                self._need("std::string_view")
            decl = f"{param_type} {p.name.name}"
            if p.initializer is not None:
                decl += f" = {self._translate_node(p.initializer)}"
            parts.append(decl)
        return ", ".join(parts)

    def _format_constructor_params(self, params: list[TSVariableDeclaration]) -> str:
        """Format constructor params with p_ prefix to avoid shadowing."""
        parts = []
        for p in params:
            cpp_type = _resolve_type(p.type_annotation, "auto", template_params=self._template_params)
            cpp_type = self._wrap_type(cpp_type)
            self._need(cpp_type)
            param_type = _param_type(cpp_type)
            if param_type == "std::string_view":
                self._need("std::string_view")
            parts.append(f"{param_type} p_{p.name.name}")
        return ", ".join(parts)

    @staticmethod
    def resolve_type(ts_type: TSType | None, context_hint: str = "auto") -> str:
        return _resolve_type(ts_type, context_hint)


def _is_string_literal(expr: str) -> bool:
    return expr.startswith('"') or expr.startswith("'")
