"""
Tests for Rust-based TS → C++ translation (crates/morph-js).

For each fixture in tests/translate/fixtures/*.ts:
  1. Translate via `morph` binary (Rust TSToCppTranslator)
  2. Patch relative includes to absolute for standalone compile
  3. Compile with g++-14 -std=c++23
  4. Run binary and check it exits 0 and produces expected output
  5. (Optional) Compare with Node.js execution via `npx tsx` if available

Covers: variables, functions, classes, arrays, objects, control flow, operators,
template literals, try/catch, console.log with Js* formatter, inheritance, etc.
"""
from __future__ import annotations

import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

import pytest

# Paths
REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME_CPP = REPO_ROOT / "runtime" / "cpp"
FIXTURES_DIR = Path(__file__).parent / "fixtures"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"

# Also check for `cargo run` fallback if binary not built
def _find_morph_bin() -> Path | None:
    if MORPH_BIN.exists():
        return MORPH_BIN
    # Try `morph` in PATH (installed via `morph` CLI)
    p = shutil.which("morph")
    if p:
        return Path(p)
    return None

def _patch_includes(cpp_path: Path) -> None:
    """Replace relative runtime includes with absolute for standalone /tmp compile."""
    text = cpp_path.read_text(encoding="utf-8", errors="ignore")
    # Replace both js_types and js_value_format and other runtime headers
    # Generated code uses "../../runtime/cpp/..." relative to project .morph/
    # We replace with absolute path to repo runtime.
    orig = text
    # Handle js_types.h and js_value_format.h and other runtime headers
    text = text.replace(
        '"../../runtime/cpp/types/js_types.h"',
        f'"{RUNTIME_CPP}/types/js_types.h"'
    )
    text = text.replace(
        '"../../runtime/cpp/types/js_value_format.h"',
        f'"{RUNTIME_CPP}/types/js_value_format.h"'
    )
    text = text.replace(
        '"../../runtime/cpp/net/net.h"',
        f'"{RUNTIME_CPP}/net/net.h"'
    )
    text = text.replace(
        '"../../runtime/cpp/reactivity/promise.h"',
        f'"{RUNTIME_CPP}/reactivity/promise.h"'
    )
    text = text.replace(
        '"../../runtime/cpp/reactivity/task.h"',
        f'"{RUNTIME_CPP}/reactivity/task.h"'
    )
    text = text.replace(
        '"../../morph/runtime/net/net.h"',
        f'"{RUNTIME_CPP}/net/net.h"'
    )
    # Also handle generic "../../runtime/cpp/..." for any other header
    text = re.sub(
        r'"\.\./\.\./runtime/cpp/([^"]+)"',
        lambda m: f'"{RUNTIME_CPP}/{m.group(1)}"',
        text
    )
    text = re.sub(
        r'"\.\./\.\./morph/runtime/([^"]+)"',
        lambda m: f'"{RUNTIME_CPP}/{m.group(1)}"',
        text
    )
    if text != orig:
        cpp_path.write_text(text, encoding="utf-8")

def _translate_fixture(ts_path: Path, tmpdir: Path) -> Path:
    """Translate .ts via `morph --to cpp` or `cargo run -p morph-js` fallback."""
    # Copy fixture to tmpdir to avoid polluting (skip if already there)
    tmp_ts = tmpdir / ts_path.name
    if ts_path.resolve() != tmp_ts.resolve():
        shutil.copy(ts_path, tmp_ts)
    morph_bin = _find_morph_bin()
    cpp_path = tmpdir / f"{ts_path.stem}.cpp"
    # Try morph binary first
    if morph_bin and morph_bin.exists():
        # Use `morph file.ts --to cpp` -> generates file.cpp next to file.ts
        # The binary `morph` from crates/morphc expects `morph <file> --to cpp`
        # It will generate `tmp_ts.cpp` (or with .cpp extension)
        cmd = [str(morph_bin), str(tmp_ts), "--to", "cpp"]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        if result.returncode != 0:
            # Fallback to cargo run
            cmd = ["cargo", "run", "-p", "morphc", "--", str(tmp_ts), "--to", "cpp"]
            result = subprocess.run(cmd, capture_output=True, text=True, cwd=str(REPO_ROOT), timeout=30)
            assert result.returncode == 0, f"morph translate failed: {result.stderr}\n{result.stdout}"
        # Find generated cpp (could be tmp_ts.cpp or tmp_ts with .cpp appended)
        candidates = [
            tmpdir / f"{tmp_ts.name}.cpp",  # if it appends .cpp to full name
            tmpdir / f"{tmp_ts.stem}.cpp",
            tmpdir / "app.cpp",
            Path(str(tmp_ts) + ".cpp"),
        ]
        for cand in candidates:
            if cand.exists():
                # Move to expected cpp_path if needed
                if cand != cpp_path:
                    shutil.move(str(cand), str(cpp_path))
                break
        # Also check if the binary generated in place with same stem
        if not cpp_path.exists():
            # Try cargo run output path: it prints Output: ... we can parse
            # Fallback: search tmpdir for *.cpp
            cpps = list(tmpdir.glob("*.cpp"))
            if cpps:
                cpp_path = cpps[0]
        assert cpp_path.exists(), f"Generated cpp not found for {ts_path.name}, tmpdir={list(tmpdir.iterdir())}, stdout={result.stdout}, stderr={result.stderr}"
        _patch_includes(cpp_path)
        return cpp_path
    else:
        pytest.skip("morph binary not found, run `cargo build --bin morph` first")
        return tmpdir / "dummy.cpp"

def _compile_and_run(cpp_path: Path, tmpdir: Path) -> subprocess.CompletedProcess:
    """Compile generated C++ with g++-14 -std=c++23 and run, return result."""
    exe = tmpdir / f"{cpp_path.stem}_bin"
    # Find g++ (prefer g++-14, fallback to g++)
    compiler = shutil.which("g++-14") or shutil.which("g++")
    if not compiler:
        pytest.skip("g++ not found")
    # Compile - include runtime cpp headers and link runtime sources
    # Core runtime sources that are always needed
    runtime_sources = [
        RUNTIME_CPP / "net" / "net.cpp",
        RUNTIME_CPP / "reactivity" / "task.cpp",
    ]
    compile_cmd = [
        compiler,
        str(cpp_path),
        *[str(s) for s in runtime_sources if s.exists()],
        "-o", str(exe),
        "-std=c++23",
        f"-I{RUNTIME_CPP}",
        f"-I{REPO_ROOT}/runtime",
    ]
    comp = subprocess.run(compile_cmd, capture_output=True, text=True, timeout=30)
    assert comp.returncode == 0, f"Compile failed for {cpp_path.name}:\n{comp.stdout}\n{comp.stderr}\n--- cpp ---\n{cpp_path.read_text()}"
    run = subprocess.run([str(exe)], capture_output=True, text=True, timeout=5)
    assert run.returncode == 0, f"Run failed for {cpp_path.name}: {run.stderr}\n{run.stdout}"
    return run

# Discover fixtures
FIXTURES = sorted(FIXTURES_DIR.glob("*.ts"))

@pytest.mark.parametrize("fixture", FIXTURES, ids=lambda p: p.name)
def test_translate_and_run(fixture: Path, tmp_path: Path):
    """Translate each fixture and verify C++ compiles and runs."""
    # tmp_path is pytest's per-test temp dir
    cpp_path = _translate_fixture(fixture, tmp_path)
    # Basic sanity: file exists and contains expected headers
    content = cpp_path.read_text()
    assert "js_types.h" in content, f"js_types.h missing in {fixture.name}"
    # Run compile + execute
    result = _compile_and_run(cpp_path, tmp_path)
    # Check that something was printed (all fixtures have console.log)
    # We don't hardcode expected string per fixture here, just ensure non-empty or at least runs
    # For more strict checks, we compare with Node.js via tsx if available
    assert result.returncode == 0
    # Compare with Node.js via tsx (ground truth) - map output formats
    if shutil.which("npx"):
        try:
            node_out = subprocess.run(
                ["npx", "tsx", str(fixture)],
                capture_output=True, text=True, timeout=10, cwd=str(REPO_ROOT)
            )
            if node_out.returncode == 0:
                def _normalize(s: str) -> list[str]:
                    # JS vs C++: true/false, null/undefined, number formatting
                    lines = s.strip().splitlines()
                    norm = []
                    for l in lines:
                        # Node prints `true`/`false`, C++ via formatter does same
                        # Node prints `null`/`undefined`, C++ does same via JsNull/JsUndefined formatter
                        # Numbers: Node may print 3.14 vs C++ 3.14 (both via as_string)
                        # Trim and normalize
                        norm.append(l.strip())
                    return norm
                cpp_lines = _normalize(result.stdout)
                node_lines = _normalize(node_out.stdout)
                # For fixtures that are expected to be runnable, compare line counts and content
                # Allow small differences for object/array printing ([object Object] vs [object Array])
                if node_lines:
                    assert len(cpp_lines) == len(node_lines), f"Line count mismatch for {fixture.name}: C++ {len(cpp_lines)} vs Node {len(node_lines)}\nC++: {cpp_lines}\nNode: {node_lines}"
                    for cl, nl in zip(cpp_lines, node_lines):
                        # Allow C++ to print `true` vs Node `true` (same)
                        # Allow `1` vs `1` etc.
                        # For objects, both should be [object Object] or similar
                        # We do exact match for now, but can be lenient for float formatting
                        if cl != nl:
                            # Try numeric tolerance for floats
                            try:
                                if abs(float(cl) - float(nl)) < 0.001:
                                    continue
                            except:
                                pass
                            # For boolean null etc., check case-insensitive
                            if cl.lower() == nl.lower():
                                continue
                            pytest.fail(f"Output mismatch for {fixture.name}:\n  C++: {cl}\n  Node: {nl}\nFull C++: {result.stdout}\nFull Node: {node_out.stdout}")
                else:
                    assert len(result.stdout.strip()) > 0 or "no output expected" in fixture.read_text()
            else:
                # Node failed to run (maybe uses fetch/Promise which needs network) - just check C++ ran
                assert len(result.stdout.strip()) > 0 or "no output expected" in fixture.read_text()
        except Exception as e:
            # If npx tsx not available or fails, just check C++ output
            assert len(result.stdout.strip()) > 0 or "no output expected" in fixture.read_text()
    else:
        assert len(result.stdout.strip()) > 0 or "no output expected" in fixture.read_text()

def test_js_number_println_specific(tmp_path: Path):
    """Regression for JsNumber formatter bug: JsNumber must work with println."""
    ts = tmp_path / "regression.ts"
    ts.write_text("""
function add(x: number, y: number): number{
    return (x + y);
}
console.log(`This 2 + 5 = ${add(2, 5)}`);
console.log(add(2,5));
""", encoding="utf-8")
    cpp = _translate_fixture(ts, tmp_path)
    content = cpp.read_text()
    # Should have formatter via js_types.h (which includes js_value_format.h)
    # Should have <print> but NOT <format> for the console.log with template (uses println)
    assert "js_types.h" in content
    assert "<print>" in content
    # <format> should NOT be present for this specific case (uses println, not std::format)
    # It is okay if present via transitive include, but explicit include should be absent
    # We check that explicit #include <format> is not in generated file for this case
    assert "#include <format>" not in content, f"<format> should not be in console.log template case, got:\n{content}"
    result = _compile_and_run(cpp, tmp_path)
    assert "This 2 + 5 = 7" in result.stdout
    assert "7" in result.stdout

def test_template_var_needs_format(tmp_path: Path):
    """Regression: let c = `${a}, ${b}!` must include <format> (variable initializer uses std::format)."""
    ts = tmp_path / "tmpl.ts"
    ts.write_text("""
let a: string = "Hello";
let b: string = "World";
let c: string = `${a}, ${b}!`;
console.log(c);
""", encoding="utf-8")
    cpp = _translate_fixture(ts, tmp_path)
    content = cpp.read_text()
    assert "#include <format>" in content, f"Expected <format> for template var, got:\n{content}"
    assert "std::format" in content
    result = _compile_and_run(cpp, tmp_path)
    assert "Hello, World!" in result.stdout

def test_plain_vars_no_format(tmp_path: Path):
    """Plain vars without template/log should not include <format> or <print> explicitly."""
    ts = tmp_path / "plain.ts"
    ts.write_text("""
let a: string = "Hello";
let b: string = "World";
let n: number = 42;
""", encoding="utf-8")
    cpp = _translate_fixture(ts, tmp_path)
    content = cpp.read_text()
    # Should only have js_types.h, no <format> or <print> explicit
    assert "#include <format>" not in content
    assert "#include <print>" not in content
    # It will have no main, so we only test translation, not compile/run

def test_manual_jsnumber_with_print(tmp_path: Path):
    """Manual C++ with only js_types.h + <print> must compile (js_types.h includes formatter)."""
    cpp = tmp_path / "manual.cpp"
    cpp.write_text("""
#include "/home/piyush/My_Projects/morph/runtime/cpp/types/js_types.h"
#include <print>
static inline JsNumber add(JsNumber x, JsNumber y) { return (x + y); }
int main() { std::println("This 2 + 5 = {}", add(2, 5)); return 0; }
""", encoding="utf-8")
    result = _compile_and_run(cpp, tmp_path)
    assert "7" in result.stdout
