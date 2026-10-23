#!/usr/bin/env python3
"""Check/build parity sweep for common JS spellings.

For every case: write a fresh fixture, run `morph check`. If check is clean
(no errors, exit 0), run `morph build --no-upx`. A `BUILD-FAIL` line is a
check/build parity gap: check said the program was valid, but codegen or the
C++ compiler rejected it. Cases in `EXPECTED_REJECTIONS` must fail check with
the listed codes; any other check error is an `UNEXPECTED-CHECK-ERROR` gap —
supported syntax must not be rejected.

Usage (from the repo root, after `cargo build --bin morph`):

    python3 tests/runtime/check-parity.py [name-filter ...]

Fixtures are generated under the system temp directory (override with
MORPH_PARITY_DIR).
"""
import concurrent.futures as cf
import os
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
MORPH = os.path.join(ROOT, "target", "debug", "morph")
BASE = os.environ.get("MORPH_PARITY_DIR") or os.path.join(
    tempfile.gettempdir(), "morph-js-corpus"
)
ANSI = re.compile(r"\x1b\[[0-9;]*m")

CONFIG = (
    '{"name":"%s","entry":"src/App.mx","window":{"width":420,"height":320,'
    '"title":"%s"},"dependencies":{},"cpp_sources":[]}'
)
WIN = 'export const windowConfig = { title: "%s", width: 420, height: 320 }\n'


def app(body, imports=""):
    return WIN % "t" + imports + "\n" + textwrap.dedent(body)


CASES = {}

# Constructs check must reject before build. Every other case must pass
# check: a supported spelling regressing to a check error is a parity bug.
# Values are the error codes check must report.
EXPECTED_REJECTIONS = {
    "array_sort": {"mx-js-method"},
    "class_expr_const": {"mx-js-syntax"},
    "cpp_keyword_fn": {"mx-js-syntax"},
    "cpp_keyword_name": {"mx-js-syntax"},
    "date_now": {"mx-js-member"},
    "delete_op": {"mx-js-op"},
    "enum_decl": {"mx-js-syntax"},
    "filter_non_jsx": {"mx-js-method"},
    "find_non_jsx": {"mx-js-method"},
    "forEach_body": {"mx-js-method"},
    "instanceof_op": {"mx-js-member", "mx-js-op"},
    "map_non_jsx": {"mx-js-method"},
    "namespace_import": {"mx-js-syntax"},
    "object_method": {"mx-js-method"},
    "pow_op": {"mx-js-op"},
    "promise_static": {"mx-js-member"},
    "reduce_non_jsx": {"mx-js-method"},
    "regex_literal": {"mx-js-syntax"},
    "rest_param_helper": {"mx-js-syntax"},
    "some_every": {"mx-js-method"},
    "typeof_op": {"mx-js-op"},
}


def case(name, body, imports=""):
    CASES[name] = {"src/App.mx": app(body, imports)}


def module_case(name, files):
    CASES[name] = files


# ── component const forms ──
case(
    "arrow_const_handler",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [count, setCount] = morphState(0)
      const inc = () => setCount(count + 1)
      const twice = () => count * 2
      const add = (n: number) => count + n
      return (<body><text>{twice()} {add(2)}</text><button onClick={inc}>+</button></body>)
    }
    """,
)
case(
    "fn_expr_const_handler",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [count, setCount] = morphState(0)
      const inc = function() { setCount(count + 1) }
      const read = function() { return count }
      return (<body><text>{read()}</text><button onClick={inc}>+</button></body>)
    }
    """,
)
case(
    "generic_arrow_const",
    """
    export default function App() {
      const id = <T,>(x: T) => x
      return (<body><text>{id(2)}</text></body>)
    }
    """,
)
case(
    "arrow_const_block",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [count, setCount] = morphState(0)
      const inc = () => { setCount(count + 2) }
      return (<body><text>{count}</text><button onClick={inc}>+</button></body>)
    }
    """,
)
case(
    "object_const",
    """
    export default function App() {
      const user = { name: "ada", age: 3 }
      return (<body><text>{user.name}</text></body>)
    }
    """,
)
case(
    "object_nested",
    """
    export default function App() {
      const user = { name: "ada", address: { city: "paris" } }
      return (<body><text>{user.address.city}</text></body>)
    }
    """,
)
case(
    "arrow_returns_object",
    """
    export default function App() {
      const make = () => ({ name: "ada" })
      return (<body><text>{make().name}</text></body>)
    }
    """,
)
case(
    "object_shorthand",
    """
    export default function App() {
      const name = "ada"
      const user = { name }
      return (<body><text>{user.name}</text></body>)
    }
    """,
)
case(
    "object_method",
    """
    export default function App() {
      const user = { name: "ada", greet() { return "hi " + this.name } }
      return (<body><text>{user.greet()}</text></body>)
    }
    """,
)
case(
    "object_array_member",
    """
    export default function App() {
      const users = [{ name: "ada" }, { name: "bob" }]
      return (<body><text>{users[0].name}</text></body>)
    }
    """,
)
case(
    "object_array_handler",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [msg, setMsg] = morphState("")
      const users = [{ name: "ada" }]
      return (<body><button onClick={() => setMsg(users[0].name)}>{msg}</button></body>)
    }
    """,
)
case(
    "object_condition",
    """
    export default function App() {
      const user = { age: 3 }
      return (<body>{user.age > 2 ? <text>old</text> : <text>young</text>}</body>)
    }
    """,
)
case(
    "class_expr_const",
    """
    export default function App() {
      const C = class { greet() { return "hi" } }
      const c = new C()
      return (<body><text>{c.greet()}</text></body>)
    }
    """,
)
case(
    "cpp_keyword_name",
    """
    export default function App() {
      const double = 2
      return (<body><text>{double}</text></body>)
    }
    """,
)
case(
    "cpp_keyword_fn",
    """
    export default function App() {
      const double = () => 2
      return (<body><text>{double()}</text></body>)
    }
    """,
)

# ── statements in component bodies ──
case(
    "switch_body",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [n, setN] = morphState(0)
      let label = "zero"
      switch (n) {
        case 0: label = "zero"; break
        case 1: label = "one"; break
        default: label = "many"
      }
      return (<body><text>{label}</text></body>)
    }
    """,
)
case(
    "try_catch_body",
    """
    export default function App() {
      let label = "ok"
      try { throw new Error("x") } catch (e) { label = "caught" }
      return (<body><text>{label}</text></body>)
    }
    """,
)
case(
    "for_loop_body",
    """
    export default function App() {
      let total = 0
      for (let i = 0; i < 4; i++) { total = total + i }
      return (<body><text>{total}</text></body>)
    }
    """,
)
case(
    "while_loop_body",
    """
    export default function App() {
      let n = 0
      while (n < 3) { n = n + 1 }
      return (<body><text>{n}</text></body>)
    }
    """,
)
case(
    "for_of_body",
    """
    export default function App() {
      let total = 0
      for (const x of [1, 2, 3]) { total = total + x }
      return (<body><text>{total}</text></body>)
    }
    """,
)
case(
    "forEach_body",
    """
    export default function App() {
      let total = 0
      ;[1, 2, 3].forEach((x) => { total = total + x })
      return (<body><text>{total}</text></body>)
    }
    """,
)
case(
    "map_non_jsx",
    """
    export default function App() {
      const doubled = [1, 2, 3].map((x) => x * 2)
      return (<body><text>{doubled.length}</text></body>)
    }
    """,
)
case(
    "filter_non_jsx",
    """
    export default function App() {
      const evens = [1, 2, 3].filter((x) => x % 2 === 0)
      return (<body><text>{evens.length}</text></body>)
    }
    """,
)
case(
    "find_non_jsx",
    """
    export default function App() {
      const found = [1, 2, 3].find((x) => x > 1)
      return (<body><text>{found}</text></body>)
    }
    """,
)
case(
    "reduce_non_jsx",
    """
    export default function App() {
      const total = [1, 2, 3].reduce((a, b) => a + b, 0)
      return (<body><text>{total}</text></body>)
    }
    """,
)
case(
    "some_every",
    """
    export default function App() {
      const anyBig = [1, 2, 3].some((x) => x > 2)
      return (<body><text>{anyBig}</text></body>)
    }
    """,
)
case(
    "array_sort",
    """
    export default function App() {
      const xs = [3, 1, 2]
      xs.sort((a, b) => a - b)
      return (<body><text>{xs.join(",")}</text></body>)
    }
    """,
)

# ── state update forms ──
case(
    "functional_updater",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [count, setCount] = morphState(0)
      return (<body><button onClick={() => setCount((c) => c + 1)}>{count}</button></body>)
    }
    """,
)
case(
    "set_timeout",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [done, setDone] = morphState(false)
      return (<body><button onClick={() => setTimeout(() => setDone(true), 100)}>{done}</button></body>)
    }
    """,
)
case(
    "async_handler",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [msg, setMsg] = morphState("")
      const load = async () => { setMsg("loaded") }
      return (<body><button onClick={load}>{msg}</button></body>)
    }
    """,
)
case(
    "await_fetch",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [msg, setMsg] = morphState("")
      async function load() { const r = await fetch("http://127.0.0.1/"); setMsg(r.text()) }
      return (<body><button onClick={load}>{msg}</button></body>)
    }
    """,
)
case(
    "await_arrow_const",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [msg, setMsg] = morphState("")
      const load = async () => { const r = await fetch("http://127.0.0.1/"); setMsg(r.text()) }
      return (<body><button onClick={load}>{msg}</button></body>)
    }
    """,
)
case(
    "promise_static",
    """
    import { morphState } from 'morph'
    export default function App() {
      const p = Promise.resolve(1)
      return (<body><text>{p}</text></body>)
    }
    """,
)

# ── expressions ──
case(
    "json_stringify",
    """
    export default function App() {
      return (<body><text>{JSON.stringify({ a: 1 })}</text></body>)
    }
    """,
)
case(
    "math_max",
    """
    export default function App() {
      return (<body><text>{Math.max(1, 2)}</text></body>)
    }
    """,
)
case(
    "template_literal",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [n, setN] = morphState(2)
      return (<body><text>{`n=${n}`}</text></body>)
    }
    """,
)
case(
    "optional_chain",
    """
    export default function App() {
      const user = { name: "ada" }
      return (<body><text>{user?.name}</text></body>)
    }
    """,
)
case(
    "nullish",
    """
    export default function App() {
      const x = null
      return (<body><text>{x ?? "fallback"}</text></body>)
    }
    """,
)
case(
    "pow_op",
    """
    export default function App() {
      return (<body><text>{2 ** 3}</text></body>)
    }
    """,
)
case(
    "typeof_op",
    """
    export default function App() {
      return (<body><text>{typeof 1}</text></body>)
    }
    """,
)
case(
    "instanceof_op",
    """
    export default function App() {
      return (<body><text>{String(1) instanceof String}</text></body>)
    }
    """,
)
case(
    "delete_op",
    """
    export default function App() {
      const o = { a: 1, b: 2 }
      delete o.a
      return (<body><text>{Object.keys(o).length}</text></body>)
    }
    """,
)

# ── JSX forms ──
case(
    "map_jsx",
    """
    export default function App() {
      const items = ["a", "b"]
      return (<body>{items.map((x) => <text>{x}</text>)}</body>)
    }
    """,
)
case(
    "map_jsx_key",
    """
    export default function App() {
      const items = ["a", "b"]
      return (<body>{items.map((x) => <text key={x}>{x}</text>)}</body>)
    }
    """,
)
case(
    "keys_map_jsx",
    """
    export default function App() {
      const obj = { a: 1, b: 2 }
      return (<body>{Object.keys(obj).map((k) => <text>{k}</text>)}</body>)
    }
    """,
)
case(
    "conditional_ternary_jsx",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [on, setOn] = morphState(true)
      return (<body>{on ? <text>yes</text> : <text>no</text>}</body>)
    }
    """,
)
case(
    "logical_and_jsx",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [on, setOn] = morphState(true)
      return (<body>{on && <text>yes</text>}</body>)
    }
    """,
)
case(
    "fragment",
    """
    export default function App() {
      return (<body><><text>a</text><text>b</text></></body>)
    }
    """,
)
case(
    "inline_style",
    """
    export default function App() {
      return (<body><text style={{ color: "red" }}>hi</text></body>)
    }
    """,
)
case(
    "style_const",
    """
    export default function App() {
      const st = { color: "red" }
      return (<body><text style={st}>hi</text></body>)
    }
    """,
)
case(
    "handler_param",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [msg, setMsg] = morphState("")
      return (<body><button onClick={(e) => setMsg("clicked")}>{msg}</button></body>)
    }
    """,
)

# ── helpers / params / types ──
case(
    "default_param_helper",
    """
    function greet(name: string = "there") { return "hi " + name }
    export default function App() {
      return (<body><text>{greet()}</text></body>)
    }
    """,
)
case(
    "default_param_comparison",
    """
    function pick(n: number = 1 < 2 ? 3 : 4, suffix: string = "x") { return suffix }
    export default function App() {
      return (<body><text>{pick()}</text></body>)
    }
    """,
)
case(
    "optional_param_helper",
    """
    function greet(name?: string) { return "hi" }
    export default function App() {
      return (<body><text>{greet()}</text></body>)
    }
    """,
)
case(
    "rest_param_helper",
    """
    function sum(...xs: number[]) { let t = 0; for (const x of xs) { t = t + x } return t }
    export default function App() {
      return (<body><text>{sum(1, 2)}</text></body>)
    }
    """,
)
case(
    "generic_helper",
    """
    function id<T>(x: T): T { return x }
    export default function App() {
      return (<body><text>{id("hi")}</text></body>)
    }
    """,
)
case(
    "enum_decl",
    """
    enum Color { Red, Green }
    export default function App() {
      return (<body><text>{Color.Red}</text></body>)
    }
    """,
)
case(
    "type_alias",
    """
    type User = { name: string }
    export default function App() {
      const u: User = { name: "ada" }
      return (<body><text>{u.name}</text></body>)
    }
    """,
)
case(
    "as_cast",
    """
    export default function App() {
      const x = 1
      return (<body><text>{(x as number) + 1}</text></body>)
    }
    """,
)
case(
    "non_null",
    """
    export default function App() {
      const o: { name?: string } = { name: "ada" }
      return (<body><text>{o.name!}</text></body>)
    }
    """,
)
case(
    "class_decl",
    """
    class Greeter {
      name: string
      constructor(name: string) { this.name = name }
      greet() { return "hi " + this.name }
    }
    export default function App() {
      const g = new Greeter("ada")
      return (<body><text>{g.greet()}</text></body>)
    }
    """,
)
case(
    "class_handler",
    """
    import { morphState } from 'morph'
    class Counter {
      n: number
      constructor(n: number) { this.n = n }
      inc() { this.n = this.n + 1 }
      get() { return this.n }
    }
    export default function App() {
      const [count, setCount] = morphState(0)
      const c = new Counter(1)
      return (<body><button onClick={() => { c.inc(); setCount(c.get()) }}>{count}</button></body>)
    }
    """,
)
case(
    "inner_default_param",
    """
    export default function App() {
      function greet(name: string = "there") { return "hi " + name }
      return (<body><text>{greet()}</text></body>)
    }
    """,
)
case(
    "inner_optional_param",
    """
    export default function App() {
      function greet(name?: string) { return "hi" }
      return (<body><text>{greet()}</text></body>)
    }
    """,
)
case(
    "module_calls_default",
    """
    function greet(name: string = "there") { return "hi " + name }
    function welcome() { return greet() }
    export default function App() {
      return (<body><text>{welcome()}</text></body>)
    }
    """,
)
module_case(
    "module_object_factory",
    {
        "src/helpers.ts": 'export function makeUser() { return { name: "ada" } }\n',
        "src/App.mx": app(
            """
            import { makeUser } from './helpers.ts'
            export default function App() {
              return (<body><text>{makeUser().name}</text></body>)
            }
            """
        ),
    },
)
case(
    "regex_literal",
    """
    export default function App() {
      const re = /ab+c/
      return (<body><text>{"abc".match(re) ? "yes" : "no"}</text></body>)
    }
    """,
)
case(
    "date_now",
    """
    export default function App() {
      return (<body><text>{Date.now() > 0 ? "yes" : "no"}</text></body>)
    }
    """,
)
case(
    "throw_error",
    """
    import { morphState } from 'morph'
    export default function App() {
      const [msg, setMsg] = morphState("")
      function boom() { if (msg === "") { throw new Error("empty") } }
      return (<body><button onClick={boom}>{msg}</button></body>)
    }
    """,
)
case(
    "console_log",
    """
    export default function App() {
      console.log("hello")
      return (<body><text>hi</text></body>)
    }
    """,
)

# ── imports / exports ──
module_case(
    "import_type",
    {
        "src/lib.ts": 'export type User = { name: string }\nexport const token = "x"\n',
        "src/App.mx": app(
            """
            import type { User } from './lib.ts'
            import { token } from './lib.ts'
            export default function App() {
              const u: User = { name: "ada" }
              return (<body><text>{u.name} {token}</text></body>)
            }
            """
        ),
    },
)
module_case(
    "export_type",
    {
        "src/lib.ts": "export type User = { name: string }\n",
        "src/App.mx": app(
            """
            import type { User } from './lib.ts'
            export default function App() {
              const u: User = { name: "ada" }
              return (<body><text>{u.name}</text></body>)
            }
            """
        ),
    },
)
module_case(
    "inline_type_import",
    {
        "src/lib.ts": 'export type User = { name: string }\nexport const token = "x"\n',
        "src/App.mx": app(
            """
            import { token, type User } from './lib.ts'
            export default function App() {
              const u: User = { name: "ada" }
              return (<body><text>{u.name} {token}</text></body>)
            }
            """
        ),
    },
)
module_case(
    "namespace_import",
    {
        "src/lib.ts": 'export const token = "x"\nexport function greet() { return "hi" }\n',
        "src/App.mx": app(
            """
            import * as lib from './lib.ts'
            export default function App() {
              return (<body><text>{lib.token} {lib.greet()}</text></body>)
            }
            """
        ),
    },
)


def run(cmd, cwd, timeout=900):
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout)
    return p.returncode, ANSI.sub("", p.stdout + p.stderr)


def prepare(name, files):
    root = os.path.join(BASE, name)
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(os.path.join(root, "src"), exist_ok=True)
    with open(os.path.join(root, "morph.config.json"), "w") as f:
        f.write(CONFIG % (name, name))
    for rel, content in files.items():
        path = os.path.join(root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as f:
            f.write(content)
    return root


def check_case(name):
    root = prepare(name, CASES[name])
    rc, out = run([MORPH, "check"], root)
    clean = rc == 0 and "All " in out and "passed." in out
    if clean:
        return (name, "check-clean", "", root)
    codes = set(re.findall(r"error : (mx-[a-z-]+)", out))
    expected = EXPECTED_REJECTIONS.get(name)
    if expected:
        missing = expected - codes
        if not missing:
            return (name, "rejected", ",".join(sorted(codes)), "")
        return (
            name,
            "WRONG-ERROR",
            f"want {','.join(sorted(expected))} got {','.join(sorted(codes)) or 'no code'}",
            "",
        )
    detail = ""
    if not codes:
        lines = [l.strip() for l in out.splitlines() if "error" in l.lower()]
        detail = lines[-1] if lines else "check failed"
    return (name, "UNEXPECTED-CHECK-ERROR", ",".join(sorted(codes)) or detail, "")


def build_case(result):
    name, status, codes, root = result
    if status != "check-clean":
        return result
    rc, out = run([MORPH, "build", "--no-upx"], root)
    if rc == 0:
        return (name, "built", codes, "")
    tail = "\n".join([l for l in out.splitlines() if "error" in l.lower()][-4:])
    return (name, "BUILD-FAIL", codes, tail)


def main():
    filters = sys.argv[1:]
    names = sorted(n for n in CASES if not filters or any(f in n for f in filters))
    print(f"{len(names)} cases", flush=True)
    results = []
    with cf.ThreadPoolExecutor(max_workers=8) as ex:
        for r in ex.map(check_case, names):
            results.append(r)
    with cf.ThreadPoolExecutor(max_workers=2) as ex:
        results = list(ex.map(build_case, results))
    print("\n===== SUMMARY =====")
    build_gaps = []
    check_gaps = []
    for name, status, codes, tail in results:
        print(f"{status:20} {name:28} {codes}")
        if status == "BUILD-FAIL":
            build_gaps.append((name, tail))
        elif status in ("UNEXPECTED-CHECK-ERROR", "WRONG-ERROR"):
            check_gaps.append((status, name, codes))
    print(f"\n{len(build_gaps)} check-pass/build-fail gaps")
    print(f"{len(check_gaps)} unexpected check rejections")
    for name, tail in build_gaps:
        print(f"\n--- {name} ---\n{tail}")
    for status, name, codes in check_gaps:
        print(f"\n--- {status}: {name} ---\n{codes}")
    return 1 if build_gaps or check_gaps else 0


if __name__ == "__main__":
    sys.exit(main())
