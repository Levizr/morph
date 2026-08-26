from __future__ import annotations

import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRWindow
from morph.codegen.feature_set import FeatureSet
from morph.codegen.node_emitter import NodeEmitter, keyframe_registration_code
from morph.codegen.logic_emitter import emit_logic as _emit_logic
from morph.codegen.native_header import (
    collect_cpp_imports,
    collect_state_vars,
    generate_state_header,
    strip_static_function,
)

_TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), "templates")

_STATE_HEADER_NAME = "_morph_state.h"


def collect_list_nodes(nodes: list) -> list:
    """All __list__ IR nodes in a node forest (incl. nested/conditional)."""
    out = []
    for n in nodes:
        if getattr(n, "node_type", "") == "__list__":
            out.append(n)
        if getattr(n, "item_template", None) is not None:
            out.extend(collect_list_nodes([n.item_template]))
        out.extend(collect_list_nodes(getattr(n, "children", [])))
        out.extend(collect_list_nodes(getattr(n, "then_nodes", [])))
        out.extend(collect_list_nodes(getattr(n, "else_nodes", [])))
    return out


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

        # ── User C++ imports (single-TU include for max performance) ──
        cpp_imports = collect_cpp_imports(windows)
        native_mode = bool(cpp_imports)
        if native_mode:
            # External linkage so the user's .cpp can call JSX functions.
            premain_parts = [strip_static_function(p) for p in premain_parts]
        premain_code = "\n\n".join(premain_parts)

        # ── Collect state vars (morphState) ──────────────────
        from morph.codegen.logic_emitter import _get_cpp_type as _logic_cpp_type
        from morph.codegen.logic_emitter import _clean_init as _logic_clean_init
        state_decls = []
        for w in windows:
            for sv in w.state_vars:
                init = _logic_clean_init(sv.get("init", "0"))
                name = sv.get("getter", "")
                sig_name = f"__st_{name}" if name else "__st_unknown"
                state_decls.append({
                    "signal_name": sig_name,
                    "type": _logic_cpp_type(sv.get("init", "0")),
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
            "\"../../runtime/cpp/reactivity/signal.h\"",
            "\"../../runtime/cpp/reactivity/task.h\"",
        ]
        for h in reversed(_rt_headers):
            if h not in extra_headers:
                extra_headers.insert(0, h)

        # ── Generate node C++ code for each window ──────────
        # @keyframes are app-global — register them once, before any window.
        keyframe_code = "\n".join(
            keyframe_registration_code(w.keyframes, features.features)
            for w in windows
        )

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

            _GLFW_DONT_CARE = -1
            has_constraints = any(x is not None for x in (
                win.min_width, win.max_width, win.min_height, win.max_height))
            if has_constraints:
                min_w = win.min_width if win.min_width is not None else _GLFW_DONT_CARE
                min_h = win.min_height if win.min_height is not None else _GLFW_DONT_CARE
                max_w = win.max_width if win.max_width is not None else _GLFW_DONT_CARE
                max_h = win.max_height if win.max_height is not None else _GLFW_DONT_CARE
                win_code.append(
                    f"{var}->setConstraints({min_w}, {min_h}, {max_w}, {max_h});"
                )
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
        list_factory_code = "\n\n".join(
            self.node_emitter.emit_item_factory(n)
            for n in collect_list_nodes([nd for w in windows for nd in w.nodes])
        )
        code = tmpl.render(
            windows=windows,
            window_code="\n".join(window_code),
            keyframe_code=keyframe_code,
            list_factory_code=list_factory_code,
            headers=features.required_headers(),
            extra_headers=extra_headers,
            defines=features.required_defines(),
            dev_mode=(mode == "dev"),
            premain_code=premain_code,
            state_decls=state_decls,
            native_mode=native_mode,
            cpp_includes=cpp_imports,
        )

        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as f:
            f.write(code)

        # Write the generated state header next to the TU so
        # `#include "_morph_state.h"` resolves relative to the source file.
        if native_mode:
            header = generate_state_header(windows, state_decls, premain_parts)
            header_path = os.path.join(
                os.path.dirname(out_path) or ".", _STATE_HEADER_NAME)
            with open(header_path, "w") as f:
                f.write(header)

    def emit_logic(self, windows: list[IRWindow], out_path: str) -> None:
        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)

        # The dev logic TU also gets the generated interop header, so user
        # .cpp files see state + JSX functions (single-TU, hot-reloadable).
        if collect_cpp_imports(windows):
            state_decls = []
            for sv in collect_state_vars(windows):
                name = sv.get("getter", "")
                init = sv.get("init", "0")
                sig = f"__st_{name}" if name else "__st_unknown"
                raw = init.strip()
                if raw.startswith("'") and raw.endswith("'"):
                    init = '"' + raw[1:-1] + '"'
                    raw = init
                if raw in ("true", "false"):
                    cpp_type = "bool"
                elif raw.startswith('"') or raw.startswith("'"):
                    cpp_type = "std::string"
                elif "." in raw:
                    cpp_type = "double"
                elif raw.isdigit() or (raw.startswith("-") and raw[1:].isdigit()):
                    cpp_type = "int"
                else:
                    cpp_type = "auto"
                state_decls.append({"signal_name": sig, "type": cpp_type})
            header = generate_state_header(
                windows, state_decls, [f for w in windows for f in w.premain_functions])
            header_path = os.path.join(
                os.path.dirname(out_path) or ".", _STATE_HEADER_NAME)
            with open(header_path, "w") as f:
                f.write(header)

        code = _emit_logic(windows)
        with open(out_path, "w") as f:
            f.write(code)
