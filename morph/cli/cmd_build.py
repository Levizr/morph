from __future__ import annotations

import os
import time
from morph.config.loader import load_config
from morph.dev import pipeline
from morph.codegen.emitter import Emitter
from morph.ir.node import IRWindow, IRNode
from morph.ir.style import IRStyle
from morph.ir.event import IREvent
from morph.utils.logger import log_info, log_error, log_step, log_success, log_banner


def run(args=None) -> None:
    config = load_config()

    if args and getattr(args, "entry", None):
        config.entry = args.entry
    if args and getattr(args, "output", None):
        config.output = args.output

    log_banner("Production Build")

    log_step("Building IR")
    start = time.time()

    ir_dict = pipeline.run(config)
    if not ir_dict:
        log_error("Pipeline failed")
        return

    # ── Deserialize IR dict back to IRWindow list ──────────
    windows = _deserialize(ir_dict)
    if not windows:
        log_error("No windows in IR output")
        return

    log_step("Generating C++ code")
    out_dir = config.output
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, "app.cpp")

    from morph.codegen.feature_set import FeatureSet
    features = FeatureSet()
    features.scan(windows)

    emitter = Emitter()
    emitter.emit(windows, out_path)

    elapsed = time.time() - start
    size = os.path.getsize(out_path)
    log_success(f"Codegen complete in {elapsed:.2f}s")
    log_info(f"Output: {out_path} ({size} bytes)")

    # ── Compile ────────────────────────────────────────────
    log_step("Compiling binary")
    from morph.build.compiler import Compiler
    compiler = Compiler()
    binary_path = os.path.join(out_dir, "app")
    ok = compiler.compile(out_path, binary_path,
                          needs_freetype=features.needs_freetype(),
                          defines=features.required_defines())

    if ok:
        bin_size = os.path.getsize(binary_path) if os.path.exists(binary_path) else 0
        log_success(f"Binary: {binary_path} ({bin_size} bytes)")
    else:
        log_error("Compilation failed — run `morph doctor` to check dependencies")


def _deserialize(ir_dict: dict) -> list:
    """Convert IR dict back into IRWindow list for the emitter."""
    windows = []
    for w in ir_dict.get("windows", []):
        win = IRWindow(
            window_id=w.get("id", "win_0001"),
            title=w.get("title", "App"),
            width=w.get("width", 800),
            height=w.get("height", 600),
            visible=w.get("visible", True),
            nodes=[_deser_node(n) for n in w.get("nodes", [])],
            startup_logs=w.get("startup_logs", []),
        )
        windows.append(win)
    return windows


def _deser_node(d: dict) -> IRNode:
    s = d.get("style", {})
    style = IRStyle(
        bg_color=tuple(s.get("bg_color", [0, 0, 0, 0])),
        color=tuple(s.get("color", [0, 0, 0, 1])),
        width=s.get("width"),
        height=s.get("height"),
        margin=tuple(s.get("margin", [0, 0, 0, 0])),
        padding=tuple(s.get("padding", [0, 0, 0, 0])),
        border_radius=s.get("border_radius", 0.0),
        font_size=s.get("font_size", 16.0),
        font_weight=s.get("font_weight", "normal"),
        text_align=s.get("text_align", "left"),
        max_width=s.get("max_width"),
        display=s.get("display", "block"),
        flex_dir=s.get("flex_dir", "row"),
        flex=s.get("flex", 0.0),
        gap=s.get("gap", 0.0),
        overflow=s.get("overflow", "visible"),
        position=s.get("position", "static"),
        left=s.get("left"),
        right=s.get("right"),
        top=s.get("top"),
        bottom=s.get("bottom"),
        justify_content=s.get("justify_content", "flex-start"),
        align_items=s.get("align_items", "stretch"),
        flex_wrap=s.get("flex_wrap", "nowrap"),
        cursor=s.get("cursor", "default"),
        scrollbar_width=s.get("scrollbar_width", 8.0),
        scrollbar_track_color=tuple(s.get("scrollbar_track_color", [0.85, 0.85, 0.85, 0.4])),
        scrollbar_thumb_color=tuple(s.get("scrollbar_thumb_color", [0.5, 0.5, 0.5, 0.6])),
        scrollbar_border_radius=s.get("scrollbar_border_radius", 4.0),
        border_width=s.get("border_width", 0.0),
        border_color=tuple(s.get("border_color", [0, 0, 0, 1])),
        border_style=s.get("border_style", "none"),
        box_sizing=s.get("box_sizing", "content-box"),
    )
    node = IRNode(
        node_id=d.get("id", ""),
        node_type=d.get("type", ""),
        x=d.get("x", 0.0),
        y=d.get("y", 0.0),
        w=d.get("w", 0.0),
        h=d.get("h", 0.0),
        text_content=d.get("text", ""),
        style=style,
        children=[_deser_node(c) for c in d.get("children", [])],
        events=[
            IREvent(trigger=e.get("trigger", ""), action=e.get("action", ""),
                    target=e.get("target", ""))
            for e in d.get("events", [])
        ],
    )
    return node
