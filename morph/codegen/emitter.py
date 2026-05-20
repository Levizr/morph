import os
from jinja2 import Environment, FileSystemLoader
from morph.ir.node import IRWindow
from morph.codegen.feature_set import FeatureSet


TEMPLATES_DIR = os.path.join(os.path.dirname(__file__), "templates")


class Emitter:
    """Renders IR into app.cpp using Jinja2 templates."""

    def __init__(self):
        self.env = Environment(
            loader=FileSystemLoader(TEMPLATES_DIR),
            trim_blocks=True,
            lstrip_blocks=True,
        )

    def emit(self, windows: list[IRWindow], out_path: str,
             mode: str = "prod") -> None:
        features = FeatureSet()
        features.scan(windows)

        tmpl = self.env.get_template("app_main.cpp.j2")
        code = tmpl.render(
            windows=windows,
            headers=features.required_headers(),
            dev_mode=(mode == "dev"),
        )

        os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
        with open(out_path, "w") as f:
            f.write(code)
