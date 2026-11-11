"""Runtime test for `.then`/`.catch` compatibility chains (Part 3C).

A statement-level ``fetch(...).then(...).catch(...)`` chain inside an async
function lowers to ``co_await morph::net::fetch_response(...)`` plus the
callbacks in a try/catch. The test drives a local HTTP server and checks:

* the ``.then`` callback runs after the response arrives;
* a transport error rejects into ``.catch``;
* an exception thrown by the ``.then`` callback rejects into ``.catch``;
* a catch-only chain still awaits the fetch;
* a zero-parameter ``.then`` callback still awaits the fetch before running.

Run:  pytest tests/translate/test_then_chain.py -q
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
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-then-chain"
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


def test_then_chain_matches_js_ordering(server: socketserver.TCPServer, tmp_path: Path) -> None:
    if not shutil.which("g++-14") and not shutil.which("g++"):
        pytest.skip("g++ not found")
    project = tmp_path / "then-chain"
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

    binary = project / ".morph" / "output" / "pass-then-chain"
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
    assert "then Hello" in lines, run.stdout
    assert any(line.startswith("caught transport") for line in lines), run.stdout
    assert "caught throw boom" in lines, run.stdout
    assert any(line.startswith("caught only") for line in lines), run.stdout
    assert "then no param" in lines, run.stdout
    assert "caught only no param" in lines, run.stdout
    assert not any("unexpected" in line for line in lines), run.stdout
