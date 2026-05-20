"""Integration: HTML source → IR dict (no compile step)."""
from morph.dev.pipeline import run
from morph.config.schema import MorphConfig, WindowConfig
import os, pathlib, tempfile


def test_pipeline_with_empty_html():
    with tempfile.TemporaryDirectory() as tmp:
        src = pathlib.Path(tmp) / "src"
        src.mkdir()
        (src / "index.html").write_text("<morph-window></morph-window>")
        (src / "style.css").write_text("")
        (src / "app.js").write_text("")
        os.chdir(tmp)

        cfg = MorphConfig(
            entry="src/index.html",
            window=WindowConfig(),
        )
        result = run(cfg)
        # pipeline returns None until parsers are implemented — just no crash
        assert result is None or isinstance(result, dict)
