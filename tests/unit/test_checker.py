"""Tests for the .mx linter (morph.checker)."""

import os
import sys
from pathlib import Path

import pytest

from morph.parser.morph_parser import MorphParser
from morph.parser.jsx_walker import JSXWalker
from morph.checker.checker import Checker
from morph.config.schema import MorphConfig, LintConfig


def lint(source: str, name: str = "test.mx", config=None) -> list:
    ast = MorphParser().parse(source, name)
    walked = JSXWalker().walk(ast)
    return Checker().run(source, ast, walked, name, config)


def codes(errors: list) -> list[str]:
    return [e.code for e in errors]


def error_codes(errors: list) -> list[str]:
    return [e.code for e in errors if e.severity == "error"]


CLEAN = """
import { CSS, morphState, morphEffect } from 'morph'

CSS.load("./style.css")

export const windowConfig = { title: "App", width: 800, height: 600 }

export default function App() {
  const [count, setCount] = morphState(0)
  morphEffect(() => { console.log(count) }, [count])

  return (
    <>
      <body>
        <div className="nav" id="top" onClick={() => setCount(count + 1)}>
          {count > 1 && <span>Big</span>}
          {items.map(item => <li key={item.id}>{item.name}</li>)}
          <img src="a.png" alt="x" width={100} height={50} />
          <a href="#" target="_blank">Link</a>
        </div>
      </body>
    </>
  )
}
"""


def test_clean_file_has_no_errors(tmp_path):
    (tmp_path / "style.css").write_text(".nav { color: red }\n")
    errs = lint(CLEAN.replace("./style.css", str(tmp_path / "style.css")))
    assert error_codes(errs) == []


def test_unknown_element():
    src = CLEAN.replace("<img src=", "<imgg src=")
    errs = lint(src)
    assert "mx-tag" in error_codes(errs)


def test_unknown_prop_with_suggestion():
    src = CLEAN.replace('className="nav"', 'classNam="nav"')
    errs = lint(src)
    e = next(e for e in errs if e.code == "mx-prop")
    assert "classNam" in e.message
    assert "Did you mean 'className'" in (e.suggestion or "")


def test_img_requires_src():
    src = CLEAN.replace('<img src="a.png" alt="x" width={100} height={50} />',
                        '<img alt="x" />')
    errs = lint(src)
    assert "mx-img-src" in error_codes(errs)


def test_event_value_must_be_function():
    src = CLEAN.replace('onClick={() => setCount(count + 1)}', 'onClick={5}')
    errs = lint(src)
    assert "mx-event-value" in error_codes(errs)


def test_morph_action_needs_static_string():
    src = CLEAN.replace('<a href="#" target="_blank">Link</a>',
                        '<a href="#" morph-navigate={url}>Link</a>')
    errs = lint(src)
    assert "mx-morph-action" in error_codes(errs)


def test_key_misuse_warns_outside_map():
    src = CLEAN.replace('<div className="nav" id="top"',
                        '<div className="nav" id="top" key="k"')
    errs = lint(src)
    assert "mx-key-misuse" in codes(errs)


def test_key_on_map_root_is_fine():
    errs = lint(CLEAN)
    assert "mx-key-misuse" not in codes(errs)


def test_list_without_key_warns():
    src = CLEAN.replace("key={item.id}", "")
    errs = lint(src)
    assert "mx-list-key" in codes(errs)


def test_unsupported_inline_style_prop():
    src = CLEAN.replace('className="nav"', 'style={{ backgrounColor: "#fff" }}')
    errs = lint(src)
    e = next(e for e in errs if e.code == "mx-style-prop")
    assert "backgroun-color" in e.message  # camelCase → kebab
    assert "Did you mean 'background-color'" in (e.suggestion or "")


def test_invalid_css_value_warns():
    src = CLEAN.replace('className="nav"', 'style={{ display: "bogus" }}')
    errs = lint(src)
    assert "mx-style-value" in codes(errs)


def test_state_pattern():
    src = CLEAN.replace("const [count, setCount] = morphState(0)",
                        "const count = morphState(0)")
    errs = lint(src)
    assert "mx-state-pattern" in error_codes(errs)


def test_effect_callback_must_be_function():
    src = CLEAN.replace("morphEffect(() => { console.log(count) }, [count])",
                        "morphEffect(count, [count])")
    errs = lint(src)
    assert "mx-effect-cb" in error_codes(errs)


def test_effect_deps_must_be_state():
    src = CLEAN.replace("[count]", "[otherVar]")
    errs = lint(src)
    assert "mx-effect-deps" in codes(errs)


def test_unknown_import_from_morph():
    src = CLEAN.replace("morphState, morphEffect", "morphState, morphEffect, noSuch")
    errs = lint(src)
    assert "mx-import-morph" in error_codes(errs)


def test_missing_css_file():
    errs = lint(CLEAN)  # ./style.css does not exist here
    assert "mx-css-file-missing" in error_codes(errs)


def test_css_file_unknown_property(tmp_path):
    css = tmp_path / "style.css"
    css.write_text(".nav { letter-spacing: 1px }\n")
    src = CLEAN.replace("./style.css", str(css))
    errs = lint(src)
    assert "mx-css-prop" in codes(errs)
    assert errs[0].file_path == str(css)


def test_missing_window_errors():
    src = CLEAN.replace("export const windowConfig = { title: \"App\", width: 800, height: 600 }",
                        "").replace("<>", "").replace("</>", "")
    errs = lint(src)
    assert "mx-window-missing" in error_codes(errs)


def test_window_config_type_errors():
    src = CLEAN.replace('title: "App"', 'title: 42').replace("width: 800", 'width: "w"')
    errs = lint(src)
    assert "mx-windowconfig-type" in error_codes(errs)


def test_window_config_unknown_key():
    src = CLEAN.replace('height: 600', 'height: 600, bogus: 1')
    errs = lint(src)
    assert "mx-windowconfig-key" in error_codes(errs)


def test_window_conflict():
    src = CLEAN.replace("<>", "<morph-window>").replace("</>", "</morph-window>")
    errs = lint(src)
    assert "mx-window-conflict" in error_codes(errs)
    src2 = CLEAN.replace("export const windowConfig = { title: \"App\", width: 800, height: 600 }", "")
    errs2 = lint(src2)
    assert "mx-window-conflict" not in codes(errs2)


def test_morph_window_prop_type():
    src = CLEAN.replace("export const windowConfig = { title: \"App\", width: 800, height: 600 }",
                        "").replace("<>", '<morph-window width="big">').replace("</>", "</morph-window>")
    errs = lint(src)
    assert "mx-window-prop-type" in error_codes(errs)


def test_stub_tags_warn():
    src = CLEAN.replace('<img src="a.png" alt="x" width={100} height={50} />',
                        "<input />")
    errs = lint(src)
    assert "mx-tag-stub" in codes(errs)


def test_component_imports_warn():
    src = "import { Foo } from './Foo.mx'\n" + CLEAN
    errs = lint(src)
    assert "mx-import-type" in codes(errs)


def test_no_default_export_errors():
    src = CLEAN.replace("export default function App() {",
                        "function App() {")
    errs = lint(src)
    assert "mx-export" in error_codes(errs)


def test_multiple_default_exports_error():
    src = CLEAN + "\n\nexport default function Second() { return (<div/>) }\n"
    errs = lint(src)
    assert "mx-export" in error_codes(errs)


def test_transpile_failure_errors(monkeypatch):
    # Simulate a JS construct the C++ translator cannot compile.
    import morph.checker.checker as checker_mod
    def _boom(src, state_map):
        return "boom: unsupported syntax"
    monkeypatch.setattr(checker_mod, "_transpile_expr", _boom)
    errs = lint(CLEAN)
    assert "mx-transpile" in error_codes(errs)
    e = next(e for e in errs if e.code == "mx-transpile")
    assert "boom" in e.message


def test_config_disable_and_severity():
    cfg = MorphConfig()
    cfg.lint = LintConfig(disable=["mx-list-key"],
                          severities={"mx-img-src": "warning"})
    src = CLEAN.replace('<img src="a.png" alt="x" width={100} height={50} />',
                        "<img alt=\"x\" />")
    src = src.replace("key={item.id}", "")
    errs = lint(src, config=cfg)
    assert "mx-list-key" not in codes(errs)
    e = next(e for e in errs if e.code == "mx-img-src")
    assert e.severity == "warning"


def test_positions_are_1_based():
    errs = lint(CLEAN.replace("<imgg ", "<div ") .replace("<img src=", "<imgg src="))
    e = next(e for e in errs if e.code == "mx-tag")
    assert e.line >= 1 and e.col >= 1
    assert e.source_lines is not None


def test_tailwind_unknown_class_warns(tmp_path, monkeypatch):
    css = tmp_path / "style.css"
    css.write_text(".nav { color: red }\n")
    src = CLEAN.replace("./style.css", str(css)).replace(
        'className="nav"', 'className="nav definitely-not-a-class"')
    errs = lint(src)
    assert "mx-tailwind-class" in codes(errs)


def test_tailwind_custom_css_class_not_warned(tmp_path):
    css = tmp_path / "style.css"
    css.write_text(".nav { color: red }\n")
    src = CLEAN.replace("./style.css", str(css))
    errs = lint(src)
    assert "mx-tailwind-class" not in codes(errs)


def test_dup_class_warns():
    src = CLEAN.replace('className="nav"', 'className="nav" class="alt"')
    errs = lint(src)
    assert "mx-dup-class" in codes(errs)


# ── mx-transpile: whole-statement codegen paths ──────────────────────


def test_transpile_component_const_errors():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const o = { get x() { return 1 } }\n  const [count, setCount] = morphState(0)")
    errs = lint(src)
    transpile = [e for e in errs if e.code == "mx-transpile"]
    assert transpile, [e.code for e in errs]
    assert any("TSMethodDefinition" in e.message for e in transpile)


def test_transpile_top_level_function_errors():
    src = CLEAN + (
        "\nfunction makeCounter() {\n"
        "  const obj = { get value() { return 5 } };\n  return obj;\n}\n")
    errs = lint(src)
    transpile = [e for e in errs if e.code == "mx-transpile"]
    assert transpile
    assert any("TSMethodDefinition" in e.message for e in transpile)


def test_transpile_global_var_errors():
    src = "const BASE = { get x() { return 1 } };\n" + CLEAN
    errs = lint(src)
    assert "mx-transpile" in codes(errs)


def test_transpile_inner_function_errors():
    src = CLEAN.replace(
        "return (", "function inner() { const o = { get v() { return 2 } }; return o; }\n  return (")
    errs = lint(src)
    transpile = [e for e in errs if e.code == "mx-transpile"]
    assert transpile


def test_transpile_effect_callback_errors():
    src = CLEAN.replace(
        "morphEffect(() => { console.log(count) }, [count])",
        "morphEffect(() => { const o = { get x() { return 1 } }; }, [])")
    errs = lint(src)
    transpile = [e for e in errs if e.code == "mx-transpile"]
    assert transpile
    assert any("TSMethodDefinition" in e.message for e in transpile)


def test_transpile_healthy_code_stays_clean():
    src = CLEAN + (
        "\nfunction compute(a: double, b: double, k: int): double {\n"
        "  if (k === 1) return a + b;\n  return b;\n}\n"
        "const API = \"http://localhost\";\n")
    errs = lint(src)
    assert "mx-transpile" not in codes(errs)


# ── mx-js-*: static TSX→C++ translator checks ────────────────────────


def test_js_global_flags_browser_and_js_globals():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const [count, setCount] = morphState(0)\n  "
        "const a = document.title\n  const b = alert('x')\n  const c = Math\n")
    errs = lint(src)
    globals_hit = [e for e in errs if e.code == "mx-js-global"]
    names = {e.message.split("'")[1] for e in globals_hit}
    assert {"document", "alert", "Math"} <= names


def test_js_global_skips_declared_and_supported():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const window = 5\n  const [count, setCount] = morphState(0)\n  "
        "const t = setTimeout(() => {}, 100)\n  const f = fetch('/x')")
    errs = lint(src)
    assert "mx-js-global" not in codes(errs)


def test_js_member_flags_math_json_date_console():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const [count, setCount] = morphState(0)\n  "
        "const a = Math.floor(1.5)\n  const b = JSON.parse(s)\n  "
        "const c = Date.now()\n  console.assert(count)")
    errs = lint(src)
    hits = [e for e in errs if e.code == "mx-js-member"]
    assert any("Math" in e.message for e in hits)
    assert any("JSON" in e.message for e in hits)
    assert any("console" in e.message for e in hits)
    assert any("Date" in e.message for e in hits)


def test_js_member_allows_console_log():
    src = CLEAN.replace(
        "morphEffect(() => { console.log(count) }, [count])",
        "morphEffect(() => { console.log('hi', count) }, [count])")
    errs = lint(src)
    assert "mx-js-member" not in codes(errs)


def test_js_method_flags_state_and_string_literal_methods():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const [count, setCount] = morphState(0)\n  "
        "const [items, setItems] = morphState([1, 2])\n  "
        "const a = items.map(i => i * 2)\n  const b = \"a,b\".split(',')\n  "
        "const c = \"abc\".toUpperCase()")
    errs = lint(src)
    hits = [e for e in errs if e.code == "mx-js-method"]
    assert any("map" in e.message and "state" in e.message for e in hits)
    assert any("split" in e.message for e in hits)
    assert any("toUpperCase" in e.message for e in hits)


def test_js_op_flags_unsupported_operators():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const [count, setCount] = morphState(0)\n  "
        "const a = typeof count\n  const b = count ** 2\n  "
        "const c = count instanceof Number\n  const d = 'x' in obj")
    errs = lint(src)
    ops = {e.message.split("'")[1] for e in errs if e.code == "mx-js-op"}
    assert {"typeof", "**", "instanceof", "in"} <= ops


def test_js_syntax_flags_unsupported_constructs():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const [count, setCount] = morphState(0)\n  "
        "const h = a?.b\n  const { x } = obj\n  "
        "function* gen() { yield 1 }\n  for (const k in obj) { }\n  "
        "const sp = { ...obj }\n  f(...args)\n  "
        "const o = { get v() { return 1 } }")
    errs = lint(src)
    synt = [e for e in errs if e.code == "mx-js-syntax"]
    messages = " ".join(e.message for e in synt)
    for kw in ("optional chaining", "destructuring", "generator", "yield",
               "for...in", "object spread", "spread in call", "methods/getters"):
        assert kw in messages, kw


def test_js_syntax_allows_morphstate_destructuring():
    src = CLEAN  # uses const [count, setCount] = morphState(0)
    errs = lint(src)
    assert "mx-js-syntax" not in codes(errs)


def test_js_syntax_allows_array_spread():
    src = CLEAN.replace(
        "const [count, setCount] = morphState(0)",
        "const [count, setCount] = morphState(0)\n  "
        "const xs = [1, ...[2, 3]]")
    errs = lint(src)
    assert "mx-js-syntax" not in codes(errs)