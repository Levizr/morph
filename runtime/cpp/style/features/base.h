#pragma once
#include <cmath>
#include <cctype>
#include <cstdint>
#include <cstdlib>
#include <string>

#include "../css_enums.h"

// CSS length units with browser-style bases. Absolute physical units
// (`in/cm/mm/q/pt/pc`) are folded to px at parse time; everything here
// resolves at layout/paint against its own base:
//   Px        absolute pixels
//   Pct       containing-block dimension (`pctBase`)
//   Em/Ex/Ch  element's computed font size (`fontSize`, measured `chW`/`exW`)
//   Rem       root element's computed font size (`rootFont`)
//   Vw/Vh/Vmin/Vmax  layout viewport (`vw`/`vh`)
enum class LengthUnit : uint8_t
{
    Px = 0,
    Pct,
    Em,
    Rem,
    Vw,
    Vh,
    Vmin,
    Vmax,
    Ch,
    Ex
};

// A CSS length: px value plus its unit. Unset is value <= -1e8f, which
// also covers the legacy -1.0f explicit-dimension sentinel.
struct CssLength
{
    float value = -1e9f;
    LengthUnit unit = LengthUnit::Px;
    bool isSet() const { return value > -1e8f; }
    bool isPercent() const { return unit == LengthUnit::Pct; }
    // Explicit zero without a relative unit (fast-path decoration checks).
    bool isZero() const { return unit == LengthUnit::Px && value == 0.0f; }
    bool operator==(const CssLength& o) const
    {
        return value == o.value && unit == o.unit;
    }
    bool operator!=(const CssLength& o) const { return !(*this == o); }
};

inline CssLength pxLen(float v)
{
    CssLength l;
    l.value = v;
    l.unit = LengthUnit::Px;
    return l;
}

// Resolution context for one element: `%` needs the caller's base,
// font units the (already computed) font sizes, viewport units the
// window size that flowed down through m_winW/m_winH at layout.
struct UnitEnv
{
    float pctBase = 0.0f;
    float fontSize = 16.0f;
    float rootFont = 16.0f;
    float vw = 0.0f;
    float vh = 0.0f;
    // Measured `0`/`x` advances at fontSize; < 0 selects the 0.5em fallback
    // (headless measure passes with no renderer).
    float chW = -1.0f;
    float exW = -1.0f;
};

inline float resolveUnits(const CssLength& l, const UnitEnv& e)
{
    switch (l.unit)
    {
        case LengthUnit::Pct: return l.value * 0.01f * e.pctBase;
        case LengthUnit::Em: return l.value * e.fontSize;
        case LengthUnit::Rem: return l.value * e.rootFont;
        case LengthUnit::Vw: return l.value * 0.01f * e.vw;
        case LengthUnit::Vh: return l.value * 0.01f * e.vh;
        case LengthUnit::Vmin:
            return l.value * 0.01f * (e.vw < e.vh ? e.vw : e.vh);
        case LengthUnit::Vmax:
            return l.value * 0.01f * (e.vw > e.vh ? e.vw : e.vh);
        case LengthUnit::Ch:
            return l.value * (e.chW >= 0.0f ? e.chW : 0.5f * e.fontSize);
        case LengthUnit::Ex:
            return l.value * (e.exW >= 0.0f ? e.exW : 0.5f * e.fontSize);
        case LengthUnit::Px:
        default: return l.value;
    }
}

// Parse a CSS length string into a CssLength. Mirrors the IR builder's
// `parse_length` table: px, bare numbers (= px), `%`, font-relative
// (`em/rem/ex/ch`), viewport (`vw/vh/vmin/vmax` + `sv*/lv*/dv*`
// aliases), absolute physical units folded to px at 96dpi
// (`in/cm/mm/q/pt/pc`). Case-insensitive; a trailing digit guard keeps
// bare words like `system` from matching the `em` suffix. Returns false
// for keywords (`auto`) and garbage.
inline bool parseCssLength(const std::string& s, CssLength& out)
{
    size_t b = 0, e = s.size();
    while (b < e && isspace((unsigned char)s[b])) b++;
    while (e > b && isspace((unsigned char)s[e - 1])) e--;
    if (b >= e) return false;
    std::string t = s.substr(b, e - b);
    if (!t.empty() && t.back() == '%') {
        char* end = nullptr;
        double v = strtod(t.c_str(), &end);
        if (end == t.c_str()) return false;
        while (*end && isspace((unsigned char)*end)) end++;
        if (*end != '%') return false;
        out.value = (float)v;
        out.unit = LengthUnit::Pct;
        return true;
    }
    std::string low = t;
    for (char& c : low) c = (char)tolower((unsigned char)c);
    struct UnitRow { const char* suffix; float scale; LengthUnit unit; };
    static const UnitRow table[] = {
        {"vmin", 1.0f, LengthUnit::Vmin}, {"vmax", 1.0f, LengthUnit::Vmax},
        {"svmin", 1.0f, LengthUnit::Vmin}, {"svmax", 1.0f, LengthUnit::Vmax},
        {"lvmin", 1.0f, LengthUnit::Vmin}, {"lvmax", 1.0f, LengthUnit::Vmax},
        {"dvmin", 1.0f, LengthUnit::Vmin}, {"dvmax", 1.0f, LengthUnit::Vmax},
        {"svh", 1.0f, LengthUnit::Vh}, {"lvh", 1.0f, LengthUnit::Vh},
        {"dvh", 1.0f, LengthUnit::Vh}, {"svw", 1.0f, LengthUnit::Vw},
        {"lvw", 1.0f, LengthUnit::Vw}, {"dvw", 1.0f, LengthUnit::Vw},
        {"rem", 1.0f, LengthUnit::Rem},
        {"vh", 1.0f, LengthUnit::Vh}, {"vw", 1.0f, LengthUnit::Vw},
        {"em", 1.0f, LengthUnit::Em}, {"ex", 1.0f, LengthUnit::Ex},
        {"ch", 1.0f, LengthUnit::Ch}, {"px", 1.0f, LengthUnit::Px},
        {"in", 96.0f, LengthUnit::Px}, {"cm", 96.0f / 2.54f, LengthUnit::Px},
        {"mm", 96.0f / 25.4f, LengthUnit::Px}, {"pt", 96.0f / 72.0f, LengthUnit::Px},
        {"pc", 16.0f, LengthUnit::Px}, {"q", 96.0f / 101.6f, LengthUnit::Px},
    };
    for (const auto& row : table) {
        const std::string suf = row.suffix;
        if (low.size() > suf.size()
            && low.compare(low.size() - suf.size(), suf.size(), suf) == 0) {
            std::string num = low.substr(0, low.size() - suf.size());
            while (!num.empty() && isspace((unsigned char)num.back())) num.pop_back();
            if (num.empty()) continue;
            char last = num.back();
            if (!isdigit((unsigned char)last) && last != '.') continue;
            char* end = nullptr;
            double v = strtod(num.c_str(), &end);
            if (end == num.c_str()) continue;
            while (*end && isspace((unsigned char)*end)) end++;
            if (*end != '\0') continue;
            out.value = (float)(v * row.scale);
            out.unit = row.unit;
            return true;
        }
    }
    char* end = nullptr;
    double v = strtod(t.c_str(), &end);
    if (end == t.c_str()) return false;
    while (*end && isspace((unsigned char)*end)) end++;
    if (*end != '\0') return false;
    out.value = (float)v;
    out.unit = LengthUnit::Px;
    return true;
}

struct StyleBase {
    float bgColor[4] = {0,0,0,0};
    float color[4]   = {0,0,0,1};
    CssLength borderRadius = pxLen(0.0f);
    // Per-corner radius longhands (clockwise from top-left); unset
    // falls back to borderRadius.
    CssLength borderTopLeftRadius;
    CssLength borderTopRightRadius;
    CssLength borderBottomRightRadius;
    CssLength borderBottomLeftRadius;
    CssLength fontSize = pxLen(16.0f);
    CssLength padding[4] = {pxLen(0.0f), pxLen(0.0f), pxLen(0.0f), pxLen(0.0f)};
    CssLength margin[4] = {pxLen(0.0f), pxLen(0.0f), pxLen(0.0f), pxLen(0.0f)};
    bool marginAuto[4] = {false,false,false,false};
    CssLength explicitWidth;
    CssLength explicitHeight;
    CssLength minWidth;
    CssLength maxWidth;
    CssLength minHeight;
    CssLength maxHeight;

    CSS::FontWeight fontWeight = CSS::FontWeight::Normal;
    CSS::Overflow overflowX = CSS::Overflow::Visible;
    CSS::Overflow overflowY = CSS::Overflow::Visible;
    CSS::Display display = CSS::Display::Block;
    CSS::Position position = CSS::Position::Static;
    CSS::TextAlign textAlign = CSS::TextAlign::Left;
    CSS::BoxSizing boxSizing = CSS::BoxSizing::ContentBox;
};

// Effective corner radii in clockwise order from top-left
// (TL TR BR BL). A set longhand wins, otherwise the shorthand applies.
// Relative units resolve against the element's own box width via `env`
// (`%` of the border-box width, like Chrome's horizontal radius).
inline void resolveBorderRadii(const StyleBase& s, const UnitEnv& env, bool snap, float out[4])
{
    const CssLength* corners[4] = {&s.borderTopLeftRadius, &s.borderTopRightRadius,
                                   &s.borderBottomRightRadius, &s.borderBottomLeftRadius};
    for (int i = 0; i < 4; i++)
    {
        const CssLength& c = corners[i]->isSet() ? *corners[i] : s.borderRadius;
        out[i] = resolveUnits(c, env);
    }
    if (snap)
    {
        for (int i = 0; i < 4; i++)
        {
            out[i] = std::round(out[i]);
        }
    }
}

// Control corners (buttons, inputs): like resolveBorderRadii but an unset
// shorthand falls back to the control default instead of sharp.
inline void resolveControlRadii(const StyleBase& s, const UnitEnv& env, bool snap, float out[4])
{
    CssLength base = s.borderRadius.isZero() ? pxLen(6.0f) : s.borderRadius;
    float basePx = resolveUnits(base, env);
    const CssLength* corners[4] = {&s.borderTopLeftRadius, &s.borderTopRightRadius,
                                   &s.borderBottomRightRadius, &s.borderBottomLeftRadius};
    for (int i = 0; i < 4; i++)
    {
        out[i] = corners[i]->isSet() ? resolveUnits(*corners[i], env) : basePx;
    }
    if (snap)
    {
        for (int i = 0; i < 4; i++)
        {
            out[i] = std::round(out[i]);
        }
    }
}
