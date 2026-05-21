import time
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
            client.send_error("Build failed — check terminal")

    reload()

    watcher = SourceWatcher("src/", on_change=reload)
    watcher.start()

    log_info("Watching src/ — Ctrl+C to stop\n")
    try:
        while True:
            time.sleep(0.5)
    except KeyboardInterrupt:
        watcher.stop()
        process.terminate()
        client.close()
        log_info("Dev mode stopped")
