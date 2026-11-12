"""Runtime test for Promise statics (Part 3E).

``Promise.resolve`` lowers to an eager runtime helper, and ``Promise.all`` /
``allSettled`` / ``race`` / ``any`` lower as the whole ``await`` of a
declaration or expression statement inside an async function. The test drives
a local HTTP server and checks:

* ``Promise.resolve`` returns the wrapped value;
* ``Promise.all`` preserves input order across fetch, resolve, and plain
  elements;
* ``Promise.allSettled`` reports fulfilled values and rejected reasons;
* a rejecting ``Promise.all`` rejects into the surrounding ``try``/``catch``;
* ``Promise.race`` / ``Promise.any`` take the first settled element while
  still evaluating later elements for side effects;
* an empty ``Promise.all([])`` yields an empty array;
* copying an already-resolved promise binding leaves both bindings usable.

Run:  pytest tests/translate/test_promise_statics.py -q
"""
from __future__ import annotations

import http.server
import shutil
import socketserver
import subprocess
import threading
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-promise-statics"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


class _Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:  # noqa: N802 (stdlib naming)
        body = b"Hello"
        self.send_response(200)
        self.send_header("content-type", "text/plain")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args: object) -> None:
        pass


@pytest.fixture()
def server() -> socketserver.TCPServer:
    with socketserver.TCPServer(("127.0.0.1", 0), _Handler) as srv:
        thread = threading.Thread(target=srv.serve_forever, daemon=True)
        thread.start()
        try:
            yield srv
        finally:
            srv.shutdown()
            thread.join(timeout=5)


def test_promise_statics_match_js_ordering(
    server: socketserver.TCPServer, tmp_path: Path
) -> None:
    if not shutil.which("g++-14") and not shutil.which("g++"):
        pytest.skip("g++ not found")
    project = tmp_path / "promise-statics"
    shutil.copytree(FIXTURE, project, ignore=shutil.ignore_patterns(".morph"))
    app = project / "src" / "App.mx"
    port = server.server_address[1]
    app.write_text(app.read_text().replace("PORT_OK", str(port)))

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

    binary = project / ".morph" / "output" / "pass-promise-statics"
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
    assert "resolve 1" in lines, run.stdout
    assert "returns 9 10 11 12" in lines, run.stdout
    assert "all Hello,middle,last" in lines, run.stdout
    assert "settled 2 fulfilled ok rejected" in lines, run.stdout
    assert any(line.startswith("all caught") for line in lines), run.stdout
    assert "race later second" in lines, run.stdout
    assert "race first" in lines, run.stdout
    assert "any alpha" in lines, run.stdout
    assert "empty 0" in lines, run.stdout
    assert "copy bound bound" in lines, run.stdout
    assert not any("unexpected" in line for line in lines), run.stdout
