from __future__ import annotations
import hashlib
import time
import os
from pathlib import Path

from morph.parser.morph_parser import MorphParser, collect_errors
from morph.parser.jsx_walker import JSXWalker
from morph.parser.errors import MorphParseError
from morph.style.css_parser import CSSParser
from morph.style.css_fetcher import CSSFetcher
from morph.style.tailwind import TailwindResolver
from morph.ir.builder import IRBuilder
from morph.ir.serializer import IRSerializer, _clean_inf
from morph.ir.node import IRWindow
from morph.layout.engine import LayoutEngine
from morph.codegen.emitter import Emitter
from morph.build.compiler import Compiler
from morph.build.platform import shared_lib_ext
from morph.utils.logger import log_error, log_parse_error, log_dim, log_warn, log_success, log_lint_errors

_tw_resolver: TailwindResolver | None = None
_fetcher = CSSFetcher()
_css_parser = CSSParser()

# Latest IR windows (used by compile_logic for codegen)
_latest_windows: list[IRWindow] | None = None

# Last build error (for surfacing in the in-app DevTools)
_last_error: str | None = None


def get_latest_windows() -> list[IRWindow] | None:
    return _latest_windows


def last_error() -> str | None:
    return _last_error


def _fmt_error(e: MorphParseError) -> str:
    loc = ""
    if e.file_path:
        loc = f"{e.file_path}"
        if e.line:
            loc += f":{e.line}:{e.col}"
        loc += ": "
    return f"{loc}{e.args[0] if e.args else 'Parse error'}"


def _fmt(secs: float) -> str:
    if secs < 0.5:
        return f"{secs*1000:.1f}ms"
    return f"{secs:.2f}s"


def run(config, verbose: bool = True) -> dict | None:
    global _tw_resolver, _latest_windows, _last_error
    if _tw_resolver is None:
        _tw_resolver = TailwindResolver(project_root=".")

    ts = time.time()
    _last_error = None

    try:
        source = Path(config.entry).read_text(encoding="utf-8")
        source_lines = source.split("\n")

        t = time.time()
        ast = MorphParser().parse(source, file_path=str(config.entry))
        if verbose:
            log_dim(f"parsed  {Path(config.entry).name}  in {_fmt(time.time() - t)}")

        t = time.time()
        for err_node in collect_errors(ast):
            if err_node.is_missing:
                raise MorphParseError(
                    "Missing required syntax element",
                    file_path=str(config.entry),
                    line=err_node.start_point[0] + 1,
                    col=err_node.start_point[1] + 1,
                    source_lines=source_lines,
                )
            if err_node.type == "ERROR":
                text = err_node.text.decode()
                if "<" in text or "{" in text or "}" in text:
                    raise MorphParseError(
                        text.strip(),
                        file_path=str(config.entry),
                        line=err_node.start_point[0] + 1,
                        col=err_node.start_point[1] + 1,
                        source_lines=source_lines,
                    )
        if verbose:
            log_dim(f"validated  in {_fmt(time.time() - t)}")

        t = time.time()
        walked = JSXWalker().walk(ast)
        if verbose:
            log_dim(f"walked  AST  in {_fmt(time.time() - t)}")

        # ── Lint (semantic rules) ───────────────────────────
        # Errors block the reload/build (Next.js-style: dev keeps watching,
        # build fails); warnings are reported and ignored.
        t = time.time()
        from morph.checker.checker import Checker
        lint_errors = Checker().run(source, ast, walked,
                                    file_path=str(config.entry),
                                    config=config)
        lint_fails = [e for e in lint_errors if e.severity == "error"]
        if lint_fails:
            log_lint_errors(lint_errors)
            e0 = lint_fails[0]
            loc = ""
            if e0.file_path:
                loc = f"{e0.file_path}"
                if e0.line:
                    loc += f":{e0.line}:{e0.col}"
                loc += ": "
            _last_error = (f"{loc}{len(lint_fails)} lint error(s) — "
                           "fix and save to hot reload")
            return None
        if lint_errors:
            log_lint_errors(lint_errors)
        if verbose:
            log_dim(f"linted  in {_fmt(time.time() - t)}")

        t = time.time()
        css_rules: dict = {}
        keyframes: dict = {}
        for imp in walked.get("imports", []):
            if imp["type"] == "css_local":
                path = Path(config.entry).parent / imp["path"]
                if path.exists():
                    rules, kfs = _css_parser.parse_sheet(path.read_text(encoding="utf-8"))
                    css_rules.update(rules)
                    for name, kf_list in kfs.items():
                        keyframes.setdefault(name, []).extend(kf_list)
                else:
                    log_warn(f"CSS file not found: {path} (imported from {config.entry})")
            elif imp["type"] == "css_url":
                css_text = _fetcher.fetch(imp["url"])
                if css_text:
                    rules, kfs = _css_parser.parse_sheet(css_text)
                    css_rules.update(rules)
                    for name, kf_list in kfs.items():
                        keyframes.setdefault(name, []).extend(kf_list)
        if verbose:
            log_dim(f"resolved  CSS  in {_fmt(time.time() - t)}")

        t = time.time()
        ir = IRBuilder(config).build(walked, css_rules, _tw_resolver, keyframes)
        if verbose:
            log_dim(f"built  IR  in {_fmt(time.time() - t)}")

        t = time.time()
        LayoutEngine().compute(ir)
        if verbose:
            log_dim(f"laid  out  in {_fmt(time.time() - t)}")

        _latest_windows = ir

        t = time.time()
        result = _clean_inf(IRSerializer().to_dict(ir))
        if verbose:
            log_dim(f"serialized  in {_fmt(time.time() - t)}")
            log_dim(f"total  {_fmt(time.time() - ts)}")
        return result

    except MorphParseError as e:
        _last_error = _fmt_error(e)
        log_parse_error(e)
        return None
    except Exception as e:
        _last_error = f"{type(e).__name__}: {e}"
        log_error(_last_error)
        return None


LOGIC_CACHE_DIR = os.path.abspath(".morph/cache")
LOGIC_SOURCE_PATH = os.path.join(LOGIC_CACHE_DIR, "app_logic.cpp")

# Precompiled-header superset: every header the logic emitter may emit
# (fixed preheader + conditional widget includes + common extras). Headers
# missing from a given runtime build are skipped via __has_include, and the
# source's own #include lines become no-ops through include guards.
_PCH_HEADER_CONTENT = r"""// Auto-generated by morph dev — do not edit.
// Precompiled once per runtime-headers change; reused by every hot reload.
#include <cstdio>
#include <string>
#if __has_include("logic_prelude.h")
#include "logic_prelude.h"
#endif
#if __has_include("core/node.h")
#include "core/node.h"
#endif
#if __has_include("reactivity/signal.h")
#include "reactivity/signal.h"
#endif
#if __has_include("reactivity/task.h")
#include "reactivity/task.h"
#endif
#if __has_include("core/event.h")
#include "core/event.h"
#endif
#if __has_include("types/js_value.h")
#include "types/js_value.h"
#endif
#if __has_include("types/js_types.h")
#include "types/js_types.h"
#endif
#if __has_include("ui/image.h")
#include "ui/image.h"
#endif
#if __has_include("ui/morph_list.h")
#include "ui/morph_list.h"
#endif
#if __has_include("ui/rect.h")
#include "ui/rect.h"
#endif
#if __has_include("ui/text.h")
#include "ui/text.h"
#endif
#if __has_include("ui/button.h")
#include "ui/button.h"
#endif
#if __has_include("ui/input.h")
#include "ui/input.h"
#endif
#if __has_include("signal_store.h")
#include "signal_store.h"
#endif
#if __has_include("node_registry.h")
#include "node_registry.h"
#endif
"""


_last_logic_hash: str | None = None
_last_logic_so_path: str | None = None
_dev_cxx_noted: bool = False


def _resolve_dev_cxx(config) -> str:
    """Pick the hot-reload logic compiler.

    Order: morph.config.json build.dev_cxx → MORPH_DEV_CXX env → platform
    default (g++). NOTE: clang is NOT auto-preferred — benchmarked on this
    toolchain (GCC 14 / libstdc++, template-heavy runtime headers), g++
    parses our header chain measurably faster (~3.5s vs ~4.4s for login).
    Users on other toolchains can opt in via build.dev_cxx or MORPH_DEV_CXX;
    `find_clang()` in morph.build.compiler locates/probes a usable clang++.
    """
    global _dev_cxx_noted
    from morph.build.platform import pick_cpp

    def _note_once(msg: str) -> None:
        global _dev_cxx_noted
        if not _dev_cxx_noted:
            log_dim(msg)
        _dev_cxx_noted = True

    configured = ""
    if config is not None:
        configured = getattr(getattr(config, "build", None), "dev_cxx", "") or ""
    configured = configured or os.environ.get("MORPH_DEV_CXX", "")
    if configured:
        _note_once(f"hot-reload compiler (configured): {configured}")
        return configured

    default = pick_cpp()
    _note_once(f"hot-reload compiler: {default} "
               "(set build.dev_cxx to override)")
    return default


def get_logic_so_path() -> str | None:
    return _last_logic_so_path


def _ensure_logic_pch(compiler: Compiler, native=None) -> str | None:
    """Build (once) and return the dev logic precompiled-header path.

    The PCH captures every heavy runtime header the generated logic may
    include, compiled with the exact same flag set as the logic .so, so hot
    reloads only parse the app logic itself. Keyed by the full flag list +
    runtime header contents; any runtime change rebuilds it automatically.
    """
    import hashlib as _hashlib

    flags_str = " ".join(compiler._dev_shared_flags(None, native))
    rt_hash = _runtime_headers_hash()
    key = _hashlib.sha256((flags_str + rt_hash).encode()).hexdigest()[:16]

    pch_dir = os.path.join(LOGIC_CACHE_DIR, "pch", key)
    header = os.path.join(pch_dir, "logic_pch.h")
    try:
        os.makedirs(pch_dir, exist_ok=True)
        changed = True
        if os.path.exists(header):
            try:
                with open(header, "r") as f:
                    changed = f.read() != _PCH_HEADER_CONTENT
            except OSError:
                changed = True
        if changed:
            with open(header, "w") as f:
                f.write(_PCH_HEADER_CONTENT)

        base, _ = os.path.splitext(header)
        is_clang = "clang" in os.path.basename(compiler.gpp)
        pch_file = base + (".pch" if is_clang else ".h.gch")
        if (not os.path.exists(pch_file)
                or os.path.getmtime(pch_file) < os.path.getmtime(header)):
            if not compiler.compile_pch(header, pch_file, native=native):
                return None

        # Keep only the two most recent PCH directories.
        pch_root = os.path.join(LOGIC_CACHE_DIR, "pch")
        try:
            dirs = sorted(
                (os.path.getmtime(os.path.join(pch_root, d)), d)
                for d in os.listdir(pch_root)
                if os.path.isdir(os.path.join(pch_root, d)))
            for _, d in dirs[:-2]:
                import shutil as _shutil
                _shutil.rmtree(os.path.join(pch_root, d), ignore_errors=True)
        except OSError:
            pass
        return header
    except OSError as e:
        log_warn(f"PCH unavailable, compiling without it: {e}")
        return None


def _runtime_headers_hash() -> str:
    """Hash of the runtime C++ headers the logic .so is compiled against.

    The generated logic source calls into MorphNode/MorphStyle, so any
    change to the runtime layout (e.g. a new style field) invalidates
    previously compiled logic libraries — reusing them would read stale
    member offsets and crash.
    """
    from morph.dev import devrt
    return devrt._compute_source_hash()


def compile_logic(windows: list[IRWindow], verbose: bool = True,
                  config=None) -> bool:
    """Generate and compile the JS logic to a native shared library."""
    global _last_logic_hash, _last_logic_so_path, _last_error
    try:
        os.makedirs(LOGIC_CACHE_DIR, exist_ok=True)

        emitter = Emitter()
        t = time.time()
        emitter.emit_logic(windows, LOGIC_SOURCE_PATH)
        if verbose:
            log_dim(f"generated logic  in {_fmt(time.time() - t)}")

        # Skip compilation if source unchanged.  The hash covers the
        # generated logic source, any imported user .cpp files, and the
        # runtime headers it compiles against, so a change anywhere forces
        # a rebuild (and a fresh .so for dlopen hot-reload).
        with open(LOGIC_SOURCE_PATH, "rb") as f:
            logic_hash = hashlib.sha256(f.read()).hexdigest()
        from morph.codegen.native_header import collect_cpp_imports
        for ci in collect_cpp_imports(windows):
            path = ci.get("path", "")
            if not path or not os.path.exists(path):
                continue
            try:
                with open(path, "rb") as f:
                    logic_hash = hashlib.sha256(
                        (logic_hash + f.read().hex()).encode()).hexdigest()
            except OSError:
                pass
        cur_hash = hashlib.sha256(
            (logic_hash + _runtime_headers_hash()).encode()).hexdigest()
        if cur_hash == _last_logic_hash:
            if verbose:
                log_dim("logic source unchanged — skipping compilation")
            return True

        # Use a content-hash-based unique filename so loading always gets a fresh file
        ext = shared_lib_ext()
        so_filename = f"logic.{cur_hash[:16]}{ext}"
        so_path = os.path.join(LOGIC_CACHE_DIR, so_filename)

        # Only compile if the target file doesn't already exist
        if os.path.exists(so_path):
            _last_logic_hash = cur_hash
            _last_logic_so_path = so_path
            if verbose:
                log_dim(f"logic already compiled — reusing")
            return True

        compiler = Compiler(cxx=_resolve_dev_cxx(config))
        compiler.silent = True
        t = time.time()
        # Compile to a temp file, then atomically rename to the final name.
        # NOTE: GCC precompiled headers were evaluated here and are a net
        # LOSS for our header set (~138MB .gch; restore cost ≈ parse cost,
        # plus an 8s first-reload penalty) — logic compiles deliberately run
        # without one. The extern-template prelude (logic_prelude.h) is what
        # actually cuts per-reload work.
        so_tmp = so_path + ".tmp." + str(os.getpid())
        native_cfg = getattr(config, "native", None) if config else None
        ok = compiler.compile_shared(LOGIC_SOURCE_PATH, so_tmp,
                                     native=native_cfg)
        if ok:
            os.rename(so_tmp, so_path)
            # Clean up old logic libraries (keep last 3)
            _cleanup_old_sos(LOGIC_CACHE_DIR, so_filename, keep=3)
            _last_logic_hash = cur_hash
            _last_logic_so_path = so_path
            if verbose:
                log_dim(f"compiled logic  in {_fmt(time.time() - t)}")
            return True
        _last_logic_hash = None
        _last_logic_so_path = None
        _last_error = "Logic compilation failed"
        log_error("Logic compilation failed")
        if os.path.exists(so_tmp):
            os.remove(so_tmp)
        return False
    except Exception as e:
        _last_logic_hash = None
        _last_logic_so_path = None
        _last_error = f"Logic compilation error: {e}"
        log_error(f"Logic compilation error: {e}")
        return False


def _cleanup_old_sos(cache_dir: str, keep_name: str, keep: int = 3) -> None:
    """Remove old logic.<hash>.<ext> files, keeping the `keep` most recent."""
    import re
    ext = shared_lib_ext()
    pattern = re.compile(rf"logic\.([a-f0-9]+){re.escape(ext)}$")
    sos = []
    for f in os.listdir(cache_dir):
        m = pattern.match(f)
        if m and f != keep_name:
            path = os.path.join(cache_dir, f)
            sos.append((os.path.getmtime(path), path))
    sos.sort(reverse=True)  # newest first
    for _, path in sos[keep:]:
        try:
            os.remove(path)
        except OSError:
            pass
