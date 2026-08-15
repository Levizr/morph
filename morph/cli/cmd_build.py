from __future__ import annotations

import os
import subprocess
import time
from morph.config.loader import load_config
from morph.dev import pipeline
from morph.codegen.emitter import Emitter
from morph.ir.node import IRWindow, IRNode
from morph.ir.style import IRStyle
from morph.ir.event import IREvent
from morph.utils.logger import log_info, log_error, log_step, log_success, log_banner, log_dim, log_key, log_bullet


def _fmt_bytes(n: int) -> str:
    if n >= 1024 * 1024:
        return f"{n / (1024*1024):.2f} MB"
    if n >= 1024:
        return f"{n / 1024:.1f} KB"
    return f"{n} B"


def _fmt_duration(secs: float) -> str:
    if secs < 0.5:
        return f"{secs*1000:.1f}ms"
    return f"{secs:.2f}s"


FEATURE_SIZE_ESTIMATES: dict[str, tuple[str, str]] = {
    "text":           ("Text rendering (SDF)", "8-12 KB"),
    "scroll":         ("Scroll containers",    "3-5 KB"),
    "radius":         ("Border radius",        "1-2 KB"),
    "bold":           ("Bold text variant",    "<1 KB"),
    "position":       ("Positioned layout",   "1-2 KB"),
    "flex":           ("Flexbox layout",      "4-6 KB"),
    "cursor":         ("Custom cursors",      "<1 KB"),
    "border":         ("Border rendering",    "2-4 KB"),
    "image":          ("Image (stb_image)",   "25-30 KB"),
    "event":          ("Event system",        "2-3 KB"),
    "margin_collapse":("Margin collapse",    "<1 KB"),
    "display_none":   ("Display none",       "<1 KB"),
    "inline":         ("Inline layout",      "<1 KB"),
    "min_max":        ("Min/max sizing",     "1-2 KB"),
    "border_box":     ("Border-box sizing",  "<1 KB"),
    "viewport":       ("Viewport driver",    "3-5 KB"),
    "button":         ("Button widget",      "1-2 KB"),
    "input":          ("Input widget",       "2-3 KB"),
}


def _size_sections(binary_path: str) -> dict[str, int] | None:
    try:
        r = subprocess.run(["size", binary_path], capture_output=True, text=True)
        if r.returncode != 0:
            return None
        lines = r.stdout.strip().split("\n")
        # typical output: "   text    data     bss     dec     hex filename"
        parts = lines[-1].split()
        return {"text": int(parts[0]), "data": int(parts[1]), "bss": int(parts[2]),
                "total": int(parts[3])}
    except Exception:
        return None


def run(args=None) -> None:
    config = load_config()

    if args and getattr(args, "entry", None):
        config.entry = args.entry
    if args and getattr(args, "output", None):
        config.output = args.output

    t_start = time.time()

    log_banner("morph build")

    # ── 1. Pipeline ───────────────────────────────────
    log_step("Analyzing source")
    ir_dict = pipeline.run(config)
    if not ir_dict:
        log_error("Pipeline failed")
        return

    windows = _deserialize(ir_dict)
    if not windows:
        log_error("No windows in IR output")
        return

    # ── 2. Feature scan ────────────────────────────────
    from morph.codegen.feature_set import FeatureSet
    features = FeatureSet()
    features.scan(windows)
    active = features.features

    log_step("Assets & features")
    enabled = sorted(active)
    if enabled:
        for f in enabled:
            label, est = FEATURE_SIZE_ESTIMATES.get(f, (f, "?"))
            log_bullet(f"{label}  {_DIM}~{est}{_RESET}")
    else:
        log_dim("(no optional features detected)")

    freetype = features.needs_freetype()
    if freetype:
        log_bullet(f"FreeType library  {_DIM}~40-50 KB{_RESET}")

    # ── 3. Codegen ─────────────────────────────────────
    log_step("Generating C++ code")
    out_dir = config.output
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, "app.cpp")

    emitter = Emitter()
    t1 = time.time()
    emitter.emit(windows, out_path)
    cpp_size = os.path.getsize(out_path)
    log_success(f"app.cpp  {_fmt_bytes(cpp_size)}  in {_fmt_duration(time.time() - t1)}")

    # ── 4. Compile ─────────────────────────────────────
    log_step("Compiling binary")
    from morph.build.compiler import Compiler
    from morph.build.platform import exe_suffix
    compiler = Compiler()
    binary_path = os.path.join(out_dir, "app" + exe_suffix())

    t1 = time.time()
    compiler.silent = True
    static = bool(getattr(args, "static", False))
    if static:
        log_bullet("Static linking enabled — bundling GLFW/FreeType/HarfBuzz "
                   "(auto-built from source if missing)")
        log_bullet(f"GLFW backend: {'Wayland + X11' if config.build.wayland else 'X11 only (Wayland off)'}")
        if config.build.system_freetype:
            log_bullet("FreeType: system archive (config.build.system_freetype)")
        else:
            log_bullet("FreeType: trimmed self-build (zlib/png/brotli off)")

    # UPX compression: config default is on; --upx / --no-upx override.
    upx_enabled = config.build.upx
    if getattr(args, "upx", None) is not None:
        upx_enabled = args.upx
    upx_version = config.build.upx_version or None
    if getattr(args, "upx_version", None):
        upx_version = args.upx_version
    upx_bin = None
    if upx_enabled:
        from morph.build.upx import ensure_upx
        log_bullet("UPX: enabled — compressing binary after link"
                   + (f" (version {upx_version})" if upx_version else ""))
        upx_bin = ensure_upx(upx_version)

    ok = compiler.compile(out_path, binary_path,
                          needs_freetype=freetype,
                          needs_harfbuzz=freetype,
                          defines=features.required_defines(),
                          static=static,
                          wayland=config.build.wayland,
                          system_freetype=config.build.system_freetype)

    if not ok:
        log_error("Compilation failed — run `morph doctor` to check dependencies")
        return

    if upx_bin:
        log_info("Compressing binary with UPX ...")
        from morph.build.upx import compress
        compressed = compress(binary_path, upx_bin)
    else:
        compressed = False

    compile_time = _fmt_duration(time.time() - t1)
    bin_size = os.path.getsize(binary_path)

    # ── 5. Binary analysis ────────────────────────────
    sections = _size_sections(binary_path)
    total_time = _fmt_duration(time.time() - t_start)

    log_banner("Build complete")

    if sections and sections["total"] > 0:
        log_key("Binary",  f"{binary_path}  ({_fmt_bytes(bin_size)})")
        log_key("Code",    f"{_fmt_bytes(sections['text'])}")
        log_key("Data",    f"{_fmt_bytes(sections['data'])}")
        log_key("BSS",     f"{_fmt_bytes(sections['bss'])}")
    else:
        # UPX-compressed ELF has no section table, so `size` reads 0.
        note = "  (UPX-compressed)" if compressed else ""
        log_key("Output",  f"{binary_path}  ({_fmt_bytes(bin_size)}){note}")

    log_key("Compile", compile_time)
    log_key("Total",   total_time)


def _deserialize(ir_dict: dict) -> list:
    windows = []
    for w in ir_dict.get("windows", []):
        win = IRWindow(
            window_id=w.get("id", "win_0001"),
            title=w.get("title", "App"),
            width=w.get("width", 800),
            height=w.get("height", 600),
            visible=w.get("visible", True),
            renderer=w.get("renderer", "flash"),
            nodes=[_deser_node(n) for n in w.get("nodes", [])],
            startup_logs=w.get("startup_logs", []),
            premain_functions=w.get("premain_functions", []),
            extra_headers=w.get("extra_headers", []),
            state_vars=w.get("state_vars", []),
            effect_decls=w.get("effect_decls", []),
        )
        windows.append(win)
    return windows


def _clean_margin(val):
    """Convert None back to float('inf') (DEFERRED) after JSON round-trip."""
    import math
    if val is None:
        return math.inf
    return float(val)


def _clean_float(val: float | None, default: float = 0.0) -> float:
    """Convert None back to default after JSON round-trip (inf → None → default)."""
    if val is None:
        return default
    return float(val)


def _deser_style(s: dict) -> IRStyle:
    return IRStyle(
        bg_color=tuple(s.get("bg_color", [0, 0, 0, 0])),
        color=tuple(s.get("color", [0, 0, 0, 1])),
        width=s.get("width"),
        height=s.get("height"),
        margin=tuple(_clean_margin(v) for v in s.get("margin", [0, 0, 0, 0])),
        margin_auto=tuple(s.get("margin_auto", [False, False, False, False])),
        padding=tuple(s.get("padding", [0, 0, 0, 0])),
        border_radius=_clean_float(s.get("border_radius"), 0.0),
        font_size=_clean_float(s.get("font_size"), 16.0),
        font_weight=s.get("font_weight", "normal"),
        text_align=s.get("text_align", "left"),
        max_width=s.get("max_width"),
        display=s.get("display", "block"),
        flex_dir=s.get("flex_dir", "row"),
        flex_grow=_clean_float(s.get("flex_grow"), 0.0),
        flex_shrink=_clean_float(s.get("flex_shrink"), 1.0),
        flex_basis=s.get("flex_basis", "auto"),
        gap=_clean_float(s.get("gap"), 0.0),
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
        scrollbar_width=_clean_float(s.get("scrollbar_width"), 8.0),
        scrollbar_track_color=tuple(s.get("scrollbar_track_color", [0.85, 0.85, 0.85, 0.4])),
        scrollbar_thumb_color=tuple(s.get("scrollbar_thumb_color", [0.5, 0.5, 0.5, 0.6])),
        scrollbar_border_radius=_clean_float(s.get("scrollbar_border_radius"), 4.0),
        border_width=_clean_float(s.get("border_width"), 0.0),
        border_color=tuple(s.get("border_color", [0, 0, 0, 1])),
        border_style=s.get("border_style", "none"),
        box_sizing=s.get("box_sizing", "content-box"),
        z_index=s.get("z_index"),
    )


def _deser_node(d: dict) -> IRNode:
    hs = d.get("hover_style")
    hover_style = _deser_style(hs) if hs and isinstance(hs, dict) else None
    acs = d.get("active_style")
    active_style = _deser_style(acs) if acs and isinstance(acs, dict) else None
    # Deserialize ancestor_hover_rules
    ancestor_hover_rules = []
    for rule in d.get("ancestor_hover_rules", []):
        if isinstance(rule, dict):
            tag = rule.get("ancestor_tag", "")
            style_data = rule.get("style", {})
            if isinstance(style_data, dict) and tag:
                ancestor_hover_rules.append((tag, _deser_style(style_data)))
    # Deserialize ancestor_active_rules
    ancestor_active_rules = []
    for rule in d.get("ancestor_active_rules", []):
        if isinstance(rule, dict):
            tag = rule.get("ancestor_tag", "")
            style_data = rule.get("style", {})
            if isinstance(style_data, dict) and tag:
                ancestor_active_rules.append((tag, _deser_style(style_data)))
    then_nodes = [_deser_node(c) for c in d.get("then_nodes", [])]
    else_nodes = [_deser_node(c) for c in d.get("else_nodes", [])]
    def _float(key: str, default: float = 0.0) -> float:
        v = d.get(key, default)
        return default if v is None else float(v)

    return IRNode(
        node_id=d.get("id", ""),
        node_type=d.get("type", ""),
        x=_float("x"),
        y=_float("y"),
        w=_float("w"),
        h=_float("h"),
        text_content=d.get("text", ""),
        reactive_text=d.get("reactive_text", ""),
        reactive_class=d.get("reactive_class", ""),
        reactive_style=d.get("reactive_style", {}),
        class_conditional_effects=[
            (str(c), dict(o), dict(f))
            for c, o, f in d.get("class_conditional_effects", [])
        ],
        condition_expr=d.get("condition_expr", ""),
        then_nodes=then_nodes,
        else_nodes=else_nodes,
        style=_deser_style(d.get("style", {})),
        hover_style=hover_style,
        active_style=active_style,
        children=[_deser_node(c) for c in d.get("children", [])],
        attrs=d.get("attrs", {}),
        raw_styles=d.get("raw_styles", {}),
        transition_duration=d.get("transition_duration", 0.0),
        transition_easing=d.get("transition_easing", "ease-in-out"),
        events=[
            IREvent(trigger=e.get("trigger", ""), action=e.get("action", ""),
                    target=e.get("target", ""))
            for e in d.get("events", [])
        ],
        ancestor_hover_rules=ancestor_hover_rules,
        ancestor_active_rules=ancestor_active_rules,
    )


_RESET   = "\033[0m"
_DIM     = "\033[2m"
