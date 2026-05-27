import time
import os
import sys
from morph.config.loader import load_config
from morph.dev import devrt
from morph.dev.server import IPCClient
from morph.dev.watcher import SourceWatcher
from morph.dev import pipeline
from morph.utils.logger import log_info, log_error, log_success, log_step, log_banner, log_dim, log_muted


def _plural(n: float, unit: str) -> str:
    if unit == "ms":
        return f"{n*1000:.1f}" if n < 0.5 else f"{n:.2f}"
    return f"{n:.1f}{unit}"


def _fmt_duration(secs: float) -> str:
    if secs < 0.5:
        return f"{secs*1000:.1f}ms"
    return f"{secs:.2f}s"


def run(args=None) -> None:
    config = load_config()

    if args and getattr(args, "entry", None):
        config.entry = args.entry

    t0 = time.time()

    log_banner("DEV")

    t1 = time.time()
    process = devrt.launch()
    log_success(f"morph_devrt built in {_fmt_duration(time.time() - t1)}")

    t1 = time.time()
    log_step("Connecting to IPC")
    client = IPCClient()
    client.connect()
    log_success(f"IPC connected in {_fmt_duration(time.time() - t1)}")

    def reload(changed_file: str = ""):
        t_reload = time.time()
        if changed_file:
            rel = os.path.relpath(changed_file, os.getcwd())
            log_info(f"File changed: {rel}")
        ir = pipeline.run(config)
        if ir:
            client.send_ir(ir)
            log_success(f"Rebuilt & reloaded in {_fmt_duration(time.time() - t_reload)}")
        else:
            log_error("Build failed — check terminal output above")
            client.send_error("Build failed — check terminal output above")

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
