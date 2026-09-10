"""
Structural tests proving intent-based codegen decisions in generated C++.

test_rust_translate.py checks runtime behavior against Node.js. These tests
check the EMITTED SHAPE instead: native types where proven, escape-based
allocation where required, comparison helpers only where C++ differs from
JavaScript. No compilation needed — translation output is asserted directly.
"""
from __future__ import annotations

import re
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
    """Translate inline TypeScript and return the generated C++ text."""
    ts_path = tmp_path / f"{name}.ts"
    ts_path.write_text(source, encoding="utf-8")
    morph_bin = _find_morph_bin()
    result = subprocess.run(
        [str(morph_bin), str(ts_path), "--to", "cpp"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, f"morph failed: {result.stderr}"
    cpp_path = tmp_path / f"{name}.cpp"
    assert cpp_path.exists(), f"no output generated for {name}"
    return cpp_path.read_text(encoding="utf-8")


def code_without_strings(content: str) -> str:
    """Strip string literals so assertions can't match inside them."""
    return re.sub(r'"(?:[^"\\]|\\.)*"', '""', content)


def test_string_method_stays_native(tmp_path: Path):
    cpp = translate_source(
        'let greeting = "Hello";\nconsole.log(greeting.toUpperCase());\n',
        tmp_path,
        "native_string",
    )
    code = code_without_strings(cpp)
    assert "std::string greeting" in code
    assert "JsString greeting" not in code
    assert "morph::str::to_upper" in code


def test_closure_capture_uses_shared_ptr(tmp_path: Path):
    cpp = translate_source(
        "function makeCounter(): any {\n"
        "    let count: number = 0;\n"
        "    const bump = (): number => {\n"
        "        count = count + 1;\n"
        "        return count;\n"
        "    };\n"
        "    return bump();\n"
        "}\n"
        "console.log(makeCounter());\n",
        tmp_path,
        "closure",
    )
    code = code_without_strings(cpp)
    assert "std::shared_ptr<int64_t> count" in code
    assert "(*count)" in code


def test_returned_scalar_stays_on_stack(tmp_path: Path):
    cpp = translate_source(
        "function doubleIt(n: number): number {\n"
        "    let doubled: number = n * 2;\n"
        "    return doubled;\n"
        "}\n"
        "console.log(doubleIt(21));\n",
        tmp_path,
        "returned",
    )
    code = code_without_strings(cpp)
    assert "unique_ptr" not in code


def test_proven_small_int_uses_int32(tmp_path: Path):
    cpp = translate_source("let small: number = 42;\nconsole.log(small);\n", tmp_path, "small")
    assert "int32_t small" in code_without_strings(cpp)


def test_big_literal_uses_int64(tmp_path: Path):
    cpp = translate_source(
        "let big: number = 3000000000;\nconsole.log(big);\n", tmp_path, "big"
    )
    code = code_without_strings(cpp)
    assert "int64_t big" in code
    assert "int32_t big" not in code


def test_computed_reassignment_forfeits_int32(tmp_path: Path):
    cpp = translate_source(
        "let total: number = 0;\ntotal = total + 5;\nconsole.log(total);\n",
        tmp_path,
        "reassigned",
    )
    code = code_without_strings(cpp)
    assert "int64_t total" in code
    assert "int32_t total" not in code


def test_literal_reassignment_narrows_to_native(tmp_path: Path):
    cpp = translate_source(
        "async function fetchCount(): Promise<number> {\n"
        "    return 7;\n"
        "}\n"
        "async function main(): Promise<void> {\n"
        "    let tally = await fetchCount();\n"
        "    console.log(tally);\n"
        "    tally = 42;\n"
        "    console.log(tally);\n"
        "}\n"
        "main();\n",
        tmp_path,
        "narrowed",
    )
    code = code_without_strings(cpp)
    assert "int32_t tally_narrowed_1 = 42" in code


def test_branched_reassignment_keeps_wide_type(tmp_path: Path):
    cpp = translate_source(
        "async function fetchCount(): Promise<number> {\n"
        "    return 7;\n"
        "}\n"
        "async function main(): Promise<void> {\n"
        "    let tally = await fetchCount();\n"
        "    if (tally > 0) {\n"
        "        tally = 42;\n"
        "    }\n"
        "    console.log(tally);\n"
        "}\n"
        "main();\n",
        tmp_path,
        "branched",
    )
    code = code_without_strings(cpp)
    assert "tally_narrowed" not in code


def test_trusted_annotation_beats_dynamic_widening(tmp_path: Path):
    cpp = translate_source(
        "async function fetchLimit(): Promise<number> {\n"
        "    return 100;\n"
        "}\n"
        "async function main(): Promise<void> {\n"
        "    let userLimit: int = await fetchLimit();\n"
        "    console.log(userLimit);\n"
        "}\n"
        "main();\n",
        tmp_path,
        "trusted",
    )
    code = code_without_strings(cpp)
    assert re.search(r"\bint userLimit\b", code) is not None
    assert "JsNumber userLimit" not in code


def test_unannotated_await_deduces_type(tmp_path: Path):
    cpp = translate_source(
        "async function fetchLimit(): Promise<number> {\n"
        "    return 100;\n"
        "}\n"
        "async function main(): Promise<void> {\n"
        "    let plain = await fetchLimit();\n"
        "    console.log(plain);\n"
        "}\n"
        "main();\n",
        tmp_path,
        "unannotated",
    )
    code = code_without_strings(cpp)
    assert "auto plain = co_await fetchLimit();" in code
    assert "JsNumber plain" not in code


def test_mixed_comparison_generates_helper(tmp_path: Path):
    cpp = translate_source(
        'let label: string = "";\nlet count: number = 0;\nconsole.log(label == count);\n',
        tmp_path,
        "mixed_eq",
    )
    assert "namespace morph::js_cmp" in cpp
    assert "morph::js_cmp::loose_eq(label, count)" in code_without_strings(cpp)


def test_same_type_comparison_stays_direct(tmp_path: Path):
    cpp = translate_source(
        "let first: number = 1;\nlet second: number = 2;\nconsole.log(first == second);\n",
        tmp_path,
        "same_eq",
    )
    assert "namespace morph::js_cmp" not in cpp
    assert "morph::js_cmp" not in code_without_strings(cpp)


def test_optimize_flag_is_gone(tmp_path: Path):
    ts_path = tmp_path / "flag.ts"
    ts_path.write_text("let x = 1;\n", encoding="utf-8")
    result = subprocess.run(
        [str(_find_morph_bin()), str(ts_path), "--to", "cpp", "--optimize"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode != 0
