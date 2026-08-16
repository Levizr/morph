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
#ifdef MORPH_FEATURE_TRANSFORM
#include "features/transform.h"
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
#ifdef MORPH_FEATURE_TRANSFORM
    , TransformStyle
#endif
{};

// ── Transform operations (need the complete MorphStyle type) ─────────────

#ifdef MORPH_FEATURE_TRANSFORM

namespace morph {

// Apply a CSS `transform` value to a style.  Percentages resolve against
// the element's own border-box size (own_w / own_h).  Returns false when
// the value is invalid (style left unchanged).
inline bool setCssTransform(MorphStyle& s, const std::string& value,
                            float own_w, float own_h) {
    size_t i = 0;
    size_t n = value.size();
    // Skip leading whitespace.
    while (i < n && (value[i] == ' ' || value[i] == '\t' ||
                     value[i] == '\n' || value[i] == '\r'))
        i++;
    if (i >= n) return false;

    // `none` and global keywords → no transform.
    size_t start = i;
    while (i < n && (isalnum((unsigned char)value[i]) || value[i] == '-')) i++;
    std::string first = value.substr(start, i - start);
    std::string low;
    low.reserve(first.size());
    for (char c : first) low += (char)tolower((unsigned char)c);
    if (low == "none" || low == "inherit" || low == "initial" ||
        low == "revert" || low == "revert-layer" || low == "unset") {
        s.transformSet = false;
        return true;
    }

    // Parse the function list.
    float acc[16];
    mat4Identity(acc);
    while (i < n) {
        while (i < n && (value[i] == ' ' || value[i] == '\t' ||
                         value[i] == '\n' || value[i] == '\r'))
            i++;
        if (i >= n) break;
        start = i;
        while (i < n && (isalnum((unsigned char)value[i]) || value[i] == '-')) i++;
        std::string name = value.substr(start, i - start);
        for (char& c : name) c = (char)tolower((unsigned char)c);
        while (i < n && (value[i] == ' ' || value[i] == '\t' ||
                         value[i] == '\n' || value[i] == '\r'))
            i++;
        if (i >= n || value[i] != '(') return false;  // missing '('
        int depth = 1;
        size_t j = i + 1;
        while (j < n && depth > 0) {
            if (value[j] == '(') depth++;
            else if (value[j] == ')') depth--;
            j++;
        }
        if (depth != 0) return false;  // unbalanced parens
        std::string inner = value.substr(i + 1, j - i - 2);
        std::vector<std::string> args;
        detail::tSplitArgs(inner, args);
        float opm[16];
        if (!detail::tOpMatrix(name, args, own_w, own_h, opm)) return false;
        float tmp[16];
        mat4Multiply(tmp, acc, opm);
        mat4Copy(acc, tmp);
        i = j;
    }
    mat4Copy(s.matrix, acc);
    s.transformSet = true;
    return true;
}

// Reset a style's transform to `none`.
inline void resetCssTransform(MorphStyle& s) {
    s.transformSet = false;
}

// Apply a CSS `transform-origin` value to a style.  Lengths and percentages
// are stored as fractions of the element's own border-box size (own_w/h).
// Supports keywords (left/top/center/right/bottom), `%` and px.  One value
// sets both axes (second defaults to center).  Returns false on invalid
// input (style left unchanged).
inline bool setCssTransformOrigin(MorphStyle& s, const std::string& value,
                                  float own_w, float own_h) {
    std::vector<std::string> parts;
    detail::tSplitArgs(value, parts);
    if (parts.empty() || parts.size() > 2) return false;
    auto axisFraction = [](const std::string& tok, float size,
                           float& frac) -> bool {
        std::string t = tok;
        for (char& c : t) c = (char)tolower((unsigned char)c);
        if (t == "left" || t == "top")    { frac = 0.0f; return true; }
        if (t == "center")                { frac = 0.5f; return true; }
        if (t == "right" || t == "bottom"){ frac = 1.0f; return true; }
        float v; bool pct;
        if (!detail::tLength(tok, v, pct)) return false;
        frac = pct ? (v / 100.0f)
                   : (size > 1e-6f ? (v / size) : 0.0f);
        return true;
    };
    float fx, fy;
    if (!axisFraction(parts[0], own_w, fx)) return false;
    if (parts.size() == 2) {
        if (!axisFraction(parts[1], own_h, fy)) return false;
    } else {
        fy = 0.5f;
    }
    s.originX = fx;
    s.originY = fy;
    s.originSet = true;
    return true;
}

// Interpolate two transform matrices into `s` (for hover/active
// transitions).  Falls back to `a` when decomposition fails.
inline void interpolateTransform(MorphStyle& s, const float a[16],
                                 const float b[16], float t) {
    mat4Interpolate(a, b, t, s.matrix);
    s.transformSet = true;
}

} // namespace morph

#endif // MORPH_FEATURE_TRANSFORM
