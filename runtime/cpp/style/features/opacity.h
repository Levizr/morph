#pragma once

#ifdef MORPH_FEATURE_OPACITY
struct OpacityStyle {
    // 1.0 = fully opaque (CSS default). Values outside [0,1] are clamped at
    // parse time. Applied as an accumulated alpha multiplier over the whole
    // subtree during flatten (background, border, text, scrollbar).
    float opacity = 1.0f;
};
#endif
