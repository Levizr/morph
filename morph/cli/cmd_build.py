import os
import time
from morph.config.loader import load_config
from morph.dev import pipeline
from morph.codegen.emitter import Emitter
from morph.build.compiler import Compiler
from morph.build.optimizer import report_size
from morph.utils.logger import log_info, log_error


def run() -> None:
    config = load_config()
    log_info("Building production binary...")
    start = time.time()

    ir_dict = pipeline.run(config)
    if not ir_dict:
        log_error("Pipeline failed")
        return

    # TODO: deserialize ir_dict back to IRWindow list for emitter
    # Emitter().emit(windows, out_path="dist/app.cpp")
    # Compiler("prod").compile("dist/app.cpp", "dist/app")

    elapsed = time.time() - start
    out = os.path.join(config.output, "app")
    if os.path.exists(out):
        log_info(f"Done in {elapsed:.2f}s → dist/app ({report_size(out)})")
    else:
        log_info(f"Codegen complete in {elapsed:.2f}s (compile step TODO)")
