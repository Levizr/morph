#pragma once
#include <string>

#ifdef MORPH_FEATURE_BORDER
struct BorderStyle {
    float borderWidth = 0.0f;
    float borderColor[4] = {0,0,0,1};
    std::string borderStyle = "none";
};
#endif
