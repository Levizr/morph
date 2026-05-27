#pragma once
#include <string>

#ifdef MORPH_FEATURE_FLEX
struct FlexStyle {
    std::string flexDirection = "column";
    std::string justifyContent = "flex-start";
    std::string alignItems = "stretch";
    std::string flexWrap = "nowrap";
    float gap = 0.0f;
};
#endif
