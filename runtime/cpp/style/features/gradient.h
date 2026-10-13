#pragma once
#include <cmath>
#include <cstring>
#include <string>
#include <vector>

#ifdef MORPH_FEATURE_GRADIENT

inline constexpr int MORPH_GRADIENT_MAX_STOPS = 8;

struct GradientStopData
{
    float color[4] = {0, 0, 0, 0};
    float posValue = 0.0f;
    bool posIsPx = false;
    bool posIsAuto = true;
    float hint = -1.0f;
};

struct BgGradient
{
    bool enabled = false;
    bool repeating = false;
    // 0 = linear, 1 = radial, 2 = conic.
    int kind = 0;
    float angleDeg = 180.0f;
    bool isCorner = false;
    float cornerX = 0.0f;
    float cornerY = 0.0f;
    // Radial/conic center: percent number, or px when *Px is set.
    float centerX = 50.0f;
    float centerY = 50.0f;
    bool centerXPx = false;
    bool centerYPx = false;
    // Radial shape: 0 = circle, 1 = ellipse.
    int radialShape = 0;
    // Radial size: 0 = closest-side, 1 = closest-corner, 2 = farthest-side,
    // 3 = farthest-corner, 4 = explicit lengths below.
    int radialSize = 3;
    // Explicit radii (percent number, or px when *Pct is false).
    float radialRX = 0.0f;
    float radialRY = 0.0f;
    bool radialRXPct = true;
    bool radialRYPct = true;
    bool radialExplicit = false;
    // Conic start angle in degrees (CSS: 0 = up, clockwise).
    float conicFrom = 0.0f;
    int stopCount = 0;
    GradientStopData stops[MORPH_GRADIENT_MAX_STOPS];
};

struct GradientStyle
{
    BgGradient bgGradient;
    // True when the style explicitly specifies the background gradient
    // (present or deliberately removed). Deltas start false, so a hover
    // rule that never mentions backgrounds can never wipe the base
    // gradient — only an explicit removal clears it.
    bool bgGradientSet = false;
    // Border-image gradient (`border-image: <gradient> ...`). Separate
    // from the background gradient so a box can carry both at once.
    BgGradient borderGradient;
    bool borderGradientSet = false;
};

namespace morph
{

inline bool gradientsEqual(const BgGradient& a, const BgGradient& b)
{
    if (a.enabled != b.enabled)
    {
        return false;
    }
    if (!a.enabled)
    {
        return true;
    }
    if (a.repeating != b.repeating || a.isCorner != b.isCorner ||
        a.stopCount != b.stopCount || a.kind != b.kind)
    {
        return false;
    }
    if (a.kind == 1 || a.kind == 2)
    {
        if (a.centerX != b.centerX || a.centerY != b.centerY ||
            a.centerXPx != b.centerXPx || a.centerYPx != b.centerYPx)
        {
            return false;
        }
    }
    if (a.kind == 1)
    {
        if (a.radialShape != b.radialShape || a.radialSize != b.radialSize ||
            a.radialExplicit != b.radialExplicit)
        {
            return false;
        }
        if (a.radialExplicit &&
            (a.radialRX != b.radialRX || a.radialRY != b.radialRY ||
             a.radialRXPct != b.radialRXPct || a.radialRYPct != b.radialRYPct))
        {
            return false;
        }
    }
    if (a.kind == 2 && a.conicFrom != b.conicFrom)
    {
        return false;
    }
    if (a.isCorner)
    {
        if (a.cornerX != b.cornerX || a.cornerY != b.cornerY)
        {
            return false;
        }
    }
    else if (a.angleDeg != b.angleDeg)
    {
        return false;
    }
    for (int i = 0; i < a.stopCount; i++)
    {
        const GradientStopData& sa = a.stops[i];
        const GradientStopData& sb = b.stops[i];
        if (std::memcmp(sa.color, sb.color, sizeof(float) * 4) != 0 ||
            sa.posIsPx != sb.posIsPx || sa.posIsAuto != sb.posIsAuto ||
            sa.hint != sb.hint)
        {
            return false;
        }
        if (!sa.posIsAuto && sa.posValue != sb.posValue)
        {
            return false;
        }
    }
    return true;
}

// True when two gradients can be interpolated stop-by-stop (same structure).
inline bool gradientsCompatible(const BgGradient& a, const BgGradient& b)
{
    if (!a.enabled || !b.enabled || a.repeating != b.repeating ||
        a.isCorner != b.isCorner || a.stopCount != b.stopCount || a.kind != b.kind)
    {
        return false;
    }
    for (int i = 0; i < a.stopCount; i++)
    {
        if (a.stops[i].posIsPx != b.stops[i].posIsPx ||
            a.stops[i].posIsAuto != b.stops[i].posIsAuto)
        {
            return false;
        }
    }
    return true;
}

// Interpolate two gradients for hover/active transitions. Structurally
// different gradients snap to the target (browsers snap too — CSS cannot
// interpolate across stop-count changes).
inline void lerpGradient(BgGradient& out, const BgGradient& a,
                         const BgGradient& b, float t)
{
    if (!a.enabled && !b.enabled)
    {
        out.enabled = false;
        return;
    }
    if (!gradientsCompatible(a, b))
    {
        out = b;
        return;
    }
    out.enabled = true;
    out.repeating = b.repeating;
    out.isCorner = b.isCorner;
    out.stopCount = b.stopCount;
    out.kind = b.kind;
    out.centerX = a.centerX + (b.centerX - a.centerX) * t;
    out.centerY = a.centerY + (b.centerY - a.centerY) * t;
    out.centerXPx = b.centerXPx;
    out.centerYPx = b.centerYPx;
    out.radialShape = b.radialShape;
    out.radialSize = b.radialSize;
    out.radialRX = a.radialRX + (b.radialRX - a.radialRX) * t;
    out.radialRY = a.radialRY + (b.radialRY - a.radialRY) * t;
    out.radialRXPct = b.radialRXPct;
    out.radialRYPct = b.radialRYPct;
    out.radialExplicit = b.radialExplicit;
    out.conicFrom = a.conicFrom + (b.conicFrom - a.conicFrom) * t;
    if (b.isCorner)
    {
        out.cornerX = b.cornerX;
        out.cornerY = b.cornerY;
    }
    else
    {
        out.angleDeg = a.angleDeg + (b.angleDeg - a.angleDeg) * t;
    }
    for (int i = 0; i < b.stopCount; i++)
    {
        GradientStopData& so = out.stops[i];
        const GradientStopData& sa = a.stops[i];
        const GradientStopData& sb = b.stops[i];
        for (int c = 0; c < 4; c++)
        {
            so.color[c] = sa.color[c] + (sb.color[c] - sa.color[c]) * t;
        }
        so.posIsPx = sb.posIsPx;
        so.posIsAuto = sb.posIsAuto;
        so.hint = sb.hint;
        so.posValue = sa.posValue + (sb.posValue - sa.posValue) * t;
    }
}

// Resolve a radial ending shape to pixel radii for a w×h box whose
// gradient center sits at (cx, cy) px. Keyword sizes measure to sides
// (circle) or sides per axis (ellipse); corner keywords measure to the
// nearest/farthest corner. Explicit lengths resolve % against the box
// (circle % against the smaller dimension), px absolute.
inline void resolveRadialRadii(const BgGradient& g, float w, float h,
                               float cx, float cy, float& rx, float& ry)
{
    if (g.radialExplicit)
    {
        rx = g.radialRXPct ? (g.radialRX / 100.0f * (g.radialShape == 0 ? (w < h ? w : h) : w))
                           : g.radialRX;
        ry = g.radialRYPct ? (g.radialRY / 100.0f * h) : g.radialRY;
        if (g.radialShape == 0)
        {
            ry = rx;
        }
        return;
    }
    float dxL = cx, dxR = w - cx, dyT = cy, dyB = h - cy;
    if (g.radialShape == 0)
    {
        switch (g.radialSize)
        {
        case 0:
            rx = dxL;
            if (dxR < rx) rx = dxR;
            if (dyT < rx) rx = dyT;
            if (dyB < rx) rx = dyB;
            break;
        case 1:
        case 3: {
            float best = -1.0f;
            bool first = true;
            float dxs[2] = {dxL, dxR};
            float dys[2] = {dyT, dyB};
            for (int ix = 0; ix < 2; ix++)
            {
                for (int iy = 0; iy < 2; iy++)
                {
                    float d = std::sqrt(dxs[ix] * dxs[ix] + dys[iy] * dys[iy]);
                    if (first || (g.radialSize == 1 ? d < best : d > best))
                    {
                        best = d;
                        first = false;
                    }
                }
            }
            rx = (best < 0.0f) ? 0.0f : best;
            break;
        }
        case 2:
        default:
            rx = dxL;
            if (dxR > rx) rx = dxR;
            if (dyT > rx) rx = dyT;
            if (dyB > rx) rx = dyB;
            break;
        }
        ry = rx;
        return;
    }
    // Ellipse.
    switch (g.radialSize)
    {
    case 0:
        rx = (dxL < dxR) ? dxL : dxR;
        ry = (dyT < dyB) ? dyT : dyB;
        break;
    case 2:
    default:
        rx = (dxL > dxR) ? dxL : dxR;
        ry = (dyT > dyB) ? dyT : dyB;
        break;
    case 1:
    case 3: {
        float bestDx = 0.0f, bestDy = 0.0f, best = -1.0f;
        bool first = true;
        float dxs[2] = {dxL, dxR};
        float dys[2] = {dyT, dyB};
        for (int ix = 0; ix < 2; ix++)
        {
            for (int iy = 0; iy < 2; iy++)
            {
                float d = std::sqrt(dxs[ix] * dxs[ix] + dys[iy] * dys[iy]);
                if (first || (g.radialSize == 1 ? d < best : d > best))
                {
                    best = d;
                    bestDx = dxs[ix];
                    bestDy = dys[iy];
                    first = false;
                }
            }
        }
        rx = bestDx;
        ry = bestDy;
        break;
    }
    }
}

// Resolve stop positions to 0..1 fractions of the gradient line.
// lineLenPx is the box extent projected on the gradient axis (pixels).
// Percentages divide by 100, pixels divide by lineLenPx, autos distribute
// evenly between their positioned neighbors (CSS Images 4 §4.2).
inline void resolveGradientOffsets(const BgGradient& g, float lineLenPx,
                                   float out[MORPH_GRADIENT_MAX_STOPS])
{
    int n = g.stopCount;
    if (n <= 0)
    {
        return;
    }
    float pos[MORPH_GRADIENT_MAX_STOPS];
    for (int i = 0; i < n; i++)
    {
        const GradientStopData& s = g.stops[i];
        if (s.posIsAuto)
        {
            pos[i] = -1.0f;
        }
        else if (s.posIsPx)
        {
            pos[i] = (lineLenPx > 1e-6f) ? (s.posValue / lineLenPx) : 0.0f;
        }
        else
        {
            pos[i] = s.posValue / 100.0f;
        }
    }
    if (pos[0] < -0.5f)
    {
        pos[0] = 0.0f;
    }
    if (pos[n - 1] < -0.5f)
    {
        pos[n - 1] = 1.0f;
    }
    int runStart = 0;
    while (runStart < n)
    {
        if (pos[runStart] > -0.5f)
        {
            runStart++;
            continue;
        }
        int runEnd = runStart + 1;
        while (runEnd < n && pos[runEnd] < -0.5f)
        {
            runEnd++;
        }
        float before = pos[runStart - 1];
        float after = (runEnd < n) ? pos[runEnd] : before;
        int runLen = runEnd - runStart;
        for (int k = 0; k < runLen; k++)
        {
            pos[runStart + k] =
                before + (after - before) * (float)(k + 1) / (float)(runLen + 1);
        }
        runStart = runEnd;
    }
    for (int i = 0; i < n; i++)
    {
        out[i] = pos[i];
    }
}

namespace grad_detail
{

inline void skipWs(const char*& p)
{
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r')
    {
        p++;
    }
}

inline bool parseGradNumber(const char*& p, float& out)
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

inline bool parseGradColor(const char*& p, float c[4])
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
        if (*p == 'a')
        {
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
            if (!parseGradNumber(p, comp[i]))
            {
                return false;
            }
            skipWs(p);
            if (*p == '%')
            {
                comp[i] = comp[i] / 100.0f;
                if (i < 3)
                {
                    comp[i] = comp[i] * 1.0f;
                }
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
    char word[32];
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
    std::string w = word;
    for (char& ch : w)
    {
        ch = (char)std::tolower((unsigned char)ch);
    }
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
    return false;
}

} // namespace grad_detail

namespace grad_detail
{

/// Read a lowercase dash-word (`closest-side`, `at`, ...).
inline bool readGradWord(const char*& p, std::string& out)
{
    skipWs(p);
    const char* start = p;
    while ((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z') || *p == '-')
    {
        p++;
    }
    if (p == start)
    {
        return false;
    }
    out.assign(start, p);
    for (char& ch : out)
    {
        ch = (char)std::tolower((unsigned char)ch);
    }
    return true;
}

/// Parse `50%` / `10px` / bare `0` into value + is-percent flag.
inline bool parseGradLengthPct(const char*& p, float& v, bool& pct)
{
    const char* save = p;
    float num = 0.0f;
    if (!parseGradNumber(p, num))
    {
        return false;
    }
    skipWs(p);
    if (*p == '%')
    {
        v = num;
        pct = true;
        p++;
        return true;
    }
    const char* usave = p;
    std::string u;
    if (readGradWord(p, u) && u == "px")
    {
        v = num;
        pct = false;
        return true;
    }
    p = usave;
    if (num == 0.0f)
    {
        v = 0.0f;
        pct = false;
        return true;
    }
    p = save;
    return false;
}

/// Parse a center position (1–2 of keywords / `%` / px). Defaults stay
/// 50/50; partial input keeps what parsed (mirrors the Rust parser,
/// which fills missing axes with center).
inline bool parseGradCenter(const char*& p, float& cx, float& cy,
                            bool& cxPx, bool& cyPx)
{
    bool haveX = false, haveY = false;
    for (int k = 0; k < 2; k++)
    {
        const char* save = p;
        std::string w;
        if (readGradWord(p, w))
        {
            if (w == "left" && !haveX)
            {
                cx = 0.0f;
                cxPx = false;
                haveX = true;
                continue;
            }
            if (w == "right" && !haveX)
            {
                cx = 100.0f;
                cxPx = false;
                haveX = true;
                continue;
            }
            if (w == "top" && !haveY)
            {
                cy = 0.0f;
                cyPx = false;
                haveY = true;
                continue;
            }
            if (w == "bottom" && !haveY)
            {
                cy = 100.0f;
                cyPx = false;
                haveY = true;
                continue;
            }
            if (w == "center")
            {
                if (!haveX)
                {
                    haveX = true;
                    continue;
                }
                if (!haveY)
                {
                    haveY = true;
                    continue;
                }
            }
            p = save;
            return haveX || haveY;
        }
        float v = 0.0f;
        bool pct = true;
        if (!parseGradLengthPct(p, v, pct))
        {
            p = save;
            return haveX || haveY;
        }
        if (!haveX)
        {
            cx = v;
            cxPx = !pct;
            haveX = true;
        }
        else if (!haveY)
        {
            cy = v;
            cyPx = !pct;
            haveY = true;
        }
        else
        {
            p = save;
            return false;
        }
    }
    return haveX || haveY;
}

/// Angle value in degrees (`45deg`, `0.25turn`, `100grad`, `1rad`).
inline bool parseGradAngle(const char*& p, float& deg)
{
    float num = 0.0f;
    if (!parseGradNumber(p, num))
    {
        return false;
    }
    skipWs(p);
    std::string u;
    if (!readGradWord(p, u))
    {
        return false;
    }
    if (u == "deg")
    {
        deg = num;
    }
    else if (u == "turn")
    {
        deg = num * 360.0f;
    }
    else if (u == "grad")
    {
        deg = num * 0.9f;
    }
    else if (u == "rad")
    {
        deg = num * 180.0f / 3.14159265f;
    }
    else
    {
        return false;
    }
    return true;
}

/// Radial prelude: `[shape||size] [at pos]`. Returns true when a header
/// was consumed (p ends past its comma); false leaves p untouched for
/// stops. Mirrors the Rust prelude rules.
inline bool parseRadialPrelude(const char*& p, BgGradient& g)
{
    // Peek: a prelude starts with a shape/size keyword, `at`, or length.
    const char* peek = p;
    skipWs(peek);
    std::string w;
    bool isWord = readGradWord(peek, w);
    bool looksPrelude = false;
    if (isWord)
    {
        looksPrelude = (w == "circle" || w == "ellipse" || w == "closest-side" ||
                        w == "closest-corner" || w == "farthest-side" ||
                        w == "farthest-corner" || w == "at");
    }
    else
    {
        float v = 0.0f;
        bool pct = true;
        looksPrelude = parseGradLengthPct(peek, v, pct);
    }
    if (!looksPrelude)
    {
        return false;
    }
    // Commit: strict from here — garbage fails the whole value.
    int shape = 0;
    bool shapeSeen = false;
    int size = -1;
    float lx[2] = {0, 0};
    bool lpct[2] = {true, true};
    int nlen = 0;
    for (int k = 0; k < 4; k++)
    {
        skipWs(p);
        if (*p == ',' || *p == ')')
        {
            break;
        }
        const char* wsave = p;
        std::string word;
        if (readGradWord(p, word))
        {
            if (word == "at")
            {
                // Rewind: the `at` arm below consumes it.
                p = wsave;
                break;
            }
            if (word == "circle" && !shapeSeen)
            {
                shape = 0;
                shapeSeen = true;
                continue;
            }
            if (word == "ellipse" && !shapeSeen)
            {
                shape = 1;
                shapeSeen = true;
                continue;
            }
            if (word == "closest-side" && size < 0)
            {
                size = 0;
                continue;
            }
            if (word == "closest-corner" && size < 0)
            {
                size = 1;
                continue;
            }
            if (word == "farthest-side" && size < 0)
            {
                size = 2;
                continue;
            }
            if (word == "farthest-corner" && size < 0)
            {
                size = 3;
                continue;
            }
            return false;
        }
        float v = 0.0f;
        bool pct = true;
        if (!parseGradLengthPct(p, v, pct) || nlen >= 2)
        {
            return false;
        }
        lx[nlen] = v;
        lpct[nlen] = pct;
        nlen++;
    }
    if (nlen == 1)
    {
        g.radialShape = shape;
        g.radialSize = 4;
        g.radialExplicit = true;
        g.radialRX = lx[0];
        g.radialRXPct = lpct[0];
        g.radialRY = lx[0];
        g.radialRYPct = lpct[0];
    }
    else if (nlen == 2)
    {
        if (size >= 0)
        {
            return false;
        }
        if (shapeSeen && shape == 0)
        {
            return false;
        }
        g.radialShape = 1;
        g.radialSize = 4;
        g.radialExplicit = true;
        g.radialRX = lx[0];
        g.radialRXPct = lpct[0];
        g.radialRY = lx[1];
        g.radialRYPct = lpct[1];
    }
    else
    {
        g.radialShape = shape;
        g.radialSize = (size >= 0) ? size : 3;
        g.radialExplicit = false;
    }
    skipWs(p);
    // Optional `at <position>`.
    const char* asave = p;
    std::string aw;
    if (readGradWord(p, aw))
    {
        if (aw != "at")
        {
            return false;
        }
        if (!parseGradCenter(p, g.centerX, g.centerY, g.centerXPx, g.centerYPx))
        {
            return false;
        }
    }
    else
    {
        p = asave;
    }
    skipWs(p);
    if (*p != ',')
    {
        return false;
    }
    p++;
    return true;
}

/// Conic prelude: `[from <angle>] [at <pos>]`. Same consume contract.
inline bool parseConicPrelude(const char*& p, BgGradient& g)
{
    const char* peek = p;
    skipWs(peek);
    std::string w;
    if (!readGradWord(peek, w) || (w != "from" && w != "at"))
    {
        return false;
    }
    if (w == "from")
    {
        readGradWord(p, w);
        float deg = 0.0f;
        if (!parseGradAngle(p, deg))
        {
            return false;
        }
        g.conicFrom = deg;
    }
    skipWs(p);
    const char* asave = p;
    std::string aw;
    if (readGradWord(p, aw))
    {
        if (aw != "at")
        {
            return false;
        }
        if (!parseGradCenter(p, g.centerX, g.centerY, g.centerXPx, g.centerYPx))
        {
            return false;
        }
    }
    else
    {
        p = asave;
    }
    skipWs(p);
    if (*p != ',')
    {
        return false;
    }
    p++;
    return true;
}

} // namespace grad_detail

// Parse a `linear-gradient(...)` / `repeating-linear-gradient(...)` CSS
// string (keyframe payloads). Mirrors the Rust parser in
// `crates/morph-ir/src/gradient.rs`; returns false when the value is not a
// supported single-layer linear gradient (style left unchanged).
inline bool parseGradientCss(const std::string& css, BgGradient& out)
{
    const char* p = css.c_str();
    grad_detail::skipWs(p);
    bool repeating = false;
    int kind = 0;
    if (std::strncmp(p, "repeating-linear-gradient", 25) == 0)
    {
        repeating = true;
        p += 25;
    }
    else if (std::strncmp(p, "linear-gradient", 15) == 0)
    {
        p += 15;
    }
    else if (std::strncmp(p, "repeating-radial-gradient", 25) == 0)
    {
        repeating = true;
        kind = 1;
        p += 25;
    }
    else if (std::strncmp(p, "radial-gradient", 15) == 0)
    {
        kind = 1;
        p += 15;
    }
    else if (std::strncmp(p, "repeating-conic-gradient", 24) == 0)
    {
        repeating = true;
        kind = 2;
        p += 24;
    }
    else if (std::strncmp(p, "conic-gradient", 14) == 0)
    {
        kind = 2;
        p += 14;
    }
    else
    {
        return false;
    }
    grad_detail::skipWs(p);
    if (*p != '(')
    {
        return false;
    }
    p++;
    BgGradient g;
    g.enabled = true;
    g.repeating = repeating;
    g.kind = kind;
    if (kind == 1)
    {
        if (!grad_detail::parseRadialPrelude(p, g))
        {
            // No header: defaults already in place, stops follow.
            // (parseRadialPrelude only consumes on success.)
        }
    }
    else if (kind == 2)
    {
        if (!grad_detail::parseConicPrelude(p, g))
        {
            // Same: defaults stand, stops follow.
        }
    }
    // Optional first arg: direction or angle (linear only — radial
    // and conic preludes were handled above, and anything else there
    // is a stop).
    const char* save = p;
    grad_detail::skipWs(p);
    bool hasAxis = false;
    if (kind == 0 && std::strncmp(p, "to ", 3) == 0)
    {
        p += 3;
        float hx = 0.0f, hy = 0.0f;
        // One or two side keywords (`to right` or `to top right`).
        for (int k = 0; k < 2; k++)
        {
            grad_detail::skipWs(p);
            const char* wsave = p;
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
            std::string w = word;
            for (char& ch : w)
            {
                ch = (char)std::tolower((unsigned char)ch);
            }
            bool known = (w == "left" || w == "right" || w == "top" || w == "bottom");
            if (!known)
            {
                return false;
            }
            if (w == "left")
            {
                hx = -1.0f;
            }
            else if (w == "right")
            {
                hx = 1.0f;
            }
            else if (w == "top")
            {
                hy = -1.0f;
            }
            else
            {
                hy = 1.0f;
            }
            // Single side when no second side keyword follows: peek the
            // next word — a second `left|right|top|bottom` continues.
            const char* peek = p;
            grad_detail::skipWs(peek);
            char pword[16];
            size_t pwi = 0;
            while ((*peek >= 'a' && *peek <= 'z') || (*peek >= 'A' && *peek <= 'Z'))
            {
                if (pwi + 1 < sizeof(pword))
                {
                    pword[pwi++] = *peek;
                }
                peek++;
            }
            pword[pwi] = '\0';
            std::string pw = pword;
            for (char& ch : pw)
            {
                ch = (char)std::tolower((unsigned char)ch);
            }
            if (pw != "left" && pw != "right" && pw != "top" && pw != "bottom")
            {
                break;
            }
            (void)wsave;
        }
        if (hx != 0.0f && hy != 0.0f)
        {
            g.isCorner = true;
            g.cornerX = hx;
            g.cornerY = hy;
        }
        else if (hx > 0.0f)
        {
            g.angleDeg = 90.0f;
        }
        else if (hx < 0.0f)
        {
            g.angleDeg = 270.0f;
        }
        else if (hy > 0.0f)
        {
            g.angleDeg = 180.0f;
        }
        else
        {
            g.angleDeg = 0.0f;
        }
        hasAxis = true;
    }
    else if (kind == 0)
    {
        float num = 0.0f;
        const char* numStart = p;
        if (grad_detail::parseGradNumber(p, num))
        {
            grad_detail::skipWs(p);
            char unit[8];
            size_t ui = 0;
            while ((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z') ||
                   *p == '%')
            {
                if (ui + 1 < sizeof(unit))
                {
                    unit[ui++] = *p;
                }
                p++;
            }
            unit[ui] = '\0';
            std::string u = unit;
            for (char& ch : u)
            {
                ch = (char)std::tolower((unsigned char)ch);
            }
            if (u == "deg")
            {
                g.angleDeg = num;
            }
            else if (u == "turn")
            {
                g.angleDeg = num * 360.0f;
            }
            else if (u == "grad")
            {
                g.angleDeg = num * 0.9f;
            }
            else if (u == "rad")
            {
                g.angleDeg = num * 180.0f / 3.14159265f;
            }
            else
            {
                return false;
            }
            hasAxis = true;
        }
        else
        {
            p = numStart;
        }
    }
    if (hasAxis)
    {
        grad_detail::skipWs(p);
        if (*p != ',')
        {
            return false;
        }
        p++;
    }
    else
    {
        p = save;
    }
    // Stops: color [pos [pos]] | bare hint (applies to next stop).
    float pendingHint = -1.0f;
    while (true)
    {
        grad_detail::skipWs(p);
        if (*p == ')')
        {
            p++;
            break;
        }
        if (g.stopCount >= MORPH_GRADIENT_MAX_STOPS)
        {
            return false;
        }
        // Bare position = color hint for the next stop.
        const char* hintSave = p;
        float hv = 0.0f;
        if (grad_detail::parseGradNumber(p, hv))
        {
            grad_detail::skipWs(p);
            if (*p == '%')
            {
                p++;
                grad_detail::skipWs(p);
                if (*p == ',' || *p == ')')
                {
                    if (g.stopCount == 0)
                    {
                        return false;
                    }
                    pendingHint = hv / 100.0f;
                    if (*p == ',')
                    {
                        p++;
                    }
                    continue;
                }
            }
            p = hintSave;
        }
        GradientStopData s;
        if (!grad_detail::parseGradColor(p, s.color))
        {
            return false;
        }
        s.hint = pendingHint;
        pendingHint = -1.0f;
        // Up to two positions (double position = solid block; the runtime
        // shader consumes a single offset per stop, so keep the first and
        // let the second extend via the next stop — instead expand here).
        float pos[2] = {0, 0};
        bool isPx[2] = {false, false};
        int npos = 0;
        while (npos < 2)
        {
            const char* psave = p;
            float v = 0.0f;
            if (!grad_detail::parseGradNumber(p, v))
            {
                break;
            }
            grad_detail::skipWs(p);
            if (*p == '%')
            {
                pos[npos] = v;
                isPx[npos] = false;
                p++;
            }
            else if ((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z'))
            {
                char unit[8];
                size_t ui = 0;
                while ((*p >= 'a' && *p <= 'z') || (*p >= 'A' && *p <= 'Z'))
                {
                    if (ui + 1 < sizeof(unit))
                    {
                        unit[ui++] = *p;
                    }
                    p++;
                }
                unit[ui] = '\0';
                std::string u = unit;
                for (char& ch : u)
                {
                    ch = (char)std::tolower((unsigned char)ch);
                }
                if (u != "px")
                {
                    // Conic dial positions accept angles (px is meaningless
                    // on a dial and rejected, mirroring the Rust parser).
                    float frac = -1.0f;
                    if (kind == 2 && u == "deg")
                    {
                        frac = v / 360.0f;
                    }
                    else if (kind == 2 && u == "turn")
                    {
                        frac = v;
                    }
                    else if (kind == 2 && u == "grad")
                    {
                        frac = v * 0.9f / 360.0f;
                    }
                    else if (kind == 2 && u == "rad")
                    {
                        frac = v / 6.2831853f;
                    }
                    else
                    {
                        p = psave;
                        break;
                    }
                    pos[npos] = frac * 100.0f;
                    isPx[npos] = false;
                }
                else if (kind == 2)
                {
                    // px on a conic dial: reject the whole value.
                    return false;
                }
                else
                {
                    pos[npos] = v;
                    isPx[npos] = true;
                }
            }
            else if (v == 0.0f)
            {
                // Bare unitless zero is a valid <length>.
                pos[npos] = 0.0f;
                isPx[npos] = true;
            }
            else
            {
                p = psave;
                break;
            }
            npos++;
        }
        if (npos == 0)
        {
            s.posIsAuto = true;
            g.stops[g.stopCount++] = s;
        }
        else
        {
            // Expand double positions into two stops sharing the color.
            for (int k = 0; k < npos; k++)
            {
                if (g.stopCount >= MORPH_GRADIENT_MAX_STOPS)
                {
                    return false;
                }
                GradientStopData e = s;
                e.posIsAuto = false;
                e.posValue = pos[k];
                e.posIsPx = isPx[k];
                e.hint = (k == 0) ? s.hint : -1.0f;
                g.stops[g.stopCount++] = e;
            }
        }
        (void)hintSave;
        grad_detail::skipWs(p);
        if (*p == ',')
        {
            p++;
        }
        else if (*p != ')')
        {
            return false;
        }
    }
    if (g.stopCount < 2)
    {
        return false;
    }
    out = g;
    return true;
}

} // namespace morph

#endif // MORPH_FEATURE_GRADIENT
