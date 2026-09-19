#pragma once
#include <string>

#include "../css_enums.h"

#ifdef MORPH_FEATURE_BORDER
struct BorderStyle {
    float borderWidth = 0.0f;
    float borderColor[4] = {0,0,0,1};
    CSS::BorderStyle borderStyle = CSS::BorderStyle::None;
};
#endif
