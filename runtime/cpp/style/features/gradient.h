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
    float angleDeg = 180.0f;
    bool isCorner = false;
    float cornerX = 0.0f;
    float cornerY = 0.0f;
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
        a.stopCount != b.stopCount)
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
        a.isCorner != b.isCorner || a.stopCount != b.stopCount)
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

// Parse a `linear-gradient(...)` / `repeating-linear-gradient(...)` CSS
// string (keyframe payloads). Mirrors the Rust parser in
// `crates/morph-ir/src/gradient.rs`; returns false when the value is not a
// supported single-layer linear gradient (style left unchanged).
inline bool parseGradientCss(const std::string& css, BgGradient& out)
{
    const char* p = css.c_str();
    grad_detail::skipWs(p);
    bool repeating = false;
    if (std::strncmp(p, "repeating-linear-gradient", 25) == 0)
    {
        repeating = true;
        p += 25;
    }
    else if (std::strncmp(p, "linear-gradient", 15) == 0)
    {
        p += 15;
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
    // Optional first arg: direction or angle.
    const char* save = p;
    grad_detail::skipWs(p);
    bool hasAxis = false;
    if (std::strncmp(p, "to ", 3) == 0)
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
    else
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
                    p = psave;
                    break;
                }
                pos[npos] = v;
                isPx[npos] = true;
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
