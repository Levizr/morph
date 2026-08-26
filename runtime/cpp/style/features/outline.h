#pragma once
#include <string>

#ifdef MORPH_FEATURE_OUTLINE
struct OutlineStyle {
    float outlineWidth = 0.0f;
    float outlineColor[4] = {0.0f, 0.0f, 0.0f, 1.0f};
    std::string outlineStyle = "none";  // solid/dashed/dotted
    float outlineOffset = 0.0f;         // px outside border edge
};
#endif
