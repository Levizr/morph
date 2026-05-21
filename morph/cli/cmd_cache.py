import shutil
import os

from morph.utils.logger import log_info, log_success, log_warn

CACHE_DIR = ".morph/css-cache"


def run() -> None:
    if os.path.exists(CACHE_DIR):
        log_info(f"Clearing cache: {CACHE_DIR}")
        shutil.rmtree(CACHE_DIR)
        log_success("Cache cleared")
    else:
        log_warn("Nothing to clear")
