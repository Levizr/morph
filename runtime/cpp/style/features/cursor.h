#pragma once
#include <string>

#include "../css_enums.h"

#ifdef MORPH_FEATURE_CURSOR
struct CursorStyle {
    CSS::Cursor cursor = CSS::Cursor::Default;
};
#endif
