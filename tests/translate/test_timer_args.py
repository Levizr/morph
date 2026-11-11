"""Runtime test for timer trailing arguments (Part 3D).

``setTimeout(fn, ms, ...args)`` and ``setInterval(fn, ms, ...args)`` capture
the trailing values at scheduling time and forward them to an inline arrow
callback on every fire. The test builds the fixture and drives the timer
queue through a test-only helper, then checks the printed values:

* the exit example prints ``3``;
* extra arguments beyond the callback's parameters are ignored;
* defaulted parameters still receive the supplied value when provided;
* intervals receive the same captured values on each tick.

Run:  pytest tests/translate/test_timer_args.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-timer-args"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


def test_timer_trailing_arguments(tmp_path: Path) -> None:
    if not shutil.which("g++-14") and not shutil.which("g++"):
        pytest.skip("g++ not found")
    project = tmp_path / "timer-args"
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

    binary = project / ".morph" / "output" / "pass-timer-args"
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

    lines = run.stdout.splitlines()
    assert "timeout sum 3" in lines, run.stdout
    assert "timeout ignored" in lines, run.stdout
    assert "defaulted supplied 6" in lines, run.stdout
    assert "dropped extra 3" in lines, run.stdout
    assert "interval tick 7" in lines, run.stdout
    assert lines.index("timeout sum 3") < lines.index("interval tick 7"), run.stdout
