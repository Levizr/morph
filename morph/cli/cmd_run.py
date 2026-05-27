import os
import subprocess
import sys
from morph.config.loader import load_config
from morph.utils.logger import log_banner, log_step, log_error, log_success, log_info


def run(args=None) -> None:
    config = load_config()
    out_dir = config.output
    binary = os.path.join(out_dir, "app")

    if args and getattr(args, "binary", None):
        binary = args.binary

    if not os.path.exists(binary):
        log_error(f"Binary not found at {binary}")
        log_info("Run `morph build` first")
        sys.exit(1)

    log_banner("morph run")
    log_step(f"Running {binary}")
    log_info("Press Ctrl+C to stop\n")

    try:
        proc = subprocess.Popen([binary])
        proc.wait()
    except KeyboardInterrupt:
        proc.terminate()
        try:
            proc.wait(timeout=3)
        except Exception:
            proc.kill()
        log_success("Stopped")
    except FileNotFoundError:
        log_error(f"Binary not found: {binary}")
        sys.exit(1)
