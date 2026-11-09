"""Runtime tests for `new.target` (Part 2B).

JavaScript defines `new.target` only while a constructor invoked with `new`
is running: it names the most-derived constructor. Base constructors see the
subclass target, parameter defaults and arrows lexically nested in the
constructor inherit it, and field initializers, methods, and plain functions
see `undefined`.

Run:  pytest tests/translate/test_new_target.py -q
"""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
RUNTIME_CPP = REPO_ROOT / "runtime" / "cpp"
MORPH_BIN = REPO_ROOT / "target" / "debug" / "morph"


def _find_morph_bin() -> Path:
    if MORPH_BIN.exists():
        return MORPH_BIN
    found = shutil.which("morph")
    assert found, "morph binary not found, run `cargo build --bin morph` first"
    return Path(found)


def _find_compiler() -> str:
    compiler = shutil.which("g++-14") or shutil.which("g++")
    if not compiler:
        pytest.skip("g++ not found")
    return compiler


def _run_case(source: str, tmp_path: Path, name: str) -> str:
    ts_path = tmp_path / f"{name}.ts"
    ts_path.write_text(source, encoding="utf-8")
    result = subprocess.run(
        [str(_find_morph_bin()), str(ts_path), "--to", "cpp", "--types", "strict"],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert result.returncode == 0, f"morph failed: {result.stderr}"
    cpp_path = tmp_path / f"{name}.cpp"
    assert cpp_path.exists(), f"no output generated for {name}"
    cpp = cpp_path.read_text(encoding="utf-8")

    src = tmp_path / f"{name}.run.cpp"
    exe = tmp_path / name
    src.write_text(cpp, encoding="utf-8")
    build = subprocess.run(
        [
            _find_compiler(),
            "-std=c++23",
            f"-I{RUNTIME_CPP}",
            str(src),
            "-o",
            str(exe),
        ],
        capture_output=True,
        text=True,
        timeout=120,
    )
    assert build.returncode == 0, f"generated C++ does not compile:\n{build.stderr}\n--- cpp ---\n{cpp}"
    run = subprocess.run([str(exe)], capture_output=True, text=True, timeout=30)
    assert run.returncode == 0, f"program failed: {run.stderr}\n{run.stdout}"
    return run.stdout.strip()


CASES = {
    "constructor-self-and-subclass": (
        """
class Base {
    same = false
    constructor() {
        this.same = new.target === Base
    }
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
console.log(b.same, d.same)
""",
        "true false",
    ),
    "base-sees-derived-target": (
        """
class Base {
    base = false
    constructor() {
        this.base = new.target === Base
    }
}
class Derived extends Base {
    derived = false
    constructor() {
        super()
        this.derived = new.target === Derived
    }
}
const b = new Base()
const d = new Derived()
console.log(b.base, d.base, d.derived)
""",
        "true false true",
    ),
    "parameter-default-sees-target": (
        """
class Base {
    label: string
    constructor(label: string = new.target === Base ? "base" : "derived") {
        this.label = label
    }
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
console.log(b.label, d.label)
""",
        "base derived",
    ),
    "field-initializers-are-undefined": (
        """
class Base {
    instance = new.target === Base
    static shared = new.target === Base
    constructor() {}
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
console.log(b.instance, Base.shared, d.instance)
""",
        "false false false",
    ),
    "plain-function-is-undefined": (
        """
function plain(): boolean {
    return new.target === undefined
}
console.log(plain())
""",
        "true",
    ),
    "method-is-undefined": (
        """
class Base {
    method(): boolean {
        return new.target === undefined
    }
}
const b = new Base()
console.log(b.method())
""",
        "true",
    ),
    "arrow-in-constructor-inherits": (
        """
class Base {
    arrowSame = false
    constructor() {
        this.arrowSame = (() => new.target === Base)()
    }
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
console.log(b.arrowSame, d.arrowSame)
""",
        "true false",
    ),
    "stored-target-compares": (
        """
class Base {
    captured = false
    constructor() {
        const target = new.target
        if (target !== undefined) {
            this.captured = target === Base
        }
    }
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
console.log(b.captured, d.captured)
""",
        "true false",
    ),
    "target-is-truthy": (
        """
class Base {
    truthy = 0
    constructor() {
        const target = new.target
        if (target) {
            this.truthy = 1
        }
    }
}
const b = new Base()
console.log(b.truthy)
""",
        "1",
    ),
    "loose-equality": (
        """
class Base {
    loose = false
    constructor() {
        const target = new.target
        if (target !== undefined) {
            this.loose = target == Base
        }
    }
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
console.log(b.loose, d.loose)
""",
        "true false",
    ),
    "target-prints-class-name": (
        """
class Base {
    constructor() {
        console.log(new.target)
    }
}
class Derived extends Base {}
const b = new Base()
const d = new Derived()
""",
        "[class Base]\n[class Derived]",
    ),
}


@pytest.mark.parametrize("name", sorted(CASES))
def test_new_target(name: str, tmp_path: Path):
    source, expected = CASES[name]
    assert _run_case(source, tmp_path, name) == expected
