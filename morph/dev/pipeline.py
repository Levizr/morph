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
from morph.utils.logger import log_error, log_parse_error, log_dim, log_warn, log_success

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

        t = time.time()
        css_rules: dict = {}
        for imp in walked.get("imports", []):
            if imp["type"] == "css_local":
                path = Path(config.entry).parent / imp["path"]
                if path.exists():
                    rules = _css_parser.parse_file(str(path))
                    css_rules.update(rules)
                else:
                    log_warn(f"CSS file not found: {path} (imported from {config.entry})")
            elif imp["type"] == "css_url":
                css_text = _fetcher.fetch(imp["url"])
                if css_text:
                    rules = _css_parser.parse_string(css_text)
                    css_rules.update(rules)
        if verbose:
            log_dim(f"resolved  CSS  in {_fmt(time.time() - t)}")

        t = time.time()
        ir = IRBuilder(config).build(walked, css_rules, _tw_resolver)
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


_last_logic_hash: str | None = None
_last_logic_so_path: str | None = None


def get_logic_so_path() -> str | None:
    return _last_logic_so_path


def _runtime_headers_hash() -> str:
    """Hash of the runtime C++ headers the logic .so is compiled against.

    The generated logic source calls into MorphNode/MorphStyle, so any
    change to the runtime layout (e.g. a new style field) invalidates
    previously compiled logic libraries — reusing them would read stale
    member offsets and crash.
    """
    from morph.dev import devrt
    return devrt._compute_source_hash()


def compile_logic(windows: list[IRWindow], verbose: bool = True) -> bool:
    """Generate and compile the JS logic to a native shared library."""
    global _last_logic_hash, _last_logic_so_path, _last_error
    try:
        os.makedirs(LOGIC_CACHE_DIR, exist_ok=True)

        emitter = Emitter()
        t = time.time()
        emitter.emit_logic(windows, LOGIC_SOURCE_PATH)
        if verbose:
            log_dim(f"generated logic  in {_fmt(time.time() - t)}")

        # Skip compilation if source unchanged.  The hash covers both the
        # generated logic source and the runtime headers it compiles
        # against, so runtime layout changes force a rebuild.
        with open(LOGIC_SOURCE_PATH, "rb") as f:
            logic_hash = hashlib.sha256(f.read()).hexdigest()
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

        compiler = Compiler()
        compiler.silent = True
        t = time.time()
        # Compile to a temp file, then atomically rename to the final name
        so_tmp = so_path + ".tmp." + str(os.getpid())
        ok = compiler.compile_shared(LOGIC_SOURCE_PATH, so_tmp)
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
