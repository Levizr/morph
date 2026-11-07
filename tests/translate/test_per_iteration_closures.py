"""Runtime tests for per-iteration loop bindings (Part 1D).

`let`/`const` bindings in `for`, `for...of`, and `for...in` get a fresh
binding per iteration, so closures stored during the loop observe that
iteration's value. Each case wraps the loop in a function (module-scope
closures are captureless), translates with strict types, injects native
`std::function` aliases for callable storage, compiles, runs, and asserts
the printed result.

Run:  pytest tests/translate/test_per_iteration_closures.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME_CPP = REPO_ROOT / "runtime" / "cpp"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"

# Strict mode keeps `Array<NumFn>` as `std::vector<NumFn>`; JS array storage
# for closures is a separate gap, so the fixture declares the callable
# carrier in native C++.
ALIASES = """#include <functional>
using NumFn = std::function<int64_t()>;
using StrFn = std::function<std::string()>;
using Fn = std::function<JsValue()>;
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


def _translate(source: str, tmp_path: Path, name: str) -> str:
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
    return cpp_path.read_text(encoding="utf-8")


def _with_aliases(cpp: str) -> str:
    lines = cpp.splitlines()
    last_include = max(
        (index for index, line in enumerate(lines) if line.startswith("#include")),
        default=-1,
    )
    lines[last_include + 1 : last_include + 1] = ALIASES.splitlines()
    return "\n".join(lines) + "\n"


def _run_case(source: str, tmp_path: Path, name: str) -> str:
    cpp = _with_aliases(_translate(source, tmp_path, name))
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
    "classic-for": (
        """
function total(): number {
    const fns: Array<NumFn> = [() => 0, () => 0, () => 0];
    for (let i = 0; i < 3; i++) {
        fns[i] = () => i;
    }
    return fns[0]() + fns[1]() + fns[2]();
}
console.log(total());
""",
        "3",
    ),
    "for-of": (
        """
function total(): number {
    const fns: Array<NumFn> = [() => 0, () => 0, () => 0];
    const values: Array<int> = [10, 20, 30];
    let index = 0;
    for (const value of values) {
        fns[index] = () => value;
        index = index + 1;
    }
    return fns[0]() + fns[1]() + fns[2]();
}
console.log(total());
""",
        "60",
    ),
    "for-in": (
        """
function keys(): string {
    const fns: Array<StrFn> = [() => "", () => ""];
    const obj = { a: 1, b: 2 };
    let index = 0;
    for (const key in obj) {
        fns[index] = () => key;
        index = index + 1;
    }
    return fns[0]() + fns[1]();
}
console.log(keys());
""",
        "ab",
    ),
    "for-of-pattern": (
        """
function total(): number {
    const fns: Array<Fn> = [() => 0, () => 0];
    let index = 0;
    for (const { value } of [{ value: 10 }, { value: 20 }]) {
        fns[index] = () => value;
        index = index + 1;
    }
    return fns[0]() + fns[1]();
}
console.log(total());
""",
        "30",
    ),
    "continue": (
        """
function total(): number {
    const fns: Array<NumFn> = [() => 0, () => 0];
    let index = 0;
    for (let i = 0; i < 5; i++) {
        if (i % 2 === 0) {
            continue;
        }
        fns[index] = () => i;
        index = index + 1;
    }
    return fns[0]() + fns[1]();
}
console.log(total());
""",
        "4",
    ),
    "combined": (
        """
function closureTotal(): number {
    const numFns: Array<NumFn> = [() => 0, () => 0, () => 0];
    for (let i = 0; i < 3; i++) {
        numFns[i] = () => i;
    }
    let total = numFns[0]() + numFns[1]() + numFns[2]();

    const values: Array<int> = [10, 20, 30];
    const ofFns: Array<NumFn> = [() => 0, () => 0, () => 0];
    let index = 0;
    for (const value of values) {
        ofFns[index] = () => value;
        index = index + 1;
    }
    total = total + ofFns[0]() + ofFns[1]() + ofFns[2]();

    const keyFns: Array<StrFn> = [() => "", () => ""];
    const obj = { a: 1, b: 2 };
    index = 0;
    for (const key in obj) {
        keyFns[index] = () => key;
        index = index + 1;
    }
    total = total + (keyFns[0]() === "a" ? 1 : 0) + (keyFns[1]() === "b" ? 1 : 0);
    return total;
}
console.log(closureTotal());
""",
        "65",
    ),
}


@pytest.mark.parametrize("name", sorted(CASES))
def test_per_iteration_binding(name: str, tmp_path: Path):
    source, expected = CASES[name]
    assert _run_case(source, tmp_path, name) == expected
