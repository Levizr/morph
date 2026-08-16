"""CSS `transform` parsing and 4x4 matrix composition.

Supports the full CSS transform list grammar: `none`, `matrix`,
`matrix3d`, `perspective`, `rotate`, `rotate3d`, `rotateX/Y/Z`,
`translate`, `translate3d`, `translateX/Y/Z`, `scale`, `scale3d`,
`scaleX/Y/Z`, `skew`, `skewX`, `skewY`, chained lists, and the global
keywords (`inherit`, `initial`, `revert`, `revert-layer`, `unset` —
treated as `none`).

Representation
--------------
Parsed ops are a list of tuples, e.g.::

    [("translate", ((10.0, "px"), (None, "px"))),
     ("rotate", 45.0),
     ("scale", (1.5, 1.5))]

Length components are ``(value, unit)`` pairs where ``unit`` is ``"px"``
or ``"%"`` (``None`` value means 0).  Percentages are resolved against
the element's **own border-box size** at composition time, per the CSS
Transforms spec — that is why composition takes ``own_w`` / ``own_h``.

Matrices are stored **column-major** (16 floats) — the layout used by
OpenGL and by the CSS spec's own ``matrix3d`` notation.
"""

from __future__ import annotations

import math

# ── Global keywords — resolved as `none` (no transform) ────────
GLOBAL_KEYWORDS = frozenset({"inherit", "initial", "revert", "revert-layer", "unset"})

# Op name → number of length args that may carry a % (used by needs_layout)
_PCT_LENGTH_OPS = frozenset({"translate", "translate3d", "translatex", "translatey"})


def _is_angle(v: str) -> bool:
    return v.endswith(("deg", "rad", "turn", "grad"))


def _angle_to_deg(v: str) -> float | None:
    """Parse an angle token to degrees (float), or None if invalid."""
    s = v.strip().lower()
    if s.endswith("deg"):
        try:
            return float(s[:-3])
        except ValueError:
            return None
    # NOTE: check "grad" BEFORE "rad" — "100grad" ends with "rad".
    if s.endswith("grad"):
        try:
            return float(s[:-4]) * 0.9
        except ValueError:
            return None
    if s.endswith("rad"):
        try:
            return math.degrees(float(s[:-3]))
        except ValueError:
            return None
    if s.endswith("turn"):
        try:
            return float(s[:-4]) * 360.0
        except ValueError:
            return None
    try:
        return float(s)
    except ValueError:
        return None


def _length_to_component(v: str) -> tuple[float, str] | None:
    """Parse a length token to ``(value, unit)``; ``%`` stays unresolved."""
    s = v.strip().lower()
    if s.endswith("%"):
        try:
            return (float(s[:-1]), "%")
        except ValueError:
            return None
    if s.endswith("px"):
        try:
            return (float(s[:-2]), "px")
        except ValueError:
            return None
    try:
        return (float(s), "px")
    except ValueError:
        return None


def _split_args(inner: str) -> list[str]:
    """Split a function's argument list on commas or whitespace.

    Modern CSS allows both `translate(10px, 20px)` and `translate(10px 20px)`.
    """
    if "," in inner:
        return [p.strip() for p in inner.split(",") if p.strip()]
    return [p for p in inner.split() if p]


def parse_transform(value: str) -> list[tuple] | None:
    """Parse a CSS `transform` value into ops.

    Returns:
      - ``[]`` for `none` / global keywords (no transform),
      - a list of ops for valid transform lists,
      - ``None`` when the value is invalid (property must be ignored).
    """
    s = value.strip()
    if not s:
        return None
    low = s.lower()
    if low == "none" or low in GLOBAL_KEYWORDS:
        return []
    if low.startswith("none"):
        return None  # 'none' followed by garbage

    ops: list[tuple] = []
    i = 0
    n = len(s)
    while i < n:
        while i < n and s[i] in " \t\n\r":
            i += 1
        if i >= n:
            break
        start = i
        while i < n and (s[i].isalnum() or s[i] == "-"):
            i += 1
        name = s[start:i].lower()
        while i < n and s[i] in " \t\n\r":
            i += 1
        if i >= n or s[i] != "(":
            return None  # missing '(' — invalid
        depth = 1
        j = i + 1
        while j < n and depth > 0:
            if s[j] == "(":
                depth += 1
            elif s[j] == ")":
                depth -= 1
            j += 1
        if depth != 0:
            return None  # unbalanced parens
        inner = s[i + 1:j - 1]
        args = _split_args(inner)
        op = _build_op(name, args)
        if op is None:
            return None  # invalid function → entire property invalid
        ops.append(op)
        i = j
    return ops


def _build_op(name: str, args: list[str]) -> tuple | None:
    """Build a single op tuple, or None if the function is invalid."""
    try:
        if name == "matrix":
            if len(args) != 6:
                return None
            return ("matrix", tuple(float(a) for a in args))
        if name == "matrix3d":
            if len(args) != 16:
                return None
            return ("matrix3d", tuple(float(a) for a in args))
        if name == "perspective":
            if len(args) != 1:
                return None
            comp = _length_to_component(args[0])
            if comp is None or comp[1] == "%":
                return None
            return ("perspective", comp[0])
        if name == "rotate":
            if len(args) != 1:
                return None
            deg = _angle_to_deg(args[0])
            return ("rotate", deg) if deg is not None else None
        if name == "rotate3d":
            if len(args) != 4:
                return None
            axis = tuple(float(a) for a in args[:3])
            deg = _angle_to_deg(args[3])
            return ("rotate3d", (axis[0], axis[1], axis[2], deg)) if deg is not None else None
        if name in ("rotatex", "rotatey", "rotatez"):
            if len(args) != 1:
                return None
            deg = _angle_to_deg(args[0])
            return (name, deg) if deg is not None else None
        if name == "translate":
            if len(args) < 1 or len(args) > 2:
                return None
            tx = _length_to_component(args[0])
            ty = _length_to_component(args[1]) if len(args) == 2 else (0.0, "px")
            if tx is None or ty is None:
                return None
            return ("translate", (tx, ty))
        if name == "translate3d":
            if len(args) != 3:
                return None
            tx = _length_to_component(args[0])
            ty = _length_to_component(args[1])
            tz = _length_to_component(args[2])
            if tx is None or ty is None or tz is None or tz[1] == "%":
                return None
            return ("translate3d", (tx, ty, tz))
        if name == "translatex":
            if len(args) != 1:
                return None
            tx = _length_to_component(args[0])
            return ("translate", (tx, (0.0, "px"))) if tx is not None else None
        if name == "translatey":
            if len(args) != 1:
                return None
            ty = _length_to_component(args[0])
            return ("translate", ((0.0, "px"), ty)) if ty is not None else None
        if name == "translatez":
            if len(args) != 1:
                return None
            tz = _length_to_component(args[0])
            if tz is None or tz[1] == "%":
                return None
            return ("translate3d", ((0.0, "px"), (0.0, "px"), tz))
        if name == "scale":
            if len(args) < 1 or len(args) > 2:
                return None
            sx = float(args[0])
            sy = float(args[1]) if len(args) == 2 else sx
            return ("scale", (sx, sy))
        if name == "scale3d":
            if len(args) != 3:
                return None
            return ("scale3d", tuple(float(a) for a in args))
        if name == "scalex":
            if len(args) != 1:
                return None
            return ("scale", (float(args[0]), 1.0))
        if name == "scaley":
            if len(args) != 1:
                return None
            return ("scale", (1.0, float(args[0])))
        if name == "scalez":
            if len(args) != 1:
                return None
            return ("scale3d", (1.0, 1.0, float(args[0])))
        if name == "skew":
            if len(args) < 1 or len(args) > 2:
                return None
            ax = _angle_to_deg(args[0])
            ay = _angle_to_deg(args[1]) if len(args) == 2 else 0.0
            if ax is None or ay is None:
                return None
            return ("skew", (ax, ay))
        if name == "skewx":
            if len(args) != 1:
                return None
            ax = _angle_to_deg(args[0])
            return ("skew", (ax, 0.0)) if ax is not None else None
        if name == "skewy":
            if len(args) != 1:
                return None
            ay = _angle_to_deg(args[0])
            return ("skew", (0.0, ay)) if ay is not None else None
    except (ValueError, TypeError):
        return None
    return None  # unknown function


def needs_layout(ops: list[tuple]) -> bool:
    """True when any op carries a % length (needs own-box size)."""
    for op in ops:
        name = op[0]
        if name in _PCT_LENGTH_OPS:
            for arg in op[1]:
                if isinstance(arg, tuple) and arg[1] == "%":
                    return True
    return False


# ═══════════════════════════════════════════════════════════════
#  4x4 matrix math (column-major, 16 floats)
# ═══════════════════════════════════════════════════════════════

def identity() -> tuple[float, ...]:
    return (1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0)


def multiply(a: tuple[float, ...], b: tuple[float, ...]) -> tuple[float, ...]:
    """Column-major matrix multiply: result = a * b."""
    out = [0.0] * 16
    for col in range(4):
        for row in range(4):
            acc = 0.0
            for k in range(4):
                acc += a[k * 4 + row] * b[col * 4 + k]
            out[col * 4 + row] = acc
    return tuple(out)


def is_identity(m: tuple[float, ...]) -> bool:
    return all(abs(m[i] - (1.0 if i % 5 == 0 else 0.0)) < 1e-9 for i in range(16))


def _translate(x: float, y: float, z: float = 0.0) -> tuple[float, ...]:
    m = list(identity())
    m[12] = x
    m[13] = y
    m[14] = z
    return tuple(m)


def _scale(x: float, y: float, z: float = 1.0) -> tuple[float, ...]:
    m = list(identity())
    m[0] = x
    m[5] = y
    m[10] = z
    return tuple(m)


def _rotate_x(deg: float) -> tuple[float, ...]:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return (1.0, 0.0, 0.0, 0.0,
            0.0, c, s, 0.0,
            0.0, -s, c, 0.0,
            0.0, 0.0, 0.0, 1.0)


def _rotate_y(deg: float) -> tuple[float, ...]:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return (c, 0.0, -s, 0.0,
            0.0, 1.0, 0.0, 0.0,
            s, 0.0, c, 0.0,
            0.0, 0.0, 0.0, 1.0)


def _rotate_z(deg: float) -> tuple[float, ...]:
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    return (c, s, 0.0, 0.0,
            -s, c, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 1.0)


def _rotate_axis(x: float, y: float, z: float, deg: float) -> tuple[float, ...]:
    """Rodrigues' rotation about an arbitrary axis (CSS rotate3d)."""
    length = math.sqrt(x * x + y * y + z * z)
    if length < 1e-12:
        return identity()
    x, y, z = x / length, y / length, z / length
    a = math.radians(deg)
    c, s = math.cos(a), math.sin(a)
    t = 1.0 - c
    return (
        t * x * x + c,     t * x * y + s * z, t * x * z - s * y, 0.0,
        t * x * y - s * z, t * y * y + c,     t * y * z + s * x, 0.0,
        t * x * z + s * y, t * y * z - s * x, t * z * z + c,     0.0,
        0.0, 0.0, 0.0, 1.0,
    )


def _skew_x(deg: float) -> tuple[float, ...]:
    m = list(identity())
    m[4] = math.tan(math.radians(deg))
    return tuple(m)


def _skew_y(deg: float) -> tuple[float, ...]:
    m = list(identity())
    m[1] = math.tan(math.radians(deg))
    return tuple(m)


def _perspective(d: float) -> tuple[float, ...]:
    if d <= 0.0:
        return identity()
    m = list(identity())
    m[11] = -1.0 / d
    return tuple(m)


def _matrix6(a: float, b: float, c: float, d: float,
             e: float, f: float) -> tuple[float, ...]:
    return (a, b, 0.0, 0.0,
            c, d, 0.0, 0.0,
            0.0, 0.0, 1.0, 0.0,
            e, f, 0.0, 1.0)


def _matrix3d(v: tuple[float, ...]) -> tuple[float, ...]:
    return v  # CSS matrix3d is already column-major


_OP_COMPOSERS = {
    "matrix": lambda op: _matrix6(*op[1]),
    "matrix3d": lambda op: _matrix3d(op[1]),
    "perspective": lambda op: _perspective(op[1]),
    "rotate": lambda op: _rotate_z(op[1]),
    "rotatex": lambda op: _rotate_x(op[1]),
    "rotatey": lambda op: _rotate_y(op[1]),
    "rotatez": lambda op: _rotate_z(op[1]),
    "rotate3d": lambda op: _rotate_axis(*op[1]),
    "scale": lambda op: _scale(op[1][0], op[1][1]),
    "scale3d": lambda op: _scale(*op[1]),
    "skew": lambda op: multiply(_skew_x(op[1][0]), _skew_y(op[1][1])),
}


def _resolve_length(comp: tuple[float, str], own: float) -> float:
    value, unit = comp
    if value is None:
        return 0.0
    if unit == "%":
        return value / 100.0 * own
    return value


def compose_transform(ops: list[tuple], own_w: float = 0.0,
                      own_h: float = 0.0) -> tuple[float, ...]:
    """Compose parsed ops into a single column-major 4x4 matrix.

    ``own_w`` / ``own_h`` are the element's border-box size, used to
    resolve `%` lengths in translate functions.
    """
    m = identity()
    for op in ops:
        name = op[0]
        if name in ("translate", "translate3d"):
            tx = _resolve_length(op[1][0], own_w)
            ty = _resolve_length(op[1][1], own_h)
            tz = _resolve_length(op[1][2], own_h) if len(op[1]) == 3 else 0.0
            m = multiply(m, _translate(tx, ty, tz))
        else:
            composer = _OP_COMPOSERS.get(name)
            if composer is None:
                continue
            m = multiply(m, composer(op))
    return m