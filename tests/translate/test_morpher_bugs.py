"""
Regression tests for morpher (TS -> C++) mishandlings found by probing.

Each test translates a small, legal TypeScript snippet the user can write
and asserts the generated C++ is valid / semantics-preserving. Tests that
currently FAIL document a live bug in crates/morpher/src/codegen/cpp.rs
(see bug id + location in the docstring of each test).

Run:  pytest tests/translate/test_morpher_bugs.py -x -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


def translate_source(source: str, tmp_path: Path, name: str = "case") -> str:
    ts_path = tmp_path / f"{name}.ts"
    ts_path.write_text(source, encoding="utf-8")
    result = subprocess.run(
        [str(_find_morph_bin()), str(ts_path), "--to", "cpp"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, f"morph failed: {result.stderr}"
    cpp_path = tmp_path / f"{name}.cpp"
    assert cpp_path.exists(), f"no output generated for {name}"
    return cpp_path.read_text(encoding="utf-8")


def compiles(cpp: str, tmp_path: Path, name: str) -> tuple[bool, str]:
    """Syntax-check generated C++ (needs repo runtime headers on include path)."""
    src = tmp_path / f"{name}.check.cpp"
    src.write_text(cpp, encoding="utf-8")
    r = subprocess.run(
        ["g++", "-std=c++23", "-fsyntax-only", str(src)],
        capture_output=True,
        text=True,
        timeout=60,
    )
    return r.returncode == 0, r.stderr


# ── Function creation ────────────────────────────────────────────────

FN_EXPR = "const add = function(a: number, b: number): number { return a + b; };\n"


def test_function_expression_keeps_params(tmp_path: Path):
    """BUG-1 cpp.rs:4659 emit_function_expression discards `params`
    (`let _ = params;`, hardcodes `(JsValue _jsThis)`), so `a`/`b` are
    undeclared in the body."""
    cpp = translate_source(FN_EXPR, tmp_path, "fn_expr")
    assert "a" in cpp and "b" in cpp
    assert "return a + b;" in cpp, f"params dropped:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "fn_expr")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_nested_function_declaration_compiles(tmp_path: Path):
    """BUG-2 nested `function` decls emit `static inline` inside a function
    body, which is illegal C++ (function-definition not allowed here)."""
    src = "function outer(): void {\n function inner(): number { return 1; }\n}\n"
    cpp = translate_source(src, tmp_path, "fn_nested")
    ok, err = compiles(cpp, tmp_path, "fn_nested")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_default_param_preserved(tmp_path: Path):
    """BUG-3 `function greet(name: string = "hi")` drops the `= "hi"`
    default; calling `greet()` then fails to compile / changes semantics."""
    cpp = translate_source(
        'function greet(name: string = "hi"): string { return name; }\n',
        tmp_path,
        "fn_default",
    )
    assert '"hi"' in cpp, f"default value dropped:\n{cpp}"


def test_rest_param_not_collapsed(tmp_path: Path):
    """BUG-4 `...nums: number[]` collapses to a single `auto nums` param,
    so `sum(1, 2, 3)` at the call site cannot compile."""
    cpp = translate_source(
        "function sum(...nums: number[]): number { return nums[0]; }\n",
        tmp_path,
        "fn_rest",
    )
    assert "nums" in cpp
    # Rest param should lower to JsArray (JS semantics) - not silently collapsed to single value
    assert "JsArray" in cpp, f"rest param not lowered to JsArray:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "fn_rest")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_destructured_object_param_kept(tmp_path: Path):
    """BUG-5 `function f({a, b}: any)` drops all params -> `f()` while the
    body still reads `a`; undeclared-identifier compile error."""
    cpp = translate_source(
        "function f({a, b}: any): number { return a; }\n", tmp_path, "fn_destr"
    )
    ok, err = compiles(cpp, tmp_path, "fn_destr")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_generator_rejected_or_lowered(tmp_path: Path):
    """BUG-6 `function* gen()` emits `Generator<T>` + bare `co_yield` in a
    non-coroutine: `Generator does not name a type`. Must error or lower."""
    src = "function* gen(): Generator<number> { yield 1; }\n"
    cpp = translate_source(src, tmp_path, "fn_gen")
    # Should emit a clear error comment, not invalid C++
    assert "generators not supported" in cpp, f"should reject generators:\n{cpp}"


def test_mutual_recursion_forward_declared(tmp_path: Path):
    """BUG-7 `a()` calls `b()` defined later: `b was not declared`.
    Needs a forward declaration since C++ has no hoisting."""
    src = (
        "function a(): number { return b(); }\n"
        "function b(): number { return a(); }\n"
    )
    cpp = translate_source(src, tmp_path, "fn_mutual")
    ok, err = compiles(cpp, tmp_path, "fn_mutual")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


# ── Small things: operators / keywords ───────────────────────────────

def test_delete_has_space(tmp_path: Path):
    """BUG-8 cpp.rs:2694 `format!("{}{}", op, arg)` pastes word-operators:
    `delete o.prop` -> `deleteo.prop`."""
    cpp = translate_source(
        "function f(o: any): void { delete o.prop; }\n", tmp_path, "op_delete"
    )
    assert "deleteo" not in cpp, f"token paste:\n{cpp}"
    assert "delete" in cpp


def test_typeof_has_space(tmp_path: Path):
    """BUG-9 same paste as BUG-8: `typeof x` -> `typeofx` (undeclared)."""
    cpp = translate_source(
        "function f(x: any): string { return typeof x; }\n", tmp_path, "op_typeof"
    )
    assert "typeofx" not in cpp, f"token paste:\n{cpp}"


def test_instanceof_lowered(tmp_path: Path):
    """BUG-10 `x instanceof Foo` emitted verbatim; `instanceof` is not C++."""
    # Just verify the operator is lowered to helper call (don't require full compile
    # since constructor handling is a separate issue)
    cpp = translate_source(
        "function f(x: any, ctor: any): boolean { return x instanceof ctor; }\n",
        tmp_path,
        "op_instanceof",
    )
    # Should use the helper, not raw `instanceof`
    assert "morph::js_instanceof" in cpp, f"js_instanceof helper not used:\n{cpp}"
    # Should NOT have raw `instanceof` operator in the body
    assert " instanceof " not in cpp, f"raw instanceof operator leaked:\n{cpp}"


def test_in_operator_lowered(tmp_path: Path):
    """BUG-11 `"a" in o` emitted verbatim; `in` is not a C++ operator."""
    cpp = translate_source(
        'function f(o: any): boolean { return "a" in o; }\n', tmp_path, "op_in"
    )
    # Should use the helper
    assert "morph::js_has_property" in cpp, f"js_has_property helper not used:\n{cpp}"
    # Should NOT have raw ` in ` operator in the body
    body_start = cpp.find("JsBoolean f(auto o)")
    if body_start >= 0:
        body = cpp[body_start:]
        assert " in " not in body, f"raw 'in' operator leaked:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "op_in")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_private_field_no_hash(tmp_path: Path):
    """BUG-12 cpp.rs:2738 emits `this#secret`; `#` is not valid C++."""
    cpp = translate_source(
        "class A { #secret: number = 1; get(): number { return this.#secret; } }\n",
        tmp_path,
        "op_private",
    )
    assert "#" not in cpp.replace("#include", ""), f"hash leaked:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "op_private")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_bigint_literal_valid_cpp(tmp_path: Path):
    """BUG-13 BigInt emits `123n`, which is not a C++ literal."""
    cpp = translate_source("const x = 123n;\n", tmp_path, "op_bigint")
    assert "123n" not in cpp, f"JS literal leaked:\n{cpp}"


def test_no_span_debug_leak(tmp_path: Path):
    """BUG-14 unhandled exprs emit `/* unhandled expr Span {..} */` and even
    `return /* ... */;` (return with no value in a JsValue function)."""
    cpp = translate_source(
        "function f(): any { return new.target; }\n", tmp_path, "op_span"
    )
    assert "Span" not in cpp, f"internal debug type leaked:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "op_span")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


# ── Small things: types / includes ───────────────────────────────────

def test_int32_needs_cstdint(tmp_path: Path):
    """BUG-15 `int32_t`/`int64_t` emitted without `#include <cstdint>`."""
    cpp = translate_source("let x: number | string = 1;\n", tmp_path, "ty_union")
    if "int32_t" in cpp or "int64_t" in cpp:
        assert "cstdint" in cpp, f"missing <cstdint>:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "ty_union")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_jsarray_keeps_include(tmp_path: Path):
    """BUG-16 `const x = [1,2] as const` emits `JsArray{...}` with no
    js_types.h include -> `JsArray was not declared`."""
    cpp = translate_source("const x = [1,2] as const;\n", tmp_path, "ty_asconst")
    if "JsArray" in cpp:
        assert "js_types" in cpp, f"missing runtime include:\n{cpp}"
    ok, err = compiles(cpp, tmp_path, "ty_asconst")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_spread_array_keeps_elements(tmp_path: Path):
    """BUG-17 `[...a, 3]` drops the spread input (`{3}` only) and wraps in
    `unique_ptr<vector>` which cannot convert to the `JsValue` return."""
    cpp = translate_source(
        "function f(): any { const a: number[] = [1,2]; const c = [...a, 3]; return c; }\n",
        tmp_path,
        "ty_spread",
    )
    ok, err = compiles(cpp, tmp_path, "ty_spread")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


# ── Large things ─────────────────────────────────────────────────────

def test_closure_returning_lambda_compiles(tmp_path: Path):
    """BUG-18 `function outer(): any { ...; return () => x+1; }` returns a
    raw lambda where `JsValue` is declared: no conversion exists."""
    cpp = translate_source(
        "function outer(): any { let x: number = 1; return () => x + 1; }\n",
        tmp_path,
        "lg_closure",
    )
    ok, err = compiles(cpp, tmp_path, "lg_closure")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_arrow_this_captured(tmp_path: Path):
    """BUG-19 arrow using `this` emits `[]` capture: `this was not captured`."""
    cpp = translate_source(
        "class A { v: number = 1; m(): any { return () => this.v; } }\n",
        tmp_path,
        "lg_this",
    )
    ok, err = compiles(cpp, tmp_path, "lg_this")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"


def test_namespace_not_silently_dropped(tmp_path: Path):
    """BUG-20 `namespace N { export const x = 1; }` produces EMPTY output
    with no diagnostic — user code silently vanishes."""
    cpp = translate_source(
        "namespace N { export const x = 1; }\n", tmp_path, "lg_ns"
    )
    assert cpp.strip(), "namespace body silently dropped (empty output)"
    assert "x" in cpp, f"namespace member lost:\n{cpp}"


def test_require_diagnosed_not_emitted(tmp_path: Path):
    """BUG-21 `require("fs")` is emitted verbatim; `require` does not exist
    in C++. Must be an error/lint, not broken code."""
    cpp = translate_source('const fs = require("fs");\n', tmp_path, "lg_require")
    assert "require(" not in cpp, f"untranslatable call emitted verbatim:\n{cpp}"


def test_class_extends_unknown_base(tmp_path: Path):
    """BUG-22 `class A extends B` with unknown `B` emits `: public B {}`
    referencing an undeclared base. Must error or forward-declare."""
    cpp = translate_source(
        "class A extends B { constructor() { super(); } }\n", tmp_path, "lg_extends"
    )
    ok, err = compiles(cpp, tmp_path, "lg_extends")
    assert ok, f"generated C++ does not compile:\n{err}\n{cpp}"
