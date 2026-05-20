"""CSS color string → float RGBA tuple."""


def hex_to_float(hex_str: str) -> tuple[float, float, float, float]:
    h = hex_str.lstrip("#")
    if len(h) == 3:
        h = "".join(c*2 for c in h)
    r = int(h[0:2], 16) / 255.0
    g = int(h[2:4], 16) / 255.0
    b = int(h[4:6], 16) / 255.0
    a = int(h[6:8], 16) / 255.0 if len(h) == 8 else 1.0
    return r, g, b, a


def rgb_to_float(r: int, g: int, b: int,
                 a: int = 255) -> tuple[float, float, float, float]:
    return r/255.0, g/255.0, b/255.0, a/255.0


def parse_color(value: str) -> tuple[float, float, float, float]:
    value = value.strip()
    if value.startswith("#"):
        return hex_to_float(value)
    if value.startswith("rgb"):
        nums = [int(x.strip()) for x in
                value.split("(")[1].rstrip(")").split(",")]
        return rgb_to_float(*nums)
    return (0.0, 0.0, 0.0, 1.0)
