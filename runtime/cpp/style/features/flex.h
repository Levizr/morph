#pragma once
#include <string>

#include "../css_enums.h"

#ifdef MORPH_FEATURE_FLEX
struct FlexStyle {
    CSS::FlexDirection flexDirection = CSS::FlexDirection::Row;
    CSS::JustifyContent justifyContent = CSS::JustifyContent::FlexStart;
    CSS::AlignItems alignItems = CSS::AlignItems::Stretch;
    CSS::FlexWrap flexWrap = CSS::FlexWrap::Nowrap;
    float flexGrow = 0.0f;
    float flexShrink = 1.0f;
    std::string flexBasis = "auto";
    float gap = 0.0f;
};
#endif
