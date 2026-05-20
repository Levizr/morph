import socket
import json
import os


SOCKET_PATH = "/tmp/morph_dev.sock"


class IPCClient:
    """Sends IR updates from Python to morph_devrt over Unix socket."""

    def __init__(self):
        self.sock: socket.socket | None = None

    def connect(self) -> None:
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(SOCKET_PATH)

    def send_ir(self, ir: dict) -> None:
        payload = json.dumps(ir).encode() + b"\x00"
        self.sock.sendall(payload)

    def send_error(self, msg: str) -> None:
        payload = json.dumps({"__error__": msg}).encode() + b"\x00"
        self.sock.sendall(payload)

    def close(self) -> None:
        if self.sock:
            self.sock.close()
