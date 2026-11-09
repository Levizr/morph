"""Runtime tests for array `for...in` (Part 2C).

JavaScript `for...in` over an array enumerates own index keys as strings in
ascending order, while object `for...in` keeps enumerating object keys. The
translator lowers array receivers to an index loop and converts each index
with `std::to_string`, so keys must compare strictly as strings.

Run:  pytest tests/translate/test_array_for_in.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME_CPP = REPO_ROOT / "runtime" / "cpp"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"

# Strict mode keeps `Array<StrFn>` as `std::vector<StrFn>`; JS array storage
# for closures is a separate gap, so the captured-key case declares the
# callable carrier in native C++.
ALIASES = """#include <functional>
using StrFn = std::function<std::string()>;
"""


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


def _find_compiler() -> str:
    compiler = shutil.which("g++-14") or shutil.which("g++")
    if not compiler:
        pytest.skip("g++ not found")
    return compiler


def _with_aliases(cpp: str) -> str:
    lines = cpp.splitlines()
    last_include = max(
        (index for index, line in enumerate(lines) if line.startswith("#include")),
        default=-1,
    )
    lines[last_include + 1 : last_include + 1] = ALIASES.splitlines()
    return "\n".join(lines) + "\n"


def _run_case(source: str, tmp_path: Path, name: str) -> str:
    ts_path = tmp_path / f"{name}.ts"
    ts_path.write_text(source, encoding="utf-8")
    result = subprocess.run(
        [str(_find_morph_bin()), str(ts_path), "--to", "cpp", "--types", "strict"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, f"morph failed: {result.stderr}"
    cpp_path = tmp_path / f"{name}.cpp"
    assert cpp_path.exists(), f"no output generated for {name}"
    cpp = _with_aliases(cpp_path.read_text(encoding="utf-8"))

    src = tmp_path / f"{name}.run.cpp"
    exe = tmp_path / name
    src.write_text(cpp, encoding="utf-8")
    build = subprocess.run(
        [
            _find_compiler(),
            "-std=c++23",
            f"-I{RUNTIME_CPP}",
            str(src),
            "-o",
            str(exe),
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert build.returncode == 0, f"generated C++ does not compile:\n{build.stderr}\n--- cpp ---\n{cpp}"
    run = subprocess.run([str(exe)], capture_output=True, text=True, timeout=30)
    assert run.returncode == 0, f"program failed: {run.stderr}\n{run.stdout}"
    return run.stdout.strip()


CASES = {
    "array-literal-keys": (
        """
function keys(): string {
    let out = "";
    for (const index in [10, 20, 30]) {
        out = out + index;
    }
    return out;
}
console.log(keys());
""",
        "012",
    ),
    "typed-array-keys": (
        """
function keys(): string {
    const values: Array<int> = [10, 20];
    let out = "";
    for (const index in values) {
        out = out + index;
    }
    return out;
}
console.log(keys());
""",
        "01",
    ),
    "keys-are-strict-strings": (
        """
function score(): number {
    let total = 0;
    for (const index in [10, 20]) {
        if (index === "0") total = total + 1;
        if (index === "1") total = total + 10;
        if (index === 0) total = total + 100;
    }
    return total;
}
console.log(score());
""",
        "11",
    ),
    "assignment-target": (
        """
function lastKey(): string {
    let key = "";
    for (key in [7, 8]) {
        if (key === "0") key = key;
    }
    return key;
}
console.log(lastKey());
""",
        "1",
    ),
    "captured-key-per-iteration": (
        """
function keys(): string {
    const fns: Array<StrFn> = [() => "", () => ""];
    const values: Array<int> = [10, 20];
    let index = 0;
    for (const key in values) {
        fns[index] = () => key;
        index = index + 1;
    }
    return fns[0]() + fns[1]();
}
console.log(keys());
""",
        "01",
    ),
    "continue-skips-an-iteration": (
        """
function count(): number {
    let total = 0;
    for (const index in [10, 20, 30]) {
        if (index === "1") continue;
        total = total + 1;
    }
    return total;
}
console.log(count());
""",
        "2",
    ),
    "empty-array-has-no-keys": (
        """
function count(): number {
    let total = 0;
    for (const index in []) {
        total = total + 1;
    }
    return total;
}
console.log(count());
""",
        "0",
    ),
    "object-for-in-unchanged": (
        """
function keys(): string {
    const obj = { a: 1, b: 2 };
    let out = "";
    for (const key in obj) {
        out = out + key;
    }
    return out;
}
console.log(keys());
""",
        "ab",
    ),
    "string-literal-keys": (
        """
function keys(): string {
    let out = "";
    for (const index in "ab") {
        out = out + index;
    }
    return out;
}
console.log(keys());
""",
        "01",
    ),
    "typed-string-keys": (
        """
function keys(): string {
    const label: string = "abc";
    let out = "";
    for (const index in label) {
        out = out + index;
    }
    return out;
}
console.log(keys());
""",
        "012",
    ),
}


@pytest.mark.parametrize("name", sorted(CASES))
def test_array_for_in(name: str, tmp_path: Path):
    source, expected = CASES[name]
    assert _run_case(source, tmp_path, name) == expected
