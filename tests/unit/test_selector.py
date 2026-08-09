from morph.style.selector import (
    parse_selector, calculate_specificity, matches_selector,
    Selector, CompoundSelector, Combinator
)


def test_simple_tag():
    selectors = parse_selector("div")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 0, 1)
    assert matches_selector("div", "div", [])


def test_simple_class():
    selectors = parse_selector(".card")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 1, 0)
    assert matches_selector(".card", "div", ["card"])
    assert not matches_selector(".card", "div", ["other"])


def test_simple_id():
    selectors = parse_selector("#header")
    assert len(selectors) == 1
    assert selectors[0].specificity == (1, 0, 0)
    assert matches_selector("#header", "div", [], "header")
    assert not matches_selector("#header", "div", [], "other")


def test_compound_tag_class():
    selectors = parse_selector("div.card")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 1, 1)
    assert matches_selector("div.card", "div", ["card"])
    assert not matches_selector("div.card", "span", ["card"])
    assert not matches_selector("div.card", "div", ["other"])


def test_compound_tag_id_class():
    selectors = parse_selector("div#main.active")
    assert len(selectors) == 1
    assert selectors[0].specificity == (1, 1, 1)
    assert matches_selector("div#main.active", "div", ["active"], "main")


def test_multiple_classes():
    selectors = parse_selector(".card.active")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 2, 0)
    assert matches_selector(".card.active", "div", ["card", "active"])
    assert not matches_selector(".card.active", "div", ["card"])


def test_comma_separated():
    selectors = parse_selector(".card, .box")
    assert len(selectors) == 2
    assert matches_selector(".card, .box", "div", ["card"])
    assert matches_selector(".card, .box", "div", ["box"])
    assert not matches_selector(".card, .box", "div", ["other"])


def test_descendant():
    selectors = parse_selector("div .card")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 1, 1)
    ancestry = [("div", ["container"])]
    assert matches_selector("div .card", "span", ["card"], None, ancestry)
    assert not matches_selector("div .card", "span", ["card"], None, [])


def test_descendant_multilevel():
    ancestry = [("div", []), ("section", ["wrapper"])]
    assert matches_selector("div .card", "span", ["card"], None, ancestry)
    ancestry = [("nav", []), ("section", ["wrapper"])]
    assert not matches_selector("div .card", "span", ["card"], None, ancestry)


def test_specificity_ordering():
    tag = calculate_specificity("div")
    cls = calculate_specificity(".card")
    id_sel = calculate_specificity("#header")
    compound = calculate_specificity("div.card#main")
    assert tag < cls < id_sel < compound


def test_universal():
    selectors = parse_selector("*")
    assert selectors[0].specificity == (0, 0, 0)
    assert matches_selector("*", "div", [])


def test_universal_with_class():
    selectors = parse_selector("*.foo")
    assert selectors[0].specificity == (0, 1, 0)
    assert matches_selector("*.foo", "div", ["foo"])
    assert matches_selector(".foo*", "div", ["foo"])
    assert not matches_selector("*.foo", "div", ["other"])


def test_universal_with_id():
    assert matches_selector("*#bar", "div", [], "bar")
    assert matches_selector("#bar*", "div", [], "bar")
    assert not matches_selector("*#bar", "div", [], "baz")


def test_universal_descendant():
    assert matches_selector("div *", "span", [], None, [("div", [])])
    assert matches_selector("div *", "span", [], None, [("div", []), ("p", []), ("i", [])])
    assert not matches_selector("div *", "span", [], None, [("nav", [])])


def test_universal_child():
    from morph.style.selector import Combinator
    selectors = parse_selector("div > *")
    assert selectors[0].combinators == [Combinator.CHILD]
    assert matches_selector("div > *", "span", [], None, [("div", [])])
    assert not matches_selector("div > *", "span", [], None, [("div", []), ("section", [])])


def test_universal_pseudo():
    assert matches_selector("*:hover", "div", [])


def test_pseudo_class_ignored():
    selectors = parse_selector("div:hover")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 0, 1)
    assert matches_selector("div:hover", "div", [])


def test_child_combinator():
    selectors = parse_selector("div > p")
    assert len(selectors) == 1
    from morph.style.selector import Combinator
    assert selectors[0].combinators == [Combinator.CHILD]
    # Immediate child should match (parent is 'div')
    ancestry = [("div", [])]
    assert matches_selector("div > p", "p", [], None, ancestry)
    # Non-immediate (grandchild) should NOT match (parent is 'section', not 'div')
    ancestry = [("div", []), ("section", [])]
    assert not matches_selector("div > p", "p", [], None, ancestry)


def test_child_combinator_no_spaces():
    selectors = parse_selector("div>p")
    from morph.style.selector import Combinator
    assert selectors[0].combinators == [Combinator.CHILD]
    ancestry = [("div", [])]
    assert matches_selector("div>p", "p", [], None, ancestry)


def test_descendant_with_child_mixed():
    selectors = parse_selector("div > p span")
    assert len(selectors) == 1
    from morph.style.selector import Combinator
    assert selectors[0].combinators == [Combinator.CHILD, Combinator.DESCENDANT]
    # tree: div > p → span (parent of span is p, p's parent is div)
    ancestry = [("div", []), ("p", [])]
    assert matches_selector("div > p span", "span", [], None, ancestry)
    # tree: div > section → span (parent of span is section, not p)
    ancestry = [("div", []), ("section", [])]
    assert not matches_selector("div > p span", "span", [], None, ancestry)


def test_ancestor_hover_parsing():
    from morph.style.selector import CompoundSelector, Combinator
    selectors = parse_selector("h1:hover button")
    assert len(selectors) == 1
    sel = selectors[0]
    assert sel.compounds[0].tag == "h1"
    assert sel.compounds[0].pseudo == "hover"
    assert sel.compounds[1].tag == "button"
    assert sel.combinators == [Combinator.DESCENDANT]
    # matches_selector should work (pseudo is ignored during matching)
    ancestry = [("div", []), ("h1", [])]
    assert matches_selector("h1:hover button", "button", [], None, ancestry)


def test_ancestor_hover_with_child():
    from morph.style.selector import CompoundSelector, Combinator
    selectors = parse_selector("h1:hover > button")
    assert len(selectors) == 1
    sel = selectors[0]
    assert sel.compounds[0].tag == "h1"
    assert sel.compounds[0].pseudo == "hover"
    assert sel.compounds[1].tag == "button"
    assert sel.combinators == [Combinator.CHILD]
    # Immediate child matches
    ancestry = [("h1", [])]
    assert matches_selector("h1:hover > button", "button", [], None, ancestry)
    # Not immediate child (parent is 'div', not 'h1')
    ancestry = [("h1", []), ("div", [])]
    assert not matches_selector("h1:hover > button", "button", [], None, ancestry)


def test_selector_to_string():
    from morph.style.selector import selector_to_string, CompoundSelector, Combinator
    compounds = [
        CompoundSelector(tag="div", classes=["card"]),
        CompoundSelector(tag="span", pseudo="hover"),
        CompoundSelector(tag="button"),
    ]
    result = selector_to_string(compounds, [Combinator.DESCENDANT, Combinator.CHILD])
    assert result == "div.card span:hover > button"
    # No combinators
    result2 = selector_to_string([CompoundSelector(tag="h1", pseudo="hover")])
    assert result2 == "h1:hover"
