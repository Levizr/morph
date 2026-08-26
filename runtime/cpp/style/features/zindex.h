#pragma once

#ifdef MORPH_FEATURE_ZINDEX
struct ZIndexStyle {
    // zIndex == 0 with zIndexSet == false means `auto` (no explicit z-index).
    int zIndex = 0;
    bool zIndexSet = false;
};
#endif
