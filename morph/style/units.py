"""Convert CSS unit strings to float pixel values."""


def to_px(value: str, parent_px: float = 0.0, root_px: float = 16.0) -> float:
    value = value.strip()
    if value.endswith("px"):
        return float(value[:-2])
    if value.endswith("%"):
        return (float(value[:-1]) / 100.0) * parent_px
    if value.endswith("em"):
        return float(value[:-2]) * root_px
    if value == "0":
        return 0.0
    # TODO: rem, vh, vw
    return 0.0
