#pragma once
#include <cstring>
#include <string>

#include "../css_enums.h"
#include "base.h"

#ifdef MORPH_FEATURE_BORDER

struct BorderStyle {
    // All-side (shorthand)
    CssLength borderWidth = pxLen(0.0f);
    float borderColor[4] = {0,0,0,1};
    CSS::BorderStyle borderStyle = CSS::BorderStyle::None;

    // Per-side (longhand) - when set, they take precedence over shorthand
    CssLength borderTopWidth;
    CssLength borderRightWidth;
    CssLength borderBottomWidth;
    CssLength borderLeftWidth;
    float borderTopColor[4] = {-1,-1,-1,-1};
    float borderRightColor[4] = {-1,-1,-1,-1};
    float borderBottomColor[4] = {-1,-1,-1,-1};
    float borderLeftColor[4] = {-1,-1,-1,-1};
    CSS::BorderStyle borderTopStyle = CSS::BorderStyle::None;
    CSS::BorderStyle borderRightStyle = CSS::BorderStyle::None;
    CSS::BorderStyle borderBottomStyle = CSS::BorderStyle::None;
    CSS::BorderStyle borderLeftStyle = CSS::BorderStyle::None;

    // border-image (for gradient borders)
    bool borderImageEnabled = false;
    bool borderImageIsGradient = false;
    float borderImageSlice = 1.0f;
    // Gradient reference for border-image (points to MorphStyle::bgGradient when used)
    // The actual gradient data lives in MorphStyle::bgGradient
};

inline bool hasPerSideBorder(const BorderStyle& b)
{
    return b.borderTopWidth.isSet() || b.borderRightWidth.isSet() ||
           b.borderBottomWidth.isSet() || b.borderLeftWidth.isSet();
}

// Specified (unresolved) per-side width: longhand wins, else shorthand.
inline const CssLength& getBorderWidthSpec(const BorderStyle& b, int side) // 0=top, 1=right, 2=bottom, 3=left
{
    const CssLength* widths = &b.borderTopWidth;
    return widths[side].isSet() ? widths[side] : b.borderWidth;
}

inline float getBorderWidth(const BorderStyle& b, int side, const UnitEnv& env)
{
    return resolveUnits(getBorderWidthSpec(b, side), env);
}

inline CSS::BorderStyle getBorderStyle(const BorderStyle& b, int side)
{
    const CSS::BorderStyle* styles = &b.borderTopStyle;
    return (styles[side] != CSS::BorderStyle::None) ? styles[side] : b.borderStyle;
}

inline const float* getBorderColor(const BorderStyle& b, int side)
{
    const float (*colors)[4] = &b.borderTopColor;
    return (colors[side][0] >= 0.0f) ? colors[side] : b.borderColor;
}

// Resolve the effective per-side border for rendering. Fills widths
// (unsnapped — snap at the call site), color pointers (valid as long as
// b outlives the call; ops copy the values), and packed modes
// ((side + 1) * 16 + styleEnum) for sides 0=top..3=left. Returns the
// count of visible sides (width > 0 and style != None). When every side
// matches, the border is uniform and callers may use the single-ring
// fast path with modes[0]. Relative widths resolve against `env`.
inline int resolveBorderSides(const BorderStyle& b, const UnitEnv& env, float widths[4],
                              const float* colors[4], float modes[4])
{
    int visible = 0;
    for (int side = 0; side < 4; side++)
    {
        float w = getBorderWidth(b, side, env);
        CSS::BorderStyle st = getBorderStyle(b, side);
        widths[side] = w;
        colors[side] = getBorderColor(b, side);
        modes[side] = (float)((side + 1) * 16 + static_cast<int>(st));
        if (w > 0.0f && st != CSS::BorderStyle::None)
        {
            visible++;
        }
    }
    return visible;
}

inline bool borderSidesUniform(const float widths[4], const float* colors[4],
                               const float modes[4])
{
    for (int side = 1; side < 4; side++)
    {
        if (widths[side] != widths[0] || modes[side] != modes[0])
        {
            return false;
        }
        for (int c = 0; c < 4; c++)
        {
            if (colors[side][c] != colors[0][c])
            {
                return false;
            }
        }
    }
    return true;
}

// Pattern-start/end neighbor sides, mirroring sideAdjacents order:
// top = (left, right), right = (top, bottom), bottom = (right, left),
// left = (bottom, top).
inline void sideNeighbors(int side, int& nStart, int& nEnd)
{
    if (side == 0)
    {
        nStart = 3;
        nEnd = 1;
    }
    else if (side == 1)
    {
        nStart = 0;
        nEnd = 2;
    }
    else if (side == 2)
    {
        nStart = 1;
        nEnd = 3;
    }
    else
    {
        nStart = 2;
        nEnd = 0;
    }
}

// True when two sides share a color (any style): their joint tiles
// exactly with a hard diagonal instead of an AA'd one, so dots and
// dashes meet without a seam.
inline bool borderColorsMatch(const float* colors[4], int a, int b)
{
    for (int c = 0; c < 4; c++)
    {
        if (colors[a][c] != colors[b][c])
        {
            return false;
        }
    }
    return true;
}

// True when two sides paint identically with a solid style: their joint
// butts (full-bleed overlap, crisp like Chrome) instead of mitering.
// A mitered blend of two opaque identical paints tops out at 75%
// coverage under painter's compositing and reads as a dark smear.
// Patterned sides (dotted/dashed) always miter like Chrome: overlapping
// patterns would merge into blobs at the corner.
inline bool borderSidesMatch(const float* colors[4], const float modes[4], int a, int b)
{
    float styleA = modes[a] - (float)((a + 1) * 16);
    float styleB = modes[b] - (float)((b + 1) * 16);
    if (styleA != styleB || styleA != (float)static_cast<int>(CSS::BorderStyle::Solid))
    {
        return false;
    }
    return borderColorsMatch(colors, a, b);
}

// Joint code per corner: 0 = AA miter (different colors), 1 = butt
// (identical solids), 2 = hard miter (same color: exact tiling, no
// seam). Packed as (start + 4 * end) / 16 in the mode fraction — exact
// in float32, decoded the same way in the shader.
inline float borderJointFrac(const float* colors[4], const float modes[4], int side,
                             int nStart, int nEnd)
{
    int sc = 0;
    int ec = 0;
    if (borderSidesMatch(colors, modes, side, nStart))
    {
        sc = 1;
    }
    else if (borderColorsMatch(colors, side, nStart))
    {
        sc = 2;
    }
    if (borderSidesMatch(colors, modes, side, nEnd))
    {
        ec = 1;
    }
    else if (borderColorsMatch(colors, side, nEnd))
    {
        ec = 2;
    }
    return (float)(sc + 4 * ec) / 16.0f;
}

// Total horizontal (left + right) and vertical (top + bottom) border
// thickness for box sizing. Per-side widths fall back to the shorthand,
// so uniform borders keep the old `borderWidth * 2` result exactly.
inline float borderOuterH(const BorderStyle& b, const UnitEnv& env)
{
    return getBorderWidth(b, 3, env) + getBorderWidth(b, 1, env);
}

inline float borderOuterV(const BorderStyle& b, const UnitEnv& env)
{
    return getBorderWidth(b, 0, env) + getBorderWidth(b, 2, env);
}

#endif // MORPH_FEATURE_BORDER
