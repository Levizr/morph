#pragma once

#ifdef MORPH_FEATURE_OUTLINE
#include "base.h"
#include "../css_enums.h"

struct OutlineStyle {
    // Set when the shorthand/longhand appeared, even if the resolved outline
    // is invisible (e.g. `outline: none`) so delta application can clear it.
    bool outlineSet = false;
    // CSS initial `medium`. Style must still be non-none to paint.
    CssLength outlineWidth = pxLen(3.0f);
    float outlineColor[4] = {0.0f, 0.0f, 0.0f, 1.0f};
    // CSS initial `outline-color` is `currentcolor`; resolve it from the
    // element's text color at paint time. Explicit colors clear this flag.
    bool outlineColorCurrent = true;
    CSS::BorderStyle outlineStyle = CSS::BorderStyle::None;
    CssLength outlineOffset = pxLen(0.0f); // px outside the border edge
};
#endif
