import time
from morph.config.loader import load_config
from morph.dev import devrt
from morph.dev.server import IPCClient
from morph.dev.watcher import SourceWatcher
from morph.dev import pipeline
from morph.utils.logger import log_info, log_error


def run() -> None:
    config = load_config()
    log_info("Starting dev mode...")

    process = devrt.launch()
    time.sleep(0.5)

    client = IPCClient()
    client.connect()

    def reload(changed_file: str = ""):
        if changed_file:
            print(f"\n[morph] Change: {changed_file}")
        ir = pipeline.run(config)
        if ir:
            client.send_ir(ir)
            log_info("Hot reloaded")
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
