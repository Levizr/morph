"""Runtime test for fetch request options and the Response API (Part 3F).

The fixture is built with ``morph build --self-test`` and run headlessly
against a local HTTP server. It covers:

* GET plus ``status``/``ok()``/``statusText`` and response headers;
* POST with ``method``, request ``headers``, and a JSON string ``body``;
* ``response.json()`` for both GET and POST;
* ``response.headers.get(...)`` / ``.has(...)``, ``bodyUsed``, ``clone()``,
  and ``arrayBuffer()``.

Run:  pytest tests/translate/test_fetch_request.py -q
"""
from __future__ import annotations

import http.server
import json
import shutil
import socketserver
import subprocess
import threading
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
FIXTURE = REPO_ROOT / "tests" / "runtime" / "conformance" / "pass-fetch-request"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


class _Handler(http.server.BaseHTTPRequestHandler):
    def _send_json(self, status: int, payload: dict[str, object]) -> None:
        body = json.dumps(payload).encode()
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 (stdlib naming)
        if self.path == "/json":
            self._send_json(200, {"ok": True, "name": "Ada"})
            return
        self._send_json(404, {"error": "not found"})

    def do_POST(self) -> None:  # noqa: N802 (stdlib naming)
        if self.path != "/echo":
            self._send_json(404, {"error": "not found"})
            return
        length = int(self.headers.get("content-length", "0"))
        body = self.rfile.read(length).decode()
        payload = {
            "method": self.command,
            "contentType": self.headers.get("content-type", ""),
            "header": self.headers.get("x-morph-test", ""),
            "body": body,
        }
        response = json.dumps(payload).encode()
        self.send_response(201)
        self.send_header("content-type", "application/json")
        self.send_header("x-echo-content-type", self.headers.get("content-type", ""))
        self.send_header("content-length", str(len(response)))
        self.end_headers()
        self.wfile.write(response)

    def log_message(self, *_args: object) -> None:
        pass


@pytest.fixture()
def server() -> socketserver.TCPServer:
    with socketserver.ThreadingTCPServer(("127.0.0.1", 0), _Handler) as srv:
        thread = threading.Thread(target=srv.serve_forever, daemon=True)
        thread.start()
        try:
            yield srv
        finally:
            srv.shutdown()
            thread.join(timeout=5)


def test_fetch_request_and_response_api(
    server: socketserver.TCPServer, tmp_path: Path
) -> None:
    if not shutil.which("g++-14") and not shutil.which("g++"):
        pytest.skip("g++ not found")
    project = tmp_path / "fetch-request"
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

    binary = project / ".morph" / "output" / "pass-fetch-request"
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
    assert "get 200 true OK application/json" in lines, run.stdout
    assert f"get meta http://127.0.0.1:{port}/json false basic false" in lines, run.stdout
    assert "get json true Ada" in lines, run.stdout
    assert "post 201 true" in lines, run.stdout
    assert "post headers application/json true" in lines, run.stdout
    assert (
        'post json POST application/json yes {"count":2,"name":"Ada"}' in lines
    ), run.stdout
    assert "body used true" in lines, run.stdout
    assert "clone 201 false true" in lines, run.stdout
    assert "raw true" in lines, run.stdout
