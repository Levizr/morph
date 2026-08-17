"""IR types + parsers for CSS `animation` and `@keyframes`.

Keyframes are resolved to partial ``IRStyle`` objects (only the animated
fields are set).  The ``animation`` shorthand and its longhands are parsed
into ``IRAnimation`` configs consumed by the C++ runtime.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from morph.ir.style import IRStyle

INFINITE = -1.0  # iteration-count: infinite (C++ treats negatives as infinite)

_EASING_KEYWORDS = {
    "linear": "linear",
    "ease": "ease-in-out",       # runtime has no separate `ease` curve
    "ease-in": "ease-in",
    "ease-out": "ease-out",
    "ease-in-out": "ease-in-out",
}

_DIRECTION_KEYWORDS = {"normal", "reverse", "alternate", "alternate-reverse"}
_FILL_MODE_KEYWORDS = {"none", "forwards", "backwards", "both"}
_PLAY_STATE_KEYWORDS = {"running", "paused"}


@dataclass
class IRKeyframe:
    offset: float = 0.0          # 0..1
    style: IRStyle = field(default_factory=IRStyle)   # partial style
    # IR style field names this keyframe explicitly declares (e.g.
    # {"opacity", "bg_color"}).  Lets emitters serialize partial styles
    # without mistaking default-valued declarations for "unset".
    declared: set[str] = field(default_factory=set)
    # Raw CSS values that need layout-time resolution at runtime (% lengths,
    # transforms).  Property name → CSS string.
    raw: dict[str, str] = field(default_factory=dict)


@dataclass
class IRAnimation:
    name: str = ""
    duration: float = 0.0        # seconds (0 = no animation)
    easing: str = "linear"
    delay: float = 0.0           # seconds
    iterations: float = 1.0      # INFINITE (-1) = infinite
    direction: str = "normal"
    fill_mode: str = "none"
    play_state: str = "running"


# ── Time parsing ───────────────────────────────────────────────

def parse_time(raw: str) -> float | None:
    """Parse a CSS time like '0.3s' / '500ms' to float seconds."""
    raw = raw.strip().lower()
    try:
        if raw.endswith("ms"):
            return float(raw[:-2]) / 1000.0
        if raw.endswith("s"):
            return float(raw[:-1])
        return float(raw)  # unitless — treated as seconds
    except ValueError:
        return None


# ── Shorthand parsing ──────────────────────────────────────────

def _is_float(raw: str) -> bool:
    try:
        float(raw)
        return True
    except ValueError:
        return False


def _split_animation_list(raw: str) -> list[str]:
    """Split `animation: a, b` on top-level commas (ignores parens)."""
    parts = []
    depth = 0
    cur = []
    for ch in raw:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    parts.append("".join(cur))
    return [p.strip() for p in parts if p.strip()]


def _parse_animation_component(raw: str) -> IRAnimation | None:
    """Parse one comma-separated animation shorthand value."""
    anim = IRAnimation()
    tokens = raw.split()
    unclassified = []
    for tok in tokens:
        low = tok.lower()
        if low in _EASING_KEYWORDS:
            anim.easing = _EASING_KEYWORDS[low]
        elif low in _DIRECTION_KEYWORDS:
            anim.direction = low
        elif low in _FILL_MODE_KEYWORDS:
            anim.fill_mode = low
        elif low in _PLAY_STATE_KEYWORDS:
            anim.play_state = low
        elif low == "infinite":
            anim.iterations = INFINITE
        elif _is_float(low):
            # Fractional iteration counts (e.g. "2.5") are common — a plain
            # digits-only check would misparse them as times.
            anim.iterations = float(low)
        else:
            t = parse_time(low)
            if t is not None:
                # First time = duration, second = delay
                if anim.duration == 0.0:
                    anim.duration = t
                else:
                    anim.delay = t
            else:
                # Ignore unsupported easing functions (cubic-bezier, steps…)
                if "(" not in low:
                    unclassified.append(tok)
    if unclassified:
        anim.name = unclassified[0]
    return anim


def parse_animation_shorthand(raw: str) -> list[IRAnimation]:
    """Parse the `animation` shorthand into a list of IRAnimation."""
    return [a for a in (_parse_animation_component(p)
                        for p in _split_animation_list(raw))
            if a is not None and a.name]


# ── Longhand parsing ───────────────────────────────────────────

_LONGHANDS = (
    "animation-name", "animation-duration", "animation-timing-function",
    "animation-delay", "animation-iteration-count", "animation-direction",
    "animation-fill-mode", "animation-play-state",
)


def _longhand_values(raw: str) -> list[str]:
    return [v.strip() for v in _split_animation_list(raw)]


def _apply_longhand(anim: IRAnimation, prop: str, value: str) -> bool:
    value = value.strip().lower()
    if prop == "animation-name":
        anim.name = value
    elif prop == "animation-duration":
        t = parse_time(value)
        if t is None:
            return False
        anim.duration = t
    elif prop == "animation-timing-function":
        anim.easing = _EASING_KEYWORDS.get(value, anim.easing)
    elif prop == "animation-delay":
        t = parse_time(value)
        if t is None:
            return False
        anim.delay = t
    elif prop == "animation-iteration-count":
        try:
            anim.iterations = INFINITE if value == "infinite" else float(value)
        except ValueError:
            return False
    elif prop == "animation-direction":
        if value in _DIRECTION_KEYWORDS:
            anim.direction = value
    elif prop == "animation-fill-mode":
        if value in _FILL_MODE_KEYWORDS:
            anim.fill_mode = value
    elif prop == "animation-play-state":
        if value in _PLAY_STATE_KEYWORDS:
            anim.play_state = value
    return True


def parse_animations(css_dict: dict) -> list[IRAnimation]:
    """Build the animation list for a node from merged CSS declarations.

    Starts from the `animation` shorthand (may define several animations),
    then applies longhands as per-index overrides (CSS list semantics: the
    last value repeats for missing indices).  Animations without a name are
    dropped.  `animation-play-state` on its own never creates an animation.
    """
    shorthand = css_dict.get("animation", "")
    anims: list[IRAnimation] = []
    if shorthand:
        anims = parse_animation_shorthand(shorthand)

    longhand_lists: dict[str, list[str]] = {}
    for prop in _LONGHANDS:
        raw = css_dict.get(prop)
        if raw:
            longhand_lists[prop] = _longhand_values(raw)

    if not longhand_lists:
        return [a for a in anims if a.name]

    # Apply longhands to the shorthand list (extending it when longhands
    # arrive without a matching shorthand, e.g. animation-name + duration).
    count = max(len(anims), *[len(v) for v in longhand_lists.values()])
    while len(anims) < count:
        anims.append(IRAnimation())

    for prop, values in longhand_lists.items():
        for i in range(count):
            val = values[i] if i < len(values) else values[-1]
            _apply_longhand(anims[i], prop, val)

    return [a for a in anims if a.name]
