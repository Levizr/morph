"""Runtime test for the native RegExp subset (Part 4B).

The fixture builds with ``morph build --self-test`` and runs headlessly. It
covers:

* regex literals with no flags or ``i``;
* aliases and direct ``.test(...)`` / ``.exec(...)`` receivers;
* array-like exec results (``[0]``, ``index``, ``length``, ``input``);
* captures (including unmatched groups), misses, and reassignment;
* UTF-16 code-unit semantics for dot, astral literals, and classes;
* JS whitespace (``\\s``/``\\S``), hex/unicode/control escapes, lazy
  quantifiers, lookaheads, and empty classes.

Run:  pytest tests/translate/test_regexp.py -q
"""
from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-regexp"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


def test_regexp_subset(tmp_path: Path) -> None:
    if not shutil.which("g++-14") and not shutil.which("g++"):
        pytest.skip("g++ not found")
    project = tmp_path / "regexp-subset"
    shutil.copytree(FIXTURE, project, ignore=shutil.ignore_patterns(".morph"))
    morph = str(_find_morph_bin())

    check = subprocess.run(
        [morph, "check"], cwd=project, capture_output=True, text=True, timeout=120
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

    binary = project / ".morph" / "output" / "pass-regexp"
    assert binary.exists(), f"missing build output: {binary}"
    run = subprocess.run(
        [str(binary), "--morph-self-test"],
        cwd=project,
        capture_output=True,
        text=True,
        timeout=60,
        env=os.environ,
    )
    assert run.returncode == 0, run.stderr + run.stdout
    assert "0 failures" in run.stdout, run.stdout

    lines = run.stdout.splitlines()
    for expected in [
        "test true false false",
        "exec true ABBC 1 1 xABBC",
        "groups b undefined b 0 3",
        "miss true false",
        "dot true false false",
        "astral false false",
        "ws true true false",
        "negws false true",
        "unicode true true true",
        "lazy a 0",
        "lookahead true true",
        "index 1",
        "reassign n",
        "class true true true",
        "empty false true false",
        "noncapture true",
    ]:
        assert expected in lines, f"missing {expected!r} in:\n{run.stdout}"
