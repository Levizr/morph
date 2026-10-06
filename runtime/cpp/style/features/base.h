#pragma once
#include <cmath>
#include <string>

#include "../css_enums.h"

struct StyleBase {
    float bgColor[4] = {0,0,0,0};
    float color[4]   = {0,0,0,1};
    float borderRadius = 0.0f;
    // Per-corner radius longhands (clockwise from top-left); negative
    // means unset and falls back to borderRadius.
    float borderTopLeftRadius = -1.0f;
    float borderTopRightRadius = -1.0f;
    float borderBottomRightRadius = -1.0f;
    float borderBottomLeftRadius = -1.0f;
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

// Effective corner radii in clockwise order from top-left
// (TL TR BR BL). A set longhand wins, otherwise the shorthand applies.
inline void resolveBorderRadii(const StyleBase& s, bool snap, float out[4])
{
    out[0] = s.borderTopLeftRadius >= 0.0f ? s.borderTopLeftRadius : s.borderRadius;
    out[1] = s.borderTopRightRadius >= 0.0f ? s.borderTopRightRadius : s.borderRadius;
    out[2] = s.borderBottomRightRadius >= 0.0f ? s.borderBottomRightRadius : s.borderRadius;
    out[3] = s.borderBottomLeftRadius >= 0.0f ? s.borderBottomLeftRadius : s.borderRadius;
    if (snap)
    {
        for (int i = 0; i < 4; i++)
        {
            out[i] = std::round(out[i]);
        }
    }
}

// Control corners (buttons, inputs): like resolveBorderRadii but an unset
// shorthand falls back to the control default instead of sharp.
inline void resolveControlRadii(const StyleBase& s, bool snap, float out[4])
{
    float base = s.borderRadius > 0.0f ? s.borderRadius : 6.0f;
    out[0] = s.borderTopLeftRadius >= 0.0f ? s.borderTopLeftRadius : base;
    out[1] = s.borderTopRightRadius >= 0.0f ? s.borderTopRightRadius : base;
    out[2] = s.borderBottomRightRadius >= 0.0f ? s.borderBottomRightRadius : base;
    out[3] = s.borderBottomLeftRadius >= 0.0f ? s.borderBottomLeftRadius : base;
    if (snap)
    {
        for (int i = 0; i < 4; i++)
        {
            out[i] = std::round(out[i]);
        }
    }
}
