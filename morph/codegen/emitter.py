from __future__ import annotations

import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRWindow
from morph.codegen.feature_set import FeatureSet
from morph.codegen.node_emitter import NodeEmitter

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

            for node in win.nodes:
                code = self.node_emitter.emit_node(node, var)
                if code:
                    win_code.append(code)
                    win_code.append("")

            window_code.append("\n".join(win_code))

        tmpl = self.env.get_template("app_main.cpp.j2")
        code = tmpl.render(
            windows=windows,
            window_code="\n".join(window_code),
            headers=features.required_headers(),
            dev_mode=(mode == "dev"),
        )

        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as f:
            f.write(code)
