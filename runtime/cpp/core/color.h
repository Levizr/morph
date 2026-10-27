#pragma once
#include <cctype>
#include <string>

namespace morph {

// Parse the color strings produced by reactive inline-style expressions.
// Static CSS colors are resolved at build time; this only handles values that
// are not known until an effect runs. Unknown values leave the destination
// untouched and report false, matching CSS's "ignore an invalid declaration"
// behavior while letting callers preserve inherited-color markers.
inline int color_hex_nibble(char c)
{
    if (c >= '0' && c <= '9')
    {
        return c - '0';
    }
    c = (char)std::tolower((unsigned char)c);
    if (c >= 'a' && c <= 'f')
    {
        return c - 'a' + 10;
    }
    return -1;
}

inline int color_hex_byte(char hi, char lo)
{
    int h = color_hex_nibble(hi);
    int l = color_hex_nibble(lo);
    return (h < 0 || l < 0) ? -1 : h * 16 + l;
}

inline bool color_equals_ignore_case(const std::string& value, const char* keyword)
{
    size_t i = 0;
    for (; i < value.size() && keyword[i] != '\0'; ++i)
    {
        if (std::tolower((unsigned char)value[i]) != keyword[i])
        {
            return false;
        }
    }
    return i == value.size() && keyword[i] == '\0';
}

inline bool set_color(float rgba[4], const std::string& c)
{
    if (color_equals_ignore_case(c, "transparent"))
    {
        rgba[0] = rgba[1] = rgba[2] = 0.0f;
        rgba[3] = 0.0f;
        return true;
    }
    if (c.size() == 4 && c[0] == '#')
    {
        int r = color_hex_nibble(c[1]);
        int g = color_hex_nibble(c[2]);
        int b = color_hex_nibble(c[3]);
        if (r < 0 || g < 0 || b < 0)
        {
            return false;
        }
        rgba[0] = (float)(r * 17) / 255.0f;
        rgba[1] = (float)(g * 17) / 255.0f;
        rgba[2] = (float)(b * 17) / 255.0f;
        rgba[3] = 1.0f;
        return true;
    }
    if (c.size() == 5 && c[0] == '#')
    {
        int r = color_hex_nibble(c[1]);
        int g = color_hex_nibble(c[2]);
        int b = color_hex_nibble(c[3]);
        int a = color_hex_nibble(c[4]);
        if (r < 0 || g < 0 || b < 0 || a < 0)
        {
            return false;
        }
        rgba[0] = (float)(r * 17) / 255.0f;
        rgba[1] = (float)(g * 17) / 255.0f;
        rgba[2] = (float)(b * 17) / 255.0f;
        rgba[3] = (float)(a * 17) / 255.0f;
        return true;
    }
    if ((c.size() == 7 || c.size() == 9) && c[0] == '#')
    {
        int r = color_hex_byte(c[1], c[2]);
        int g = color_hex_byte(c[3], c[4]);
        int b = color_hex_byte(c[5], c[6]);
        int a = c.size() == 9 ? color_hex_byte(c[7], c[8]) : 255;
        if (r < 0 || g < 0 || b < 0 || a < 0)
        {
            return false;
        }
        rgba[0] = (float)r / 255.0f;
        rgba[1] = (float)g / 255.0f;
        rgba[2] = (float)b / 255.0f;
        rgba[3] = (float)a / 255.0f;
        return true;
    }
    return false;
}

} // namespace morph
