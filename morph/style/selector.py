from __future__ import annotations
from dataclasses import dataclass, field
from enum import Enum, auto


class Combinator(Enum):
    DESCENDANT = auto()
    CHILD = auto()
    ADJACENT = auto()
    SIBLING = auto()


@dataclass
class CompoundSelector:
    tag: str | None = None
    classes: list[str] = field(default_factory=list)
    id: str | None = None
    universal: bool = False


@dataclass
class Selector:
    compounds: list[CompoundSelector] = field(default_factory=list)
    combinators: list[Combinator] = field(default_factory=list)

    @property
    def specificity(self) -> tuple[int, int, int]:
        a = b = c = 0
        for comp in self.compounds:
            if comp.id:
                a += 1
            b += len(comp.classes)
            if comp.tag and not comp.universal:
                c += 1
        return (a, b, c)

    def matches(self, tag: str, classes: list[str], id: str | None,
                ancestry: list[tuple[str, list[str]]] | None = None) -> bool:
        if not self.compounds:
            return False

        target = self._match_compound(self.compounds[-1], tag, classes, id)
        if not target:
            return False

        if len(self.compounds) == 1:
            return True

        ancestors = ancestry or []
        return self._match_ancestors(self.compounds[:-1], self.combinators[:-1]
                                     if len(self.combinators) >= len(self.compounds) - 1
                                     else self.combinators,
                                     ancestors)

    @staticmethod
    def _match_compound(comp: CompoundSelector, tag: str,
                        classes: list[str], id: str | None) -> bool:
        if comp.universal:
            return True
        if comp.tag and comp.tag != tag:
            return False
        if comp.id and comp.id != id:
            return False
        for cls in comp.classes:
            if cls not in classes:
                return False
        return True

    def _match_ancestors(self, compounds: list[CompoundSelector],
                         combinators: list[Combinator],
                         ancestry: list[tuple[str, list[str]]]) -> bool:
        if not compounds:
            return True

        comp_idx = len(compounds) - 1
        for ancestor_tag, ancestor_classes in reversed(ancestry):
            if self._match_compound(compounds[comp_idx], ancestor_tag, ancestor_classes, None):
                comp_idx -= 1
                if comp_idx < 0:
                    return True
            else:
                if comp_idx < len(compounds) - 1:
                    continue
        return False


def parse_selector(key: str) -> list[Selector]:
    key = key.strip()
    if not key:
        return []

    parts = [p.strip() for p in key.split(",")]
    selectors = []
    for part in parts:
        if not part:
            continue
        sel = _parse_single(part)
        if sel:
            selectors.append(sel)
    return selectors


def _parse_single(part: str) -> Selector | None:
    if not part:
        return None

    raw_compounds = _split_by_combinators(part)
    compounds: list[CompoundSelector] = []
    combinators: list[Combinator] = []

    for i, raw in enumerate(raw_compounds):
        comp = _parse_compound(raw.strip())
        if comp is None:
            return None
        compounds.append(comp)

    for i in range(len(raw_compounds) - 1):
        combinators.append(Combinator.DESCENDANT)

    return Selector(compounds=compounds, combinators=combinators)


def _split_by_combinators(part: str) -> list[str]:
    result: list[str] = []
    current: list[str] = []
    depth = 0
    for ch in part:
        if ch in ("(", "["):
            depth += 1
            current.append(ch)
        elif ch in (")", "]"):
            depth -= 1
            current.append(ch)
        elif ch == " " and depth == 0:
            if current:
                result.append("".join(current))
                current = []
        elif ch == ">" and depth == 0:
            if current:
                result.append("".join(current))
                current = []
        else:
            current.append(ch)
    if current:
        result.append("".join(current))
    return result


def _parse_compound(raw: str) -> CompoundSelector | None:
    raw = raw.strip()
    if not raw:
        return None

    comp = CompoundSelector()
    i = 0
    buf: list[str] = []

    while i < len(raw):
        ch = raw[i]
        if ch == ".":
            if buf:
                comp.tag = "".join(buf)
                buf = []
            i += 1
            cls_start = i
            while i < len(raw) and raw[i] not in (".", "#", ":", "(", ")", "[", "]", " ", ">"):
                i += 1
            if i > cls_start:
                comp.classes.append(raw[cls_start:i])
        elif ch == "#":
            if buf:
                comp.tag = "".join(buf)
                buf = []
            i += 1
            id_start = i
            while i < len(raw) and raw[i] not in (".", "#", ":", "(", ")", "[", "]", " ", ">"):
                i += 1
            if i > id_start:
                comp.id = raw[id_start:i]
        elif ch == ":":
            while i < len(raw) and raw[i] not in (".", "#", " ", ">"):
                i += 1
        elif ch in (" ", ">"):
            break
        else:
            buf.append(ch)
            i += 1

    if buf:
        tag_str = "".join(buf)
        if tag_str == "*":
            comp.universal = True
        else:
            comp.tag = tag_str

    return comp


def calculate_specificity(key: str) -> tuple[int, int, int]:
    selectors = parse_selector(key)
    if not selectors:
        return (0, 0, 0)
    return max(s.specificity for s in selectors)


def matches_selector(key: str, tag: str, classes: list[str],
                     id: str | None = None,
                     ancestry: list[tuple[str, list[str]]] | None = None) -> bool:
    selectors = parse_selector(key)
    if not selectors:
        return False
    for sel in selectors:
        if sel.matches(tag, classes, id, ancestry):
            return True
    return False
