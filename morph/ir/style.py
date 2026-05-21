from dataclasses import dataclass, field


@dataclass
class IRStyle:
    bg_color:      tuple[float,float,float,float] = (0.0, 0.0, 0.0, 0.0)
    color:         tuple[float,float,float,float] = (0.0, 0.0, 0.0, 1.0)
    width:         float | None = None
    height:        float | None = None
    margin:        tuple[float,float,float,float] = (0,0,0,0)  # T R B L
    padding:       tuple[float,float,float,float] = (0,0,0,0)
    border_radius: float = 0.0
    font_size:     float = 16.0
    font_weight:   str   = "normal"
    text_align:    str   = "left"
    display:       str   = "block"
    flex_dir:      str   = "row"
    flex:          float = 0.0
    gap:           float = 0.0
    overflow:      str   = "visible"
    scrollbar_width:            float = 8.0
    scrollbar_track_color:      tuple[float,float,float,float] = (0.85, 0.85, 0.85, 0.4)
    scrollbar_thumb_color:      tuple[float,float,float,float] = (0.5, 0.5, 0.5, 0.6)
    scrollbar_border_radius:    float = 4.0
