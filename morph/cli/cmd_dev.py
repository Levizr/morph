import time
import os
from morph.config.loader import load_config
from morph.dev import devrt
from morph.dev.server import IPCClient
from morph.dev.watcher import SourceWatcher
from morph.dev import pipeline
from morph.utils.logger import log_info, log_error, log_success, log_step, log_banner


def run(args=None) -> None:
    config = load_config()

    if args and getattr(args, "entry", None):
        config.entry = args.entry

    log_banner("Dev Mode")

    log_step("Launching dev runtime")
    process = devrt.launch()
    time.sleep(0.5)

    log_step("Connecting to IPC")
    client = IPCClient()
    client.connect()

    def reload(changed_file: str = ""):
        if changed_file:
            log_info(f"Change detected: {changed_file}")
        ir = pipeline.run(config)
        if ir:
            client.send_ir(ir)
            log_success("Hot reloaded")
        else:
            log_error("Pipeline returned no IR — check your .mx file for errors")
            client.send_error("Build failed — check terminal")

    reload()

    # Watch the directory containing the entry file (resolved from CWD)
    entry_abs = os.path.abspath(os.path.join(os.getcwd(), config.entry))
    watch_dir = os.path.dirname(entry_abs)
    if not os.path.isdir(watch_dir):
        watch_dir = os.path.dirname(os.path.abspath(config.entry))
    watcher = SourceWatcher(watch_dir, on_change=reload)
    watcher.start()
    log_info(f"Watching {watch_dir} — Ctrl+C to stop\n")
    try:
        while True:
            time.sleep(0.5)
    except KeyboardInterrupt:
        watcher.stop()
        process.terminate()
        client.close()
        log_info("Dev mode stopped")
