#pragma once

#ifdef MORPH_FEATURE_SHADOW
#include <cstring>

#include "base.h"

// Maximum shadows per box (v1 shader limit: unrolled uniform arrays).
// Matches `MAX_BOX_SHADOWS` in `crates/morph-ir/src/shadow.rs`.
inline constexpr int MORPH_SHADOW_MAX = 4;

struct BoxShadow
{
    CssLength offsetX;
    CssLength offsetY;
    CssLength blur;
    CssLength spread;
    float color[4] = {0.0f, 0.0f, 0.0f, 0.5f};
    bool inset = false;
};

struct ShadowStyle
{
    BoxShadow boxShadows[MORPH_SHADOW_MAX];
    int boxShadowCount = 0;
    // True when the style explicitly specifies box-shadow (present or
    // deliberately removed). Deltas start false, so a hover rule that
    // never mentions shadows can never wipe the base shadow — same
    // contract as `bgGradientSet`.
    bool boxShadowSet = false;
};

namespace morph
{

inline bool shadowsEqual(const ShadowStyle& a, const ShadowStyle& b)
{
    if (a.boxShadowSet != b.boxShadowSet || a.boxShadowCount != b.boxShadowCount)
    {
        return false;
    }
    for (int i = 0; i < a.boxShadowCount; i++)
    {
        const BoxShadow& sa = a.boxShadows[i];
        const BoxShadow& sb = b.boxShadows[i];
        if (!(sa.offsetX == sb.offsetX) || !(sa.offsetY == sb.offsetY) ||
            !(sa.blur == sb.blur) || !(sa.spread == sb.spread) ||
            sa.inset != sb.inset)
        {
            return false;
        }
        for (int c = 0; c < 4; c++)
        {
            if (sa.color[c] != sb.color[c])
            {
                return false;
            }
        }
    }
    return true;
}

// True when two shadow lists interpolate layer-by-layer: same count, same
// inset-ness, same units on every length (values may differ). Anything
// else snaps discretely, like browsers across shadow-count changes.
inline bool shadowsCompatible(const ShadowStyle& a, const ShadowStyle& b)
{
    if (!a.boxShadowSet || !b.boxShadowSet || a.boxShadowCount != b.boxShadowCount)
    {
        return false;
    }
    for (int i = 0; i < a.boxShadowCount; i++)
    {
        const BoxShadow& sa = a.boxShadows[i];
        const BoxShadow& sb = b.boxShadows[i];
        if (sa.inset != sb.inset || sa.offsetX.unit != sb.offsetX.unit ||
            sa.offsetY.unit != sb.offsetY.unit || sa.blur.unit != sb.blur.unit ||
            sa.spread.unit != sb.spread.unit)
        {
            return false;
        }
    }
    return true;
}

// Interpolate two shadow lists for hover/active transitions and
// keyframes. Structurally different lists snap to the target.
inline void lerpShadow(ShadowStyle& out, const ShadowStyle& a,
                       const ShadowStyle& b, float t)
{
    if (!a.boxShadowSet && !b.boxShadowSet)
    {
        out.boxShadowSet = false;
        out.boxShadowCount = 0;
        return;
    }
    if (!shadowsCompatible(a, b))
    {
        out = b;
        return;
    }
    out.boxShadowSet = true;
    out.boxShadowCount = b.boxShadowCount;
    for (int i = 0; i < b.boxShadowCount; i++)
    {
        const BoxShadow& sa = a.boxShadows[i];
        const BoxShadow& sb = b.boxShadows[i];
        BoxShadow& so = out.boxShadows[i];
        so.offsetX.value = sa.offsetX.value + (sb.offsetX.value - sa.offsetX.value) * t;
        so.offsetX.unit = sb.offsetX.unit;
        so.offsetY.value = sa.offsetY.value + (sb.offsetY.value - sa.offsetY.value) * t;
        so.offsetY.unit = sb.offsetY.unit;
        so.blur.value = sa.blur.value + (sb.blur.value - sa.blur.value) * t;
        so.blur.unit = sb.blur.unit;
        so.spread.value = sa.spread.value + (sb.spread.value - sa.spread.value) * t;
        so.spread.unit = sb.spread.unit;
        for (int c = 0; c < 4; c++)
        {
            so.color[c] = sa.color[c] + (sb.color[c] - sa.color[c]) * t;
        }
        so.inset = sb.inset;
    }
}

// Resolve a style shadow list to px (flatten + immediate paint share
// this; `env` carries font/viewport bases like border radii).
inline int resolveBoxShadows(const ShadowStyle& src, const UnitEnv& env,
                             BoxShadow* dst)
{
    if (!src.boxShadowSet)
    {
        return 0;
    }
    int n = src.boxShadowCount < MORPH_SHADOW_MAX ? src.boxShadowCount : MORPH_SHADOW_MAX;
    for (int i = 0; i < n; i++)
    {
        const BoxShadow& s = src.boxShadows[i];
        BoxShadow& d = dst[i];
        d.offsetX.value = resolveUnits(s.offsetX, env);
        d.offsetX.unit = LengthUnit::Px;
        d.offsetY.value = resolveUnits(s.offsetY, env);
        d.offsetY.unit = LengthUnit::Px;
        d.blur.value = resolveUnits(s.blur, env);
        d.blur.unit = LengthUnit::Px;
        if (d.blur.value < 0.0f)
        {
            d.blur.value = 0.0f;
        }
        d.spread.value = resolveUnits(s.spread, env);
        d.spread.unit = LengthUnit::Px;
        std::memcpy(d.color, s.color, sizeof(float) * 4);
        d.inset = s.inset;
    }
    return n;
}

namespace shadow_detail
{

inline void skipWs(const char*& p)
{
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')
    {
        p++;
    }
}

inline bool parseNum(const char*& p, float& out)
{
    skipWs(p);
    char* end = nullptr;
    double v = std::strtod(p, &end);
    if (end == p)
    {
        return false;
    }
    out = (float)v;
    p = end;
    return true;
}

// Shadow lengths: px suffix or bare number. Font/viewport units need a
// layout context keyframes don't have, so they fail (underlying value).
inline bool parseShadowLength(const char*& p, float& out)
{
    float v = 0.0f;
    if (!parseNum(p, v))
    {
        return false;
    }
    skipWs(p);
    if ((p[0] == 'p' || p[0] == 'P') && (p[1] == 'x' || p[1] == 'X'))
    {
        p += 2;
    }
    out = v;
    return true;
}

inline bool parseShadowColor(const char*& p, float c[4])
{
    skipWs(p);
    if (*p == '#')
    {
        p++;
        const char* start = p;
        while ((*p >= '0' && *p <= '9') || (*p >= 'a' && *p <= 'f') ||
               (*p >= 'A' && *p <= 'F'))
        {
            p++;
        }
        std::string hex(start, p);
        unsigned r = 0, g = 0, b = 0, a = 255;
        if (hex.size() == 3 || hex.size() == 4)
        {
            auto d = [](char ch) -> unsigned {
                if (ch >= '0' && ch <= '9')
                {
                    return (unsigned)(ch - '0');
                }
                if (ch >= 'a' && ch <= 'f')
                {
                    return (unsigned)(ch - 'a' + 10);
                }
                return (unsigned)(ch - 'A' + 10);
            };
            r = d(hex[0]) * 17;
            g = d(hex[1]) * 17;
            b = d(hex[2]) * 17;
            if (hex.size() == 4)
            {
                a = d(hex[3]) * 17;
            }
        }
        else if (hex.size() == 6 || hex.size() == 8)
        {
            r = std::stoul(hex.substr(0, 2), nullptr, 16);
            g = std::stoul(hex.substr(2, 2), nullptr, 16);
            b = std::stoul(hex.substr(4, 2), nullptr, 16);
            if (hex.size() == 8)
            {
                a = std::stoul(hex.substr(6, 2), nullptr, 16);
            }
        }
        else
        {
            return false;
        }
        c[0] = (float)r / 255.0f;
        c[1] = (float)g / 255.0f;
        c[2] = (float)b / 255.0f;
        c[3] = (float)a / 255.0f;
        return true;
    }
    if (std::strncmp(p, "rgb", 3) == 0)
    {
        p += 3;
        bool hasA = false;
        if (*p == 'a' || *p == 'A')
        {
            hasA = true;
            p++;
        }
        skipWs(p);
        if (*p != '(')
        {
            return false;
        }
        p++;
        float comp[4] = {0, 0, 0, 1.0f};
        for (int i = 0; i < 4; i++)
        {
            if (i == 3)
            {
                skipWs(p);
                if (*p == ')')
                {
                    break;
                }
                if (*p != ',')
                {
                    return false;
                }
                p++;
            }
            else if (i > 0)
            {
                skipWs(p);
                if (*p != ',')
                {
                    return false;
                }
                p++;
            }
            if (!parseNum(p, comp[i]))
            {
                return false;
            }
            skipWs(p);
            if (*p == '%')
            {
                comp[i] = comp[i] / 100.0f;
                p++;
            }
            else if (i < 3)
            {
                comp[i] = comp[i] / 255.0f;
            }
        }
        skipWs(p);
        if (*p != ')')
        {
            return false;
        }
        p++;
        std::memcpy(c, comp, sizeof(float) * 4);
        return true;
    }
    char word[16];
    size_t wi = 0;
    while ((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z'))
    {
        if (wi + 1 < sizeof(word))
        {
            word[wi++] = *p;
        }
        p++;
    }
    word[wi] = '\0';
    if (wi == 0)
    {
        return false;
    }
    std::string w = word;
    for (char& ch : w)
    {
        ch = (char)std::tolower((unsigned char)ch);
    }
    // Named set mirrors the Rust `parse_color` used at build time.
    if (w == "transparent")
    {
        c[0] = c[1] = c[2] = 0.0f;
        c[3] = 0.0f;
        return true;
    }
    if (w == "red")
    {
        c[0] = 1.0f;
        c[1] = c[2] = 0.0f;
        c[3] = 1.0f;
        return true;
    }
    if (w == "green")
    {
        c[1] = 0.5f;
        c[0] = c[2] = 0.0f;
        c[3] = 1.0f;
        return true;
    }
    if (w == "blue")
    {
        c[2] = 1.0f;
        c[0] = c[1] = 0.0f;
        c[3] = 1.0f;
        return true;
    }
    if (w == "black")
    {
        c[0] = c[1] = c[2] = 0.0f;
        c[3] = 1.0f;
        return true;
    }
    if (w == "white")
    {
        c[0] = c[1] = c[2] = 1.0f;
        c[3] = 1.0f;
        return true;
    }
    if (w == "gray" || w == "grey")
    {
        c[0] = c[1] = c[2] = 0.5f;
        c[3] = 1.0f;
        return true;
    }
    return false;
}

} // namespace shadow_detail

// Parse a `box-shadow` value (keyframe payloads). Mirrors the Rust parser
// in `crates/morph-ir/src/shadow.rs`; fills `out` (count in
// `out.boxShadowCount`, set flag on success) and returns true. `none`
// clears; anything else unparseable returns false (underlying value).
inline bool parseShadowCss(const std::string& css, ShadowStyle& out)
{
    const char* p = css.c_str();
    shadow_detail::skipWs(p);
    if (*p == '\0')
    {
        return false;
    }
    if (std::strncmp(p, "none", 4) == 0)
    {
        const char* q = p + 4;
        shadow_detail::skipWs(q);
        if (*q == '\0')
        {
            out.boxShadowSet = true;
            out.boxShadowCount = 0;
            return true;
        }
    }
    ShadowStyle tmp;
    tmp.boxShadowSet = true;
    tmp.boxShadowCount = 0;
    while (true)
    {
        if (tmp.boxShadowCount >= MORPH_SHADOW_MAX)
        {
            return false;
        }
        BoxShadow& s = tmp.boxShadows[tmp.boxShadowCount];
        s.offsetX.unit = LengthUnit::Px;
        s.offsetY.unit = LengthUnit::Px;
        s.blur.unit = LengthUnit::Px;
        s.spread.unit = LengthUnit::Px;
        s.offsetX.value = s.offsetY.value = s.blur.value = s.spread.value = 0.0f;
        s.color[0] = s.color[1] = s.color[2] = 0.0f;
        s.color[3] = 0.5f;
        s.inset = false;
        // One layer: `[inset]? ox oy [blur]? [spread]? [color]?`.
        float lens[4] = {0, 0, 0, 0};
        int nlen = 0;
        bool haveColor = false;
        while (true)
        {
            shadow_detail::skipWs(p);
            if (*p == '\0' || *p == ',')
            {
                break;
            }
            if ((p[0] == 'i' || p[0] == 'I') && (p[1] == 'n' || p[1] == 'N') &&
                (p[2] == 's' || p[2] == 'S') && (p[3] == 'e' || p[3] == 'E') &&
                (p[4] == 't' || p[4] == 'T'))
            {
                const char after = p[5];
                if (after == ' ' || after == '\t' || after == '\0' || after == ',')
                {
                    s.inset = true;
                    p += 5;
                    continue;
                }
            }
            const char* save = p;
            float v = 0.0f;
            if (nlen < 4 && shadow_detail::parseShadowLength(p, v))
            {
                lens[nlen++] = v;
                continue;
            }
            p = save;
            if (!haveColor && shadow_detail::parseShadowColor(p, s.color))
            {
                haveColor = true;
                continue;
            }
            return false;
        }
        if (nlen < 2)
        {
            return false;
        }
        if (nlen >= 3 && lens[2] < 0.0f)
        {
            return false;
        }
        s.offsetX.value = lens[0];
        s.offsetY.value = lens[1];
        if (nlen >= 3)
        {
            s.blur.value = lens[2];
        }
        if (nlen >= 4)
        {
            s.spread.value = lens[3];
        }
        tmp.boxShadowCount++;
        shadow_detail::skipWs(p);
        if (*p == '\0')
        {
            break;
        }
        if (*p != ',')
        {
            return false;
        }
        p++;
        shadow_detail::skipWs(p);
        if (*p == '\0')
        {
            return false;
        }
    }
    out = tmp;
    return true;
}

// Outset extent of a resolved shadow list: how far pixels can spill past
// each border-box edge (left, top, right, bottom). Inset shadows never
// spill. Lengths must already be resolved to px.
inline void shadowOutsetExtent(const BoxShadow* shadows, int count,
                               float& l, float& t, float& r, float& b)
{
    l = t = r = b = 0.0f;
    for (int i = 0; i < count; i++)
    {
        const BoxShadow& s = shadows[i];
        if (s.inset)
        {
            continue;
        }
        // Lengths here are already resolved to px at flatten; unresolved
        // units fall back to their raw value (same convention as radii).
        float ox = s.offsetX.value;
        float oy = s.offsetY.value;
        float reach = s.blur.value + s.spread.value;
        if (s.blur.value < 0.0f)
        {
            reach = s.spread.value;
        }
        float sl = (ox < 0.0f ? -ox : 0.0f) + reach;
        float st = (oy < 0.0f ? -oy : 0.0f) + reach;
        float sr = (ox > 0.0f ? ox : 0.0f) + reach;
        float sb = (oy > 0.0f ? oy : 0.0f) + reach;
        if (sl > l)
        {
            l = sl;
        }
        if (st > t)
        {
            t = st;
        }
        if (sr > r)
        {
            r = sr;
        }
        if (sb > b)
        {
            b = sb;
        }
    }
}

} // namespace morph

#endif
