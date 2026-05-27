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


def test_pseudo_class_ignored():
    selectors = parse_selector("div:hover")
    assert len(selectors) == 1
    assert selectors[0].specificity == (0, 0, 1)
    assert matches_selector("div:hover", "div", [])
