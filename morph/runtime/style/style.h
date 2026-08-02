#pragma once
#include "features/base.h"
#ifdef MORPH_FEATURE_FLEX
#include "features/flex.h"
#endif
#ifdef MORPH_FEATURE_POSITION
#include "features/position.h"
#endif
#ifdef MORPH_FEATURE_SCROLL
#include "features/scroll.h"
#endif
#ifdef MORPH_FEATURE_CURSOR
#include "features/cursor.h"
#endif
#ifdef MORPH_FEATURE_BORDER
#include "features/border.h"
#endif
#ifdef MORPH_FEATURE_ZINDEX
#include "features/zindex.h"
#endif

struct MorphStyle : StyleBase
#ifdef MORPH_FEATURE_FLEX
    , FlexStyle
#endif
#ifdef MORPH_FEATURE_POSITION
    , PositionStyle
#endif
#ifdef MORPH_FEATURE_SCROLL
    , ScrollStyle
#endif
#ifdef MORPH_FEATURE_CURSOR
    , CursorStyle
#endif
#ifdef MORPH_FEATURE_BORDER
    , BorderStyle
#endif
#ifdef MORPH_FEATURE_ZINDEX
    , ZIndexStyle
#endif
{};
