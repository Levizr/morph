"""Runtime test for JSON dot access (Part 3A).

``JSON.parse(...)`` and ``Response.json()`` produce ``JsValue``; dot access
must lower to the same bracket lookup as ``["key"]`` so inline call results
and awaited readers read the field instead of emitting a missing C++
member.

Run:  pytest tests/translate/test_json_dot_access.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-json-dot"
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


def test_json_dot_access_reads_fields(tmp_path: Path) -> None:
    _find_compiler()
    project = tmp_path / "json-dot"
    shutil.copytree(FIXTURE, project, ignore=shutil.ignore_patterns(".morph"))
    morph = str(_find_morph_bin())

    check = subprocess.run(
        [morph, "check"],
        cwd=project,
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert check.returncode == 0, check.stdout + check.stderr

    build = subprocess.run(
        [morph, "build", "--no-upx", "--self-test"],
        cwd=project,
        capture_output=True,
        text=True,
        timeout=600,
    )
    assert build.returncode == 0, build.stdout + build.stderr

    binary = project / ".morph" / "output" / "pass-json-dot"
    assert binary.exists(), f"missing build output: {binary}"
    run = subprocess.run(
        [str(binary), "--morph-self-test"],
        cwd=project,
        capture_output=True,
        text=True,
        timeout=60,
    )
    assert run.returncode == 0, run.stderr + run.stdout
    assert "0 failures" in run.stdout, run.stdout

    lines = [line for line in run.stdout.splitlines() if line.strip()]
    assert "json fields Ada Ada 36 true" in lines, run.stdout
    assert "json inline Ada true" in lines, run.stdout
