from dataclasses import dataclass, field


@dataclass
class IRStyle:
    bg_color:      tuple[float,float,float,float] = (0.0, 0.0, 0.0, 0.0)
    color:         tuple[float,float,float,float] = (0.0, 0.0, 0.0, 1.0)
    width:         float | None = None
    min_width:     float | None = None
    max_width:     float | None = None
    height:        float | None = None
    min_height:    float | None = None
    max_height:    float | None = None
    margin:        tuple[float,float,float,float] = (0,0,0,0)  # T R B L
    margin_auto:   tuple[bool,bool,bool,bool]     = (False,False,False,False)
    padding:       tuple[float,float,float,float] = (0,0,0,0)
    border_radius: float = 0.0
    font_size:     float = 16.0
    font_weight:   str   = "normal"
    text_align:    str   = "left"
    display:       str   = "block"
    flex_dir:      str   = "row"
    flex_grow:     float = 0.0
    flex_shrink:   float = 1.0
    flex_basis:    str   = "auto"
    gap:           float = 0.0
    position:      str   = "static"
    left:          float | None = None
    right:         float | None = None
    top:           float | None = None
    bottom:        float | None = None
    justify_content: str = "flex-start"
    align_items:     str = "stretch"
    flex_wrap:       str = "nowrap"
    cursor:          str = "default"
    overflow:      str   = "visible"
    scrollbar_width:            float = 8.0
    scrollbar_track_color:      tuple[float,float,float,float] = (0.85, 0.85, 0.85, 0.4)
    scrollbar_thumb_color:      tuple[float,float,float,float] = (0.5, 0.5, 0.5, 0.6)
    scrollbar_border_radius:    float = 4.0
    border_width:               float = 0.0
    border_color:               tuple[float,float,float,float] = (0.0, 0.0, 0.0, 1.0)
    border_style:               str   = "none"
    box_sizing:                 str   = "content-box"
    z_index:                    int | None = None   # None = auto (no explicit z-index)
    # ── Transform (feature: "transform") ────────────────────────
    # Parsed CSS transform ops (see morph/style/transforms.py). None = not set.
    transform_ops:   list[tuple] | None = None
    # Resolved column-major 4x4 matrix (16 floats), set by the layout engine
    # once the element's own border-box size is known. None = no transform.
    transform_matrix: tuple[float, ...] | None = None
    # Raw `transform-origin` value: ((x, is_pct_x), (y, is_pct_y)) in the CSS
    # unit space (keywords left/top/center/right/bottom mapped to 0/0.5/1).
    # None = not set (defaults to 50% 50%).
    transform_origin: tuple | None = None
    # Resolved transform-origin as fractions of the element's own border-box
    # (0..1), set by the layout engine. None = not set (center default).
    transform_origin_resolved: tuple[float, float] | None = None
