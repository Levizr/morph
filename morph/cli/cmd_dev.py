import threading
import time
import os
import sys
import hashlib
from morph.config.loader import load_config
from morph.dev import devrt
from morph.dev.server import IPCClient
from morph.dev.watcher import SourceWatcher
from morph.dev import pipeline
from morph.utils.logger import log_info, log_error, log_success, log_step, log_banner, log_dim, log_muted

_DIM = "\033[2m"
_RESET = "\033[0m"
_CYAN = "\033[36m"
_GREEN = "\033[32m"
_RED = "\033[31m"
_PRINT_LOCK = threading.Lock()
_SPINNER_CHARS = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"


class _Spinner:
    """In-place single-line loader printed while a rebuild is in progress."""

    def __init__(self, msg: str):
        self._msg = msg
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, daemon=True)

    def _run(self) -> None:
        i = 0
        while not self._stop.is_set():
            with _PRINT_LOCK:
                ch = _SPINNER_CHARS[i % len(_SPINNER_CHARS)]
                sys.stdout.write(f"\r  {_CYAN}{ch}{_RESET}  {self._msg} ...")
                sys.stdout.flush()
            i += 1
            self._stop.wait(0.08)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._thread.join()
        with _PRINT_LOCK:
            sys.stdout.write("\r" + " " * 80 + "\r")
            sys.stdout.flush()


def _fmt_duration(secs: float) -> str:
    if secs < 0.5:
        return f"{secs*1000:.1f}ms"
    return f"{secs:.2f}s"


def _wait_settle(path: str, timeout: float = 2.0, poll: float = 0.05) -> None:
    """Wait until a file's content stops changing, so we never read it mid-write.

    Editors often truncate then rewrite, or write in stages; the watcher fires on
    the first touch, which can be a partially-written file."""
    def _hash() -> str | None:
        try:
            with open(path, "rb") as f:
                return hashlib.sha256(f.read()).hexdigest()
        except OSError:
            return None

    prev = _hash()
    deadline = time.time() + timeout
    while time.time() < deadline:
        time.sleep(poll)
        cur = _hash()
        if cur is not None and cur == prev:
            return
        prev = cur


_active_spinner: _Spinner | None = None
_pending_lines: list[tuple[bool, str]] = []


def _stream_reader(stream, plain: bool = False) -> None:
    """Read a subprocess stream line-by-line and print it, dimmed unless it's
    user app output (stdout). Keeps runtime output in real-time order."""
    for line in iter(stream.readline, ""):
        line = line.rstrip("\n")
        if not line.strip():
            continue
        with _PRINT_LOCK:
            if _active_spinner is not None:
                _pending_lines.append((plain, line))
            elif plain:
                print(f"  {line}")
            else:
                print(f"  {_DIM}{line}{_RESET}")
    try:
        stream.close()
    except Exception:
        pass


def _spinner_stop() -> None:
    """Stop the active spinner, clear its line, and flush buffered output."""
    global _active_spinner
    sp = _active_spinner
    _active_spinner = None
    if sp is None:
        return
    sp.stop()
    with _PRINT_LOCK:
        for plain, line in _pending_lines:
            if plain:
                print(f"  {line}")
            else:
                print(f"  {_DIM}{line}{_RESET}")
        _pending_lines.clear()


def run(args=None) -> None:
    config = load_config()

    if args and getattr(args, "entry", None):
        config.entry = args.entry

    t0 = time.time()

    log_banner("DEV")

    t1 = time.time()
    process = devrt.launch(
        cmake_bin=getattr(config.build, "cmake", "") or None)
    log_success(f"morph_devrt built in {_fmt_duration(time.time() - t1)}")

    # Read devrt stdout (user app logs) and stderr (internal) in real time
    threading.Thread(target=_stream_reader, args=(process.stdout, True), daemon=True).start()
    threading.Thread(target=_stream_reader, args=(process.stderr, False), daemon=True).start()

    t1 = time.time()
    log_step("Connecting to IPC")
    client = IPCClient()
    client.connect()
    log_success(f"IPC connected in {_fmt_duration(time.time() - t1)}")

    _file_hashes: dict[str, str] = {}
    _reloading = False

    def reload(changed_file: str = ""):
        nonlocal _file_hashes, _reloading
        global _active_spinner
        if _reloading:
            return
        _reloading = True
        try:
            t_reload = time.time()
            if changed_file:
                rel = os.path.relpath(changed_file, os.getcwd())
                # Wait for the editor to finish writing before reading the file.
                _wait_settle(changed_file)
                # Skip if file content hasn't actually changed
                try:
                    with open(changed_file, "rb") as _f:
                        cur = hashlib.sha256(_f.read()).hexdigest()
                    if _file_hashes.get(changed_file) == cur:
                        return
                    _file_hashes[changed_file] = cur
                except OSError:
                    pass
                client.send_log("info", f"File changed: {rel}")
                _active_spinner = _Spinner(f"compiling {rel}")
                _active_spinner.start()
            ir = pipeline.run(config, verbose=changed_file == "")
            if not (ir and ir.get("windows")) and changed_file:
                # Likely read the file mid-write; retry once after it settles.
                time.sleep(0.15)
                ir = pipeline.run(config, verbose=False)
            if ir and ir.get("windows"):
                # Compile JS logic to .so (if there are state vars, events, etc.)
                windows = pipeline.get_latest_windows()
                logic_ok = True
                if windows:
                    logic_ok = pipeline.compile_logic(windows, verbose=changed_file == "",
                                                      config=config)
                    so_path = pipeline.get_logic_so_path()
                    if so_path:
                        ir["logic_so_path"] = so_path
                if not logic_ok:
                    if changed_file:
                        _spinner_stop()
                    err = pipeline.last_error() or "Logic compilation failed — check terminal output above"
                    log_error(err)
                    client.send_error(err)
                else:
                    client.send_ir(ir)
                    if changed_file:
                        _spinner_stop()
                        with _PRINT_LOCK:
                            sys.stdout.write(
                                f"  {_GREEN}✓{_RESET}  compiled {rel} in "
                                f"{_fmt_duration(time.time() - t_reload)}\n")
                            sys.stdout.flush()
                        client.send_log("ok", f"Hot reloaded in {_fmt_duration(time.time() - t_reload)}")
                    else:
                        client.send_log("ok", f"Hot reloaded in {_fmt_duration(time.time() - t_reload)}")
                        log_success(f"Rebuilt & reloaded in {_fmt_duration(time.time() - t_reload)}")
            else:
                if changed_file:
                    _spinner_stop()
                err = pipeline.last_error() or "Build failed — check terminal output above"
                log_error(err)
                client.send_error(err)
        finally:
            if _active_spinner is not None:
                _spinner_stop()
            _reloading = False

    reload()

    elapsed = time.time() - t0
    log_dim(f"Ready in {_fmt_duration(elapsed)}")

    entry_abs = os.path.abspath(os.path.join(os.getcwd(), config.entry))
    watch_dir = os.path.dirname(entry_abs)
    if not os.path.isdir(watch_dir):
        watch_dir = os.path.dirname(os.path.abspath(config.entry))

    watcher = SourceWatcher(watch_dir, on_change=reload)
    watcher.start()
    log_info(f"Watching {watch_dir}")

    print(f"  {chr(0x2500)*60}")
    log_muted("")
    log_dim(f"Watcher active · Ctrl+C to stop · Close window to exit")
    log_muted("")

    try:
        while True:
            time.sleep(0.3)
            rc = process.poll()
            if rc is not None:
                if rc != 0:
                    log_error(f"morph_devrt exited with code {rc}")
                log_success("Window closed — dev mode stopping")
                break
    except KeyboardInterrupt:
        pass
    finally:
        watcher.stop()
        if process.poll() is None:
            process.terminate()
            try:
                process.wait(timeout=3)
            except Exception:
                process.kill()
        client.close()
        log_dim("Dev mode stopped")
