import os
import time
from morph.config.loader import load_config
from morph.dev import pipeline
from morph.codegen.emitter import Emitter
from morph.utils.logger import log_info, log_error, log_step, log_success, log_banner


def run(args=None) -> None:
    config = load_config()

    if args and getattr(args, "entry", None):
        config.entry = args.entry
    if args and getattr(args, "output", None):
        config.output = args.output

    log_banner("Production Build")

    log_step("Building production binary")
    start = time.time()

    ir_dict = pipeline.run(config)
    if not ir_dict:
        log_error("Pipeline failed")
        return

    log_step("Generating C++ code")
    # TODO: deserialize ir_dict back to IRWindow list for emitter
    # Emitter().emit(windows, out_path="dist/app.cpp")

    elapsed = time.time() - start
    out_path = os.path.join(config.output, "app")
    log_success(f"Codegen complete in {elapsed:.2f}s")
    log_info(f"Output: {out_path} (compile step TODO)")
