#pragma once

#ifdef MORPH_FEATURE_TEXT_SHADOW
#include "base.h"

// One resolved-at-paint text-shadow layer. `hasColor == false` means the
// shadow inherits the run's text color (`currentColor`).
struct TextShadow {
    CssLength offsetX = pxLen(0.0f);
    CssLength offsetY = pxLen(0.0f);
    CssLength blur = pxLen(0.0f);
    float color[4] = {0.0f, 0.0f, 0.0f, 1.0f};
    bool hasColor = false;
};

struct TextShadowStyle {
    bool textShadowSet = false;
    int textShadowCount = 0;
    TextShadow textShadows[4];
};
#endif
