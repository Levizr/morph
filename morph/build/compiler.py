from __future__ import annotations

import os
import subprocess
import shutil
from morph.utils.logger import log_info, log_error


class Compiler:
    """Invokes g++ to compile generated C++ into a native binary."""

    def __init__(self):
        self.gpp = shutil.which("g++") or shutil.which("clang++") or "g++"

    def compile(self, source_path: str, binary_path: str) -> bool:
        if not os.path.exists(source_path):
            log_error(f"Source not found: {source_path}")
            return False

        runtime_dir = os.path.join(os.path.dirname(__file__), "../../runtime")
        runtime_dir = os.path.abspath(runtime_dir)

        if not shutil.which(self.gpp):
            log_error(f"Compiler not found: {self.gpp}")
            log_error("Install g++: sudo apt install g++")
            return False

        cmd = [
            self.gpp,
            "-std=c++17",
            "-O2",
            source_path,
            "-o", binary_path,
            "-I", runtime_dir,
            "-lglfw", "-lGL", "-lX11", "-lpthread", "-ldl",
        ]

        log_info(f"Compiling: {' '.join(cmd)}")
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)

        if result.returncode != 0:
            log_error("Compilation failed:")
            for line in result.stderr.split("\n"):
                if line.strip():
                    print(f"  {line.strip()}")
            return False

        return True
