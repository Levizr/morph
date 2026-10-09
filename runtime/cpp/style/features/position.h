#pragma once

#ifdef MORPH_FEATURE_POSITION
// `auto` is value <= -1e8f; percentages resolve against the
// containing block (width for left/right, height for top/bottom).
struct PositionStyle {
    CssLength left;
    CssLength right;
    CssLength top;
    CssLength bottom;
};
#endif
