from __future__ import annotations

import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRWindow
from morph.codegen.feature_set import FeatureSet
from morph.codegen.node_emitter import NodeEmitter
from morph.codegen.logic_emitter import emit_logic as _emit_logic

_TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), "templates")


class Emitter:
    """Renders IR into app.cpp using Jinja2 templates + NodeEmitter."""

    def __init__(self):
        self.env = Environment(
            loader=FileSystemLoader(_TEMPLATES_DIR),
            trim_blocks=True,
            lstrip_blocks=True,
        )
        self.node_emitter = NodeEmitter()

    def emit(self, windows: list[IRWindow], out_path: str,
             mode: str = "prod") -> None:
        features = FeatureSet()
        features.scan(windows)
        self.node_emitter.features = features.features

        # ── Collect premain functions from all windows ────
        seen = set()
        premain_parts = []
        for w in windows:
            for f in w.premain_functions:
                if f not in seen:
                    seen.add(f)
                    premain_parts.append(f)
        premain_code = "\n\n".join(premain_parts)

        # ── Collect state vars (morphState) ──────────────────
        state_decls = []
        for w in windows:
            for sv in w.state_vars:
                init = sv.get("init", "0")
                name = sv.get("getter", "")
                sig_name = f"__st_{name}" if name else "__st_unknown"
                # Determine C++ type from the init value
                raw_init = init.strip()
                if raw_init.startswith("'") and raw_init.endswith("'"):
                    init = '"' + raw_init[1:-1] + '"'
                    raw_init = init
                if raw_init in ("true", "false"):
                    cpp_type = "bool"
                elif raw_init.startswith('"') or raw_init.startswith("'"):
                    cpp_type = "std::string"
                elif "." in raw_init:
                    cpp_type = "double"
                elif raw_init.isdigit() or (raw_init.startswith("-") and raw_init[1:].isdigit()):
                    cpp_type = "int"
                else:
                    cpp_type = "auto"  # let CTAD deduce or fallback
                state_decls.append({
                    "signal_name": sig_name,
                    "type": cpp_type,
                    "init": init,
                })

        # ── Collect extra headers (JS runtime types, etc.) ──
        _builtin_headers = {"<cstdio>", "<print>", "<chrono>"}
        extra_headers = sorted(
            set(h for w in windows for h in w.extra_headers)
            - _builtin_headers
        )
        # Prepend runtime headers (deduplicated)
        _rt_headers = [
            "\"../../morph/runtime/reactivity/signal.h\"",
            "\"../../morph/runtime/reactivity/task.h\"",
        ]
        for h in reversed(_rt_headers):
            if h not in extra_headers:
                extra_headers.insert(0, h)

        # ── Generate node C++ code for each window ──────────
        window_code = []
        for win in windows:
            win_code = []
            var = f"win_{win.window_id}"
            win_code.append(
                f"MorphWindow* {var} = new MorphWindow("
                f'"{win.title}", {win.width}, {win.height}, '
                f'{"true" if win.visible else "false"});'
            )
            win_code.append(f'wm.registerWindow("{win.window_id}", {var});')
            win_code.append("")

            if len(win.nodes) == 1:
                for node in win.nodes:
                    code = self.node_emitter.emit_node(node, var)
                    if code:
                        win_code.append(code)
                        win_code.append("")
            else:
                # Multiple top-level nodes — wrap in a container so only one addChild()
                root_id = f"winRoot_{win.window_id}"
                win_code.append(
                    f"RectNode* {root_id} = new RectNode(0.0f, 0.0f, "
                    f"{float(win.width):.1f}f, 0.0f);"
                )
                win_code.append(f"{var}->addChild({root_id});")
                for node in win.nodes:
                    code = self.node_emitter.emit_node(node, root_id)
                    if code:
                        win_code.append(code)
                        win_code.append("")

            if win.startup_logs:
                win_code.append("")
                for msg in win.startup_logs:
                    escaped = msg.replace("\\", "\\\\").replace('"', '\\"')
                    win_code.append(f'    fprintf(stderr, "{escaped}\\n");')

            # ── morphEffect declarations ──
            for ed in win.effect_decls:
                cpp_lambda = ed["lambda"]
                deps = ed.get("deps", "").strip()
                if deps == "[]":
                    # Empty deps — run once, no re-subscription
                    win_code.append(f"    {{ // morphEffect (run once)")
                    win_code.append(f"        auto __ef_fn = {cpp_lambda};")
                    win_code.append(f"        __ef_fn();")
                    win_code.append(f"    }}")
                elif deps:
                    # With deps — create_effect, auto-subscription handles deps
                    win_code.append(f"    morph::create_effect({cpp_lambda});")
                else:
                    # No deps — create_effect with auto-subscription
                    win_code.append(f"    morph::create_effect({cpp_lambda});")

            window_code.append("\n".join(win_code))

        tmpl = self.env.get_template("app_main.cpp.j2")
        code = tmpl.render(
            windows=windows,
            window_code="\n".join(window_code),
            headers=features.required_headers(),
            extra_headers=extra_headers,
            defines=features.required_defines(),
            dev_mode=(mode == "dev"),
            premain_code=premain_code,
            state_decls=state_decls,
        )

        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as f:
            f.write(code)

    def emit_logic(self, windows: list[IRWindow], out_path: str) -> None:
        code = _emit_logic(windows)
        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as f:
            f.write(code)
