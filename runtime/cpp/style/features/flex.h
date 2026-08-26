#pragma once
#include <string>

#ifdef MORPH_FEATURE_FLEX
struct FlexStyle {
    std::string flexDirection = "row";
    std::string justifyContent = "flex-start";
    std::string alignItems = "stretch";
    std::string flexWrap = "nowrap";
    float flexGrow = 0.0f;
    float flexShrink = 1.0f;
    std::string flexBasis = "auto";
    float gap = 0.0f;
};
#endif
