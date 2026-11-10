"""Runtime and check tests for hoisting and TDZ (Part 2D).

`var` bindings hoist to their function/program scope and start as
`undefined`; declaration sites assign. `let`/`const` reads before the
declaration line are rejected at check time instead of printing the later
value (JS throws a TDZ ReferenceError).

Run:  pytest tests/translate/test_hoisting_tdz.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

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
        [str(_find_morph_bin()), str(ts_path), "--to", "cpp", "--types", "strict"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, f"morph failed: {result.stderr}"
    cpp_path = tmp_path / f"{name}.cpp"
    assert cpp_path.exists(), f"no output generated for {name}"
    cpp = cpp_path.read_text(encoding="utf-8")

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


def _check(source: str, tmp_path: Path, name: str) -> tuple[int, str]:
    ts_path = tmp_path / f"{name}.ts"
    ts_path.write_text(source, encoding="utf-8")
    result = subprocess.run(
        [str(_find_morph_bin()), "check", str(ts_path)],
        capture_output=True,
        text=True,
        timeout=30,
    )
    return result.returncode, result.stdout + result.stderr


CASES = {
    "var-before-declaration": (
        """
function f() {
  console.log(x)
  var x = 1
  console.log(x)
}
f()
""",
        "undefined\n1",
    ),
    "var-block-scope": (
        """
function f() {
  if (true) {
    var b = 3
  }
  console.log(b)
}
f()
""",
        "3",
    ),
    "var-loop-binding-survives": (
        """
function f() {
  for (var i = 0; i < 3; i++) {
    console.log(i)
  }
  console.log(i)
}
f()
""",
        "0\n1\n2\n3",
    ),
    "var-for-in-binding-survives": (
        """
function f() {
  const obj = { a: 1, b: 2 }
  var k
  for (k in obj) {
    console.log(k)
  }
  console.log(k)
}
f()
""",
        "a\nb\nb",
    ),
    "var-for-of-binding-survives": (
        """
function f() {
  for (var e of [10, 20]) {
    console.log(e)
  }
  console.log(e)
}
f()
""",
        "10\n20\n20",
    ),
    "var-late-array": (
        """
function f() {
  console.log(x)
  var x = [1, 2]
  console.log(x.length)
}
f()
""",
        "undefined\n2",
    ),
    "var-redeclaration": (
        """
function f() {
  var x = 1
  var x = 2
  console.log(x)
}
f()
""",
        "2",
    ),
    "var-param-same-name": (
        """
function f(x: number) {
  var x = 5
  console.log(x)
}
f(1)
""",
        "5",
    ),
    "var-closure-capture": (
        """
function f() {
  var v = 7
  const read = () => v
  console.log(read())
}
f()
""",
        "7",
    ),
    "arrow-body-var": (
        """
const f = () => {
  console.log(x)
  var x = 1
}
f()
""",
        "undefined",
    ),
    "var-function-initializer": (
        """
function f() {
  var g = () => 1
  console.log(g())
}
f()
""",
        "1",
    ),
    "var-call-returning-function": (
        """
function make() { return () => 1 }
function f() {
  var g = make()
  console.log(g())
}
f()
""",
        "1",
    ),
    "var-class-expression": (
        """
function f() {
  var C = class { x = 1 }
  const c = new C()
  console.log(c.x)
}
f()
""",
        "1",
    ),
    "var-nested-closure-native": (
        """
function f() {
  var g = () => 1
  const read = () => g()
  console.log(read())
}
f()
""",
        "1",
    ),
    "var-nested-closure-class": (
        """
class C { x: number = 3 }
function f() {
  var c = new C()
  const read = () => c.x
  console.log(read())
}
f()
""",
        "3",
    ),
    "module-var-before-declaration": (
        """
console.log(top)
var top = 42
console.log(top)
""",
        "undefined\n42",
    ),
}


def test_hoisting_runtime(tmp_path: Path) -> None:
    for name, (source, expected) in CASES.items():
        output = _run_case(source, tmp_path, name)
        assert output == expected, f"{name}: expected {expected!r}, got {output!r}"


def test_tdz_let_read_before_declaration(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  console.log(x)
  let x = 1
}
f()
""",
        tmp_path,
        "tdz_let_read",
    )
    assert rc != 0
    assert "temporal dead zone" in out


def test_tdz_const_read_before_declaration(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  console.log(x)
  const x = 1
}
f()
""",
        tmp_path,
        "tdz_const_read",
    )
    assert rc != 0
    assert "temporal dead zone" in out


def test_tdz_self_initializer(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  let x = x
  console.log(x)
}
f()
""",
        tmp_path,
        "tdz_self_init",
    )
    assert rc != 0
    assert "temporal dead zone" in out


def test_tdz_skips_nested_function(tmp_path: Path) -> None:
    # The closure runs after the declaration line, so this is legal JS and
    # must stay quiet even though the reference text precedes the `let`.
    rc, out = _check(
        """
function f() {
  const read = () => x
  let x = 1
  console.log(read())
}
f()
""",
        tmp_path,
        "tdz_nested_ok",
    )
    assert rc == 0, out


def test_late_var_arrow_initializer_rejected(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  function helper() { console.log(g) }
  var g = () => 1
  helper()
}
f()
""",
        tmp_path,
        "late_var_arrow",
    )
    assert rc != 0
    assert "cannot start as `undefined`" in out


def test_destructured_var_rejected(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  var { a } = { a: 1 }
  console.log(a)
}
f()
""",
        tmp_path,
        "var_destructure",
    )
    assert rc != 0
    assert "destructured `var` declarations are not supported" in out


def test_var_redeclaring_function_rejected(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  function helper() { return 1 }
  var helper = 2
  console.log(helper)
}
f()
""",
        tmp_path,
        "var_collides_function",
    )
    assert rc != 0
    assert "redeclares a function or class" in out


def test_var_redeclaring_class_rejected(tmp_path: Path) -> None:
    rc, out = _check(
        """
function f() {
  class Thing {}
  var Thing = 2
  console.log(Thing)
}
f()
""",
        tmp_path,
        "var_collides_class",
    )
    assert rc != 0
    assert "redeclares a function or class" in out
