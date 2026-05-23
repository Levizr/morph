#pragma once
#include <string>

struct StyleBase {
    float bgColor[4] = {0,0,0,0};
    float color[4]   = {0,0,0,1};
    float borderRadius = 0.0f;
    float fontSize     = 16.0f;
    float padding[4]   = {0,0,0,0};
    float margin[4]    = {0,0,0,0};
    float explicitWidth  = -1.0f;
    float explicitHeight = -1.0f;
    float maxWidth = -1.0f;

    std::string fontWeight = "normal";
    std::string overflow = "visible";
    std::string display = "block";
    std::string position = "static";
    std::string textAlign = "left";
    std::string boxSizing = "content-box";
};
