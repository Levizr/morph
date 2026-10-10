#pragma once

#ifdef MORPH_FEATURE_SCROLL
struct ScrollStyle {
    float scrollbarWidth  = 8.0f;
    float scrollbarTrackColor[4] = {0.85f, 0.85f, 0.85f, 0.0f};
    float scrollbarThumbColor[4] = {0.5f, 0.5f, 0.5f, 0.6f};
    float scrollbarBorderRadius = 4.0f;
    // `scroll-behavior: smooth` — wheel/track paging glides toward the
    // target instead of jumping (default `auto` keeps instant steps).
    bool scrollBehaviorSmooth = false;
};
#endif
