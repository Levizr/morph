#pragma once

#ifdef MORPH_FEATURE_SHADOW
#include <vector>

struct BoxShadow {
    float offsetX = 0.0f;
    float offsetY = 0.0f;
    float blurRadius = 0.0f;
    float spreadRadius = 0.0f;
    float color[4] = {0.0f, 0.0f, 0.0f, 0.5f};
    bool inset = false;
};

struct ShadowStyle {
    std::vector<BoxShadow> boxShadows;
};
#endif
