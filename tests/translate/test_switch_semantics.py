"""Runtime tests for JavaScript `switch` semantics (Part 2A).

JavaScript switches compare with `===`, accept arbitrary case expressions,
and fall through until `break`. The translator dispatches to an integer tag
and keeps a native C++ switch for the body, so `break`/`continue` and
fallthrough must all match Node.

Run:  pytest tests/translate/test_switch_semantics.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME_CPP = REPO_ROOT / "runtime" / "cpp"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


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


def _run_case(source: str, tmp_path: Path, name: str) -> str:
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
    cpp = cpp_path.read_text(encoding="utf-8")

    exe = tmp_path / name
    build = subprocess.run(
        [
            _find_compiler(),
            "-std=c++23",
            f"-I{RUNTIME_CPP}",
            str(cpp_path),
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
    "string-fallthrough": (
        """
function label(status: string): number {
    let out = 0;
    switch (status) {
        case "ready":
            out = out + 1;
        case "done":
            out = out + 10;
            break;
        default:
            out = out + 100;
    }
    return out;
}
console.log(label("ready"));
console.log(label("done"));
console.log(label("other"));
""",
        "11\n10\n100",
    ),
    "default-in-middle": (
        """
function pick(value: string): number {
    let out = 0;
    switch (value) {
        case "a":
            out = out + 1;
            break;
        default:
            out = out + 10;
        case "b":
            out = out + 100;
    }
    return out;
}
console.log(pick("a"));
console.log(pick("b"));
console.log(pick("z"));
""",
        "1\n100\n110",
    ),
    "default-first": (
        """
function pick(value: string): number {
    let out = 0;
    switch (value) {
        default:
            out = out + 10;
        case "a":
            out = out + 1;
            break;
        case "b":
            out = out + 100;
    }
    return out;
}
console.log(pick("a"));
console.log(pick("b"));
console.log(pick("z"));
""",
        "1\n100\n11",
    ),
    "numeric-mixed-case-types": (
        """
function classify(value: number): string {
    switch (value) {
        case 1:
            return "one";
        case 2.5:
            return "two-and-half";
        case "1":
            return "string one";
        default:
            return "other";
    }
}
console.log(classify(1) + "," + classify(2.5) + "," + classify(3));
""",
        "one,two-and-half,other",
    ),
    "string-discriminant-vs-number-case": (
        """
function classify(value: string): string {
    switch (value) {
        case "1":
            return "string one";
        case 1:
            return "number one";
        default:
            return "other";
    }
}
console.log(classify("1") + "," + classify("x"));
""",
        "string one,other",
    ),
    "break-and-continue": (
        """
function total(): number {
    let sum = 0;
    for (let i = 0; i < 5; i++) {
        switch (i) {
            case 1:
                continue;
            case 3:
                break;
            default:
                sum = sum + 10;
        }
        sum = sum + i;
    }
    return sum;
}
console.log(total());
""",
        "39",
    ),
}


@pytest.mark.parametrize("name", sorted(CASES))
def test_switch_semantics(name: str, tmp_path: Path):
    source, expected = CASES[name]
    assert _run_case(source, tmp_path, name) == expected
