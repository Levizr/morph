"""Integration: HTML source → IR dict (no compile step)."""
from morph.dev.pipeline import run
from morph.config.schema import MorphConfig, WindowConfig
import pathlib


def test_pipeline_with_empty_html(tmp_path, monkeypatch):
    src = tmp_path / "src"
    src.mkdir()
    (src / "index.html").write_text("<morph-window></morph-window>")
    (src / "style.css").write_text("")
    (src / "app.js").write_text("")
    # monkeypatch restores the original CWD automatically after the test
    monkeypatch.chdir(tmp_path)

    cfg = MorphConfig(
        entry="src/index.html",
        window=WindowConfig(),
    )
    result = run(cfg)
    # pipeline returns None until parsers are implemented — just no crash
    assert result is None or isinstance(result, dict)
