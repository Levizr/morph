import socket
import json
import os
import time


HOST = "127.0.0.1"
PORT = 39573


class IPCClient:
    """Sends IR updates from Python to morph_devrt over loopback TCP."""

    def __init__(self):
        self.sock: socket.socket | None = None

    def connect(self, retries: int = 10, delay: float = 0.5) -> None:
        last_err = None
        for i in range(retries):
            try:
                self.sock = socket.create_connection((HOST, PORT), timeout=5)
                return
            except OSError as e:
                last_err = e
                if i < retries - 1:
                    time.sleep(delay)
        raise ConnectionError(f"Could not connect to morph_devrt at {HOST}:{PORT} "
                              f"after {retries} retries: {last_err}")

    def send_ir(self, ir: dict) -> None:
        payload = json.dumps(ir, ensure_ascii=False).encode() + b"\x00"
        self.sock.sendall(payload)

    def send_error(self, msg: str) -> None:
        payload = json.dumps({"__error__": msg}, ensure_ascii=False).encode() + b"\x00"
        self.sock.sendall(payload)

    def send_log(self, level: str, msg: str) -> None:
        """Send a console log message to the in-app DevTools (info|ok|warn|error)."""
        payload = json.dumps({"__log__": {"level": level, "msg": msg}},
                             ensure_ascii=False).encode() + b"\x00"
        self.sock.sendall(payload)

    def close(self) -> None:
        if self.sock:
            self.sock.close()
