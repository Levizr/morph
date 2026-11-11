"""Runtime test for imported-module side effects (Part 2E).

Top-level statements and late ``var`` declarations execute exactly once
when a module is imported, in dependency-first import order, with cycles
broken by a per-module guard. The fixture also reads a ``var`` before its
declaration, which must print ``undefined``.

Run:  pytest tests/translate/test_module_side_effects.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-module-side-effects"
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


def _copy_fixture(tmp_path: Path) -> Path:
    project = tmp_path / "module-side-effects"
    shutil.copytree(FIXTURE, project, ignore=shutil.ignore_patterns(".morph"))
    return project


def test_imported_module_side_effects_run_once_in_import_order(tmp_path: Path) -> None:
    _find_compiler()
    project = _copy_fixture(tmp_path)
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

    binary = project / ".morph" / "output" / "pass-module-side-effects"
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
    expected = [
        "state before undefined",
        "state after 10",
        "state table 2",
        "logger init 12",
        "cycle b before",
        "cycle b after ping",
        "cycle a before",
        "cycle a after b",
    ]
    for line in expected:
        assert lines.count(line) == 1, f"{line!r} should run exactly once:\n{run.stdout}"

    # Dependency-first order: the state module initializes before the
    # logger body that imports it; the cycle starts at its first import
    # (`cycle_b` before `cycle_a`), and each body runs once.
    indexes = [lines.index(line) for line in expected]
    assert indexes == sorted(indexes), run.stdout
