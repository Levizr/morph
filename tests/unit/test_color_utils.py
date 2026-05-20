from morph.utils.color import hex_to_float, parse_color


def test_hex_to_float_white():
    assert hex_to_float("#ffffff") == (1.0, 1.0, 1.0, 1.0)


def test_hex_to_float_black():
    r, g, b, a = hex_to_float("#000000")
    assert r == 0.0 and g == 0.0 and b == 0.0


def test_shorthand_hex():
    assert hex_to_float("#fff") == (1.0, 1.0, 1.0, 1.0)


def test_parse_color_hex():
    assert parse_color("#ff0000")[0] == 1.0
