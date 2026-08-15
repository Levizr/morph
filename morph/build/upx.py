"""UPX integration: find / auto-install the UPX executable on any OS and
compress the produced binary for the smallest possible on-disk size.

If `upx` is not on PATH, a prebuilt binary is downloaded from the official
GitHub releases into ~/.cache/morph/upx (works on Linux, macOS and Windows
without requiring root or a package manager).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tarfile
import urllib.request
import zipfile

from morph.utils.logger import log_info, log_error, log_success

UPX_VERSION = "4.2.4"
_RELEASE_URL = "https://github.com/upx/upx/releases/download/v{ver}/upx-{ver}-{asset}"

def _asset_name() -> str | None:
    machine = sys.platform
    arch = os.uname().machine.lower() if hasattr(os, "uname") else ""
    if machine == "win32":
        return "win64.zip" if sys.maxsize > 2**32 else "win32.zip"
    if machine == "darwin":
        if arch in ("arm64", "aarch64"):
            return "darwin_arm64.tar.xz"
        return "darwin_amd64.tar.xz"
    if machine.startswith("linux"):
        if arch in ("aarch64", "arm64"):
            return "arm64_linux.tar.xz"
        if arch in ("armv7l", "arm"):
            return "arm_linux.tar.xz"
        return "amd64_linux.tar.xz"
    return None


def _cache_dir() -> str:
    base = os.environ.get("XDG_CACHE_HOME", os.path.expanduser("~/.cache"))
    if sys.platform == "win32":
        base = os.environ.get("LOCALAPPDATA", os.path.expanduser("~"))
    return os.path.join(base, "morph", "upx")


def _download(asset: str, version: str) -> str | None:
    url = _RELEASE_URL.format(ver=version, asset=asset)
    cache = _cache_dir()
    os.makedirs(cache, exist_ok=True)
    exe = "upx.exe" if sys.platform == "win32" else "upx"

    pkg = os.path.join(cache, f"upx-{version}-{asset}")
    try:
        log_info(f"Downloading UPX {version} ({asset}) ...")
        req = urllib.request.Request(url, headers={"User-Agent": "morph/upx"})
        with urllib.request.urlopen(req, timeout=120) as resp, open(pkg, "wb") as f:
            while True:
                chunk = resp.read(65536)
                if not chunk:
                    break
                f.write(chunk)
    except Exception as e:
        log_error(f"UPX download failed: {e}")
        return None

    try:
        if asset.endswith(".zip"):
            with zipfile.ZipFile(pkg) as zf:
                zf.extractall(cache)
        else:
            with tarfile.open(pkg, "r:*") as tf:
                tf.extractall(cache)
    except Exception as e:
        log_error(f"UPX extraction failed: {e}")
        return None

    for root, _dirs, files in os.walk(cache):
        if exe in files:
            return os.path.join(root, exe)
    log_error("UPX archive did not contain the executable.")
    return None


def _find_cached(version: str) -> str | None:
    exe = "upx.exe" if sys.platform == "win32" else "upx"
    cache = _cache_dir()
    if not os.path.isdir(cache):
        return None
    for root, _dirs, files in os.walk(cache):
        if exe in files and f"upx-{version}" in root:
            return os.path.join(root, exe)
    return None


def ensure_upx(version: str | None = None) -> str | None:
    """Return a usable UPX binary.

    - version None: use system `upx` if on PATH, otherwise install the default.
    - version given: honor it — download that exact release even if a system
      `upx` exists, so users can pick what compresses their project best.
    """
    if version:
        log_success(f"UPX: using pinned version {version} from config/--upx-version")
    else:
        found = shutil.which("upx")
        if found:
            return found
        version = UPX_VERSION
        cached = _find_cached(version)
        if cached:
            return cached
        log_success("UPX not found — installing a prebuilt binary "
                    "(so `morph build` output can be compressed)...")
    asset = _asset_name()
    if not asset:
        log_error("No UPX build available for this platform.")
        return None
    return _download(asset, version)


def compress(binary_path: str, upx_bin: str) -> bool:
    """Compress binary_path in place with UPX. Returns True on success."""
    tmp = binary_path + ".upx"
    try:
        r = subprocess.run(
            [upx_bin, "--best", "-o", tmp, binary_path],
            capture_output=True, text=True, timeout=600,
        )
    except subprocess.TimeoutExpired:
        log_error("UPX compression timed out.")
        return False
    if r.returncode != 0:
        for line in (r.stdout + r.stderr).strip().split("\n")[-20:]:
            print(f"    {line}")
        log_error("UPX compression failed — keeping the uncompressed binary.")
        if os.path.exists(tmp):
            os.remove(tmp)
        return False
    os.replace(tmp, binary_path)
    return True
