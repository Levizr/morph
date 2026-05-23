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
{};
