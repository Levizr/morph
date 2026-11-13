"""Runtime test for the native Date subset (Part 4A).

The fixture builds with ``morph build --self-test`` and runs headlessly with
``TZ=UTC`` so local and UTC getters are deterministic. It covers:

* ``new Date(ms)``, ``new Date(text)``, ``new Date(year, month, ...)``;
* ISO parsing (date-only UTC, zoned offsets, local datetimes);
* ``Date.now()`` / ``Date.parse(...)``;
* local and UTC calendar getters;
* invalid dates (NaN time, NaN getters, ``toISOString`` RangeError).

Run:  pytest tests/translate/test_date.py -q
"""
from __future__ import annotations

import os
import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-date"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


def test_date_subset(tmp_path: Path) -> None:
    if not shutil.which("g++-14") and not shutil.which("g++"):
        pytest.skip("g++ not found")
    project = tmp_path / "date-subset"
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

    binary = project / ".morph" / "output" / "pass-date"
    assert binary.exists(), f"missing build output: {binary}"
    env = {**os.environ, "TZ": "UTC"}
    run = subprocess.run(
        [str(binary), "--morph-self-test"],
        cwd=project,
        capture_output=True,
        text=True,
        timeout=60,
        env=env,
    )
    assert run.returncode == 0, run.stderr + run.stdout
    assert "0 failures" in run.stdout, run.stdout

    lines = run.stdout.splitlines()
    assert "epoch 0 1970-01-01T00:00:00.000Z" in lines, run.stdout
    assert "utc 2020 0 2 4 3 4 5 6" in lines, run.stdout
    assert "local 2020 0 2 4 3 4 5 6" in lines, run.stdout
    assert "parts 2021-06-15T10:20:30.400Z 2021" in lines, run.stdout
    assert "rollover 2022-01-01T00:00:00.000Z" in lines, run.stdout
    assert "parse 1577923200000 1577925245006" in lines, run.stdout
    assert "parse bad true" in lines, run.stdout
    assert "now true" in lines, run.stdout
    assert "copy 1970-01-01T00:00:00.000Z 0" in lines, run.stdout
    assert "invalid true true" in lines, run.stdout
    assert "invalid iso RangeError" in lines, run.stdout
    assert "class 0" in lines, run.stdout
