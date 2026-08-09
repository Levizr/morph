from __future__ import annotations

import tree_sitter_typescript as tsts
from tree_sitter import Language, Parser

from morph.js.ast_builder import TSAstBuilder
from morph.js.codegen import TSToCppTranslator


def _parse_ts(source: str):
    lang = Language(tsts.language_tsx())
    parser = Parser(lang)
    tree = parser.parse(source.encode("utf-8"))
    return tree.root_node


def _translate(source: str) -> str:
    root = _parse_ts(source)
    builder = TSAstBuilder()
    program = builder.build(root)
    translator = TSToCppTranslator(indent_level=0)
    return translator.translate(program)


# ── Variable Declarations ─────────────────────────────────


def test_let_number():
    code = _translate("let x: number = 42;")
    assert "JsNumber x = 42" in code


def test_const_string():
    code = _translate('const name: string = "hello";')
    assert '"hello"' in code
    assert "const JsString" in code


def test_let_boolean():
    code = _translate("let active: boolean = true;")
    assert "JsBoolean active" in code
    assert "true" in code


def test_let_no_type():
    code = _translate("let x = 5;")
    assert "JsNumber x = 5;" in code


def test_let_no_initializer():
    code = _translate("let x: number;")
    assert "JsNumber x{};" in code


def test_array_type():
    code = _translate("let items: string[] = [];")
    assert "JsArray" in code


# ── Functions ─────────────────────────────────────────────


def test_function_declaration():
    source = """
function greet(name: string): void {
    console.log(name);
}
"""
    code = _translate(source)
    assert "void greet(" in code
    assert "const JsString& name" in code
    assert 'std::println("{}", name)' in code
    assert "#include <print>" in code


def test_arrow_function():
    source = """
let fn = (x: number): number => {
    return x + 1;
};
"""
    code = _translate(source)
    assert "auto fn = " in code or "JsNumber fn = " in code
    assert "-> JsNumber" in code


def test_arrow_expression_body():
    source = """
const add = (a: number, b: number): number => a + b;
"""
    code = _translate(source)
    assert "return" in code


# ── Control Flow ──────────────────────────────────────────


def test_if_else():
    code = _translate("""
if (x > 5) {
    console.log("big");
} else {
    console.log("small");
}
""")
    assert "if (" in code
    assert "else" in code
    assert "std::println" in code


def test_while():
    code = _translate("""
while (i < 10) {
    i++;
}
""")
    assert "while (" in code
    assert "i++" in code


def test_for():
    code = _translate("""
for (let i: number = 0; i < 10; i++) {
    console.log(i);
}
""")
    assert "for (" in code
    assert "JsNumber i" in code
    assert "i++" in code


# ── Expressions ───────────────────────────────────────────


def test_binary_operators():
    code = _translate("let r: number = a + b * c;")
    assert "JsNumber r = " in code


def test_strict_equality():
    code = _translate("let eq: boolean = x === y;")
    assert "x == y" in code


def test_strict_inequality():
    code = _translate("let neq: boolean = x !== y;")
    assert "x != y" in code


def test_update_operators():
    code = _translate("i++;")
    assert "i++" in code

    code = _translate("++i;")
    assert "++i" in code


def test_assignment():
    code = _translate("x += 5;")
    assert "x += 5" in code


def test_template_literal():
    code = _translate('let msg: string = `Hello, ${name}!`;')
    assert 'std::format("Hello, {}!", name)' in code


# ── Console.log builtin ───────────────────────────────────


def test_console_log():
    code = _translate('console.log("hello world");')
    assert 'std::println("{}", "hello world")' in code


def test_console_log_variable():
    code = _translate("console.log(count);")
    assert 'std::println("{}", count)' in code


def test_console_log_template():
    code = _translate('console.log(`Hello, ${name}!`);')
    assert 'std::println("Hello, {}!", name)' in code
    assert "#include <print>" in code
    assert "std::ios_base::sync_with_stdio(false)" in code
    assert "std::cin.tie(NULL)" in code


# ── Type Resolution ───────────────────────────────────────


def test_type_mapping():
    from morph.js.codegen import TSToCppTranslator
    from morph.js.ast import TSPredefinedType, TSArrayType

    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="string")) == "JsString"
    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="number")) == "JsNumber"
    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="boolean")) == "JsBoolean"
    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="void")) == "void"
    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="any")) == "JsValue"
    assert TSToCppTranslator.resolve_type(
        TSArrayType(raw="string[]", element_type=TSPredefinedType(raw="string"))
    ) == "JsArray"
    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="int")) == "int"
    assert TSToCppTranslator.resolve_type(TSPredefinedType(raw="float")) == "float"


# ── Edge Cases ────────────────────────────────────────────


def test_empty_program():
    code = _translate("")
    # Always includes the runtime types header now
    assert "js_types.h" in code


def test_multiple_statements():
    code = _translate("""
let a: number = 1;
let b: number = 2;
let c: number = a + b;
""")
    assert "JsNumber a = 1" in code
    assert "JsNumber b = 2" in code
    assert "JsNumber c" in code


def test_return_statement():
    code = _translate("""
function add(a: number, b: number): number {
    return a + b;
}
""")
    assert "return a + b" in code
    assert "static inline" in code


# ── Ternary ────────────────────────────────────────────────


def test_ternary():
    code = _translate("const max = x > y ? x : y;")
    assert "x > y ? x : y" in code


def test_ternary_nested():
    code = _translate("const biggest = a > b ? (a > c ? a : c) : (b > c ? b : c);")
    assert "?" in code
    assert ":" in code


# ── Logical Operators ──────────────────────────────────────


def test_logical_and():
    code = _translate("const r = t && f;")
    assert "t && f" in code


def test_logical_or():
    code = _translate("const r = t || f;")
    assert "t || f" in code


# ── Do-While ───────────────────────────────────────────────


def test_do_while():
    code = _translate("""
do {
    sum += i;
    i++;
} while (i < 5);
""")
    assert "do" in code
    assert "while" in code


# ── Switch ─────────────────────────────────────────────────


def test_switch():
    code = _translate("""
switch (level) {
    case 0:
        break;
    default:
        break;
}
""")
    assert "switch" in code
    assert "case" in code
    assert "default" in code
    assert "break" in code


# ── Break / Continue ───────────────────────────────────────


def test_break():
    code = _translate("""
for (let i = 0; i < 10; i++) {
    if (i > 5) break;
}
""")
    assert "break" in code
    assert "for" in code


def test_continue():
    code = _translate("""
for (let i = 0; i < 10; i++) {
    if (i % 2 == 0) continue;
}
""")
    assert "continue" in code
    assert "for" in code


# ── Comma Operator ─────────────────────────────────────────


def test_comma():
    code = _translate("const y = (x++, x + 1);")
    assert "," in code
    assert "x++" in code


# ── Number Formats ─────────────────────────────────────────


def test_hex_number():
    code = _translate("const x = 0xFF;")
    assert "255" in code


def test_octal_number():
    code = _translate("const x = 0o77;")
    assert "63" in code


def test_binary_number():
    code = _translate("const x = 0b11;")
    assert "3" in code


def test_scientific_number():
    code = _translate("const x = 1.5e3;")
    assert "1500" in code
    assert ".0" in code or "1500" in code


# ── String Escapes ─────────────────────────────────────────


def test_string_escape_newline():
    code = _translate('console.log("hello\\nworld");')
    assert r'\n' in code


def test_string_escape_tab():
    code = _translate('console.log("tab\\there");')
    assert "tab" in code
    assert r"\t" in code


def test_string_escape_quote():
    code = _translate('console.log("quote\\"inside");')
    assert r"\"" in code or 'quote\\"' in code


def test_string_escape_backslash():
    code = _translate('console.log("backslash\\\\here");')
    assert "backslash" in code
    assert r"\\" in code


# ── Default Parameters ─────────────────────────────────────


def test_default_params():
    code = _translate("""
function f(a: number, b: number = 10) {}
""")
    assert "b = 10" in code or "b = JsNumber(10)" in code


# ── Object Shorthand ───────────────────────────────────────


def test_shorthand():
    code = _translate("const obj = {a, b, c};")
    assert "a" in code
    assert "b" in code
    assert "c" in code
    assert "JsObject" in code


# ── Nullish Coalescing ─────────────────────────────────────


def test_nullish_coalescing():
    code = _translate("const r = a ?? b;")
    assert "is_undefined" in code or "is_null" in code


# ── For Loop Variants ──────────────────────────────────────


def test_for_no_init():
    code = _translate("""
let j = 0;
for (; j < 5; j++) {}
""")
    assert "for (; j < 5; j++)" in code


def test_for_no_condition():
    code = _translate("""
for (let k = 0; ; k++) {
    if (k > 10) break;
}
""")
    assert "for (" in code
    assert "break" in code


def test_for_no_update():
    code = _translate("""
for (let m = 0; m < 5; ) {
    m++;
}
""")
    assert "for (" in code


# ── Template with Expressions ──────────────────────────────


def test_template_expression():
    code = _translate('console.log(`Hello, ${name}!`);')
    assert 'std::println("Hello, {}!", name)' in code


def test_template_with_method():
    code = _translate('console.log(`Upper: ${name.toUpperCase()}`);')
    assert "std::println" in code
    assert "toUpperCase" in code


def test_template_with_ternary():
    code = _translate('console.log(`Val is ${x > 10 ? "big" : "small"}`);')
    assert "std::println" in code


# ── Try / Catch / Throw ──────────────────────────────────


def test_try_catch():
    code = _translate("""
try {
    risky();
} catch (e) {
    console.log(e);
}
""")
    assert "try {" in code
    assert "catch (JsValue& e)" in code
    assert "risky" in code


def test_try_finally():
    code = _translate("""
try {
    risky();
} finally {
    cleanup();
}
""")
    assert "try {" in code
    assert "catch (...)" in code
    assert "throw;" in code
    assert "cleanup" in code


def test_try_catch_finally():
    code = _translate("""
try {
    risky();
} catch (e) {
    handle(e);
} finally {
    cleanup();
}
""")
    assert "try {" in code
    assert "catch (JsValue& e)" in code
    assert "catch (...)" in code
    assert "throw;" in code
    assert "cleanup" in code
    assert "handle" in code


def test_throw():
    code = _translate('throw new Error("fail");')
    assert "throw" in code
    assert "JsValue" in code or "new Error" in code


# ── var statements ────────────────────────────────────────


def test_var_statement():
    code = _translate("var x = 42;")
    assert "x = 42" in code or "int64_t" in code or "JsNumber" in code


# ── abstract keyword (should not crash) ───────────────────


def test_abstract_class():
    code = _translate("""
abstract class Base {
    abstract foo(): void;
}
""")
    # Should not crash; abstract keywords silently dropped
    assert "class Base" in code


# ── Generic type parameter constraints ────────────────────


def test_generic_constraint():
    code = _translate("""
function identity<T extends string>(x: T): T {
    return x;
}
""")
    assert "template" in code or "T" in code


# ── Async / Await ─────────────────────────────────────────


def test_await_in_async_function():
    code = _translate("""
async function fetchData(): Promise<string> {
    let r = await fetch("https://example.com");
    return r;
}
""")
    assert "co_await morph::net::fetch(\"https://example.com\")" in code
    assert "morph::Result<JsString>" in code
    assert "co_return r" in code
    assert "#include \"../../morph/runtime/net/net.h\"" in code
    assert "#include \"../../morph/runtime/reactivity/promise.h\"" in code


def test_await_fetch_no_annotation():
    code = _translate("""
async function foo() {
    let x = await fetch("/api");
}
""")
    assert "co_await morph::net::fetch(\"/api\")" in code


def test_fetch_without_await():
    code = _translate('fetch("https://example.com");')
    assert "morph::net::fetch(\"https://example.com\")" in code
    assert "#include \"../../morph/runtime/net/net.h\"" in code


def test_async_main_becomes_coroutine():
    code = _translate("""
async function main() {
    let data = await fetchData();
    console.log(data);
}
""")
    # async main is wrapped in a morph::Task lambda + event pump
    assert "morph::Task _main_task" in code
    assert "process_tasks()" in code
    assert "co_await fetchData" in code


def test_async_arrow_function():
    code = _translate("""
let f = async () => {
    await task();
};
""")
    assert "co_await task()" in code
    assert "morph::Result<JsValue>" in code or "morph::Task" in code


def test_async_method():
    code = _translate("""
class A {
    async m(): Promise<void> {
        await foo();
    }
}
""")
    assert "co_await foo()" in code
    assert "morph::Task" in code or "morph::Result" in code


def test_plain_await_expression_builds():
    # await of a plain call (non-fetch) still translates to co_await
    code = _translate("""
async function run() {
    await delay(100);
}
""")
    assert "co_await delay(100)" in code
