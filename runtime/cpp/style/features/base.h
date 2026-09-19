#pragma once
#include <string>

#include "../css_enums.h"

struct StyleBase {
    float bgColor[4] = {0,0,0,0};
    float color[4]   = {0,0,0,1};
    float borderRadius = 0.0f;
    float fontSize     = 16.0f;
    float padding[4]   = {0,0,0,0};
    float margin[4]    = {0,0,0,0};
    bool marginAuto[4] = {false,false,false,false};
    float explicitWidth  = -1.0f;
    float explicitHeight = -1.0f;
    float minWidth  = -1.0f;
    float maxWidth  = -1.0f;
    float minHeight = -1.0f;
    float maxHeight = -1.0f;

    CSS::FontWeight fontWeight = CSS::FontWeight::Normal;
    CSS::Overflow overflow = CSS::Overflow::Visible;
    CSS::Display display = CSS::Display::Block;
    CSS::Position position = CSS::Position::Static;
    CSS::TextAlign textAlign = CSS::TextAlign::Left;
    CSS::BoxSizing boxSizing = CSS::BoxSizing::ContentBox;
};
