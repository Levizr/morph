#pragma once
// JavaScript RegExp subset: literal construction, `test`, and `exec`.
// Patterns are matched as UTF-16 code units (the unit JavaScript uses
// without the `u` flag), so `.` and quantifiers behave like JS even for
// astral characters. `morph check` rejects flags and constructs that this
// runtime cannot reproduce.
#include <cstdint>
#include <regex>
#include <string>
#include "js_types.h"

namespace morph
{
namespace regexp_detail
{
// Decode UTF-8 and encode as UTF-16 code units stored in a std::wstring.
// Invalid sequences become U+FFFD; surrogate pairs stay as two units so
// std::wregex sees the same units as a JavaScript string.
inline std::wstring utf8_to_utf16_units(const std::string& text)
{
    std::wstring out;
    out.reserve(text.size());
    size_t index = 0;
    while (index < text.size())
    {
        unsigned char first = static_cast<unsigned char>(text[index]);
        uint32_t code_point = 0;
        size_t width = 0;
        if (first < 0x80)
        {
            code_point = first;
            width = 1;
        }
        else if ((first & 0xE0) == 0xC0)
        {
            code_point = first & 0x1F;
            width = 2;
        }
        else if ((first & 0xF0) == 0xE0)
        {
            code_point = first & 0x0F;
            width = 3;
        }
        else if ((first & 0xF8) == 0xF0)
        {
            code_point = first & 0x07;
            width = 4;
        }
        else
        {
            out.push_back(static_cast<wchar_t>(0xFFFD));
            ++index;
            continue;
        }
        if (index + width > text.size())
        {
            out.push_back(static_cast<wchar_t>(0xFFFD));
            ++index;
            continue;
        }
        bool valid = true;
        for (size_t offset = 1; offset < width; ++offset)
        {
            unsigned char continuation = static_cast<unsigned char>(text[index + offset]);
            if ((continuation & 0xC0) != 0x80)
            {
                valid = false;
                break;
            }
            code_point = (code_point << 6) | (continuation & 0x3F);
        }
        if (!valid || code_point > 0x10FFFF || (code_point >= 0xD800 && code_point <= 0xDFFF))
        {
            out.push_back(static_cast<wchar_t>(0xFFFD));
            ++index;
            continue;
        }
        if (code_point >= 0x10000)
        {
            code_point -= 0x10000;
            out.push_back(static_cast<wchar_t>(0xD800 + (code_point >> 10)));
            out.push_back(static_cast<wchar_t>(0xDC00 + (code_point & 0x3FF)));
        }
        else
        {
            out.push_back(static_cast<wchar_t>(code_point));
        }
        index += width;
    }
    return out;
}

// Encode UTF-16 code units back to UTF-8. Unpaired surrogates become U+FFFD.
inline std::string utf16_units_to_utf8(const std::wstring& units)
{
    std::string out;
    out.reserve(units.size());
    size_t index = 0;
    while (index < units.size())
    {
        uint32_t code_point = static_cast<uint32_t>(units[index]);
        if (code_point >= 0xD800 && code_point <= 0xDBFF && index + 1 < units.size())
        {
            uint32_t low = static_cast<uint32_t>(units[index + 1]);
            if (low >= 0xDC00 && low <= 0xDFFF)
            {
                code_point = 0x10000 + ((code_point - 0xD800) << 10) + (low - 0xDC00);
                index += 2;
            }
            else
            {
                code_point = 0xFFFD;
                ++index;
            }
        }
        else if (code_point >= 0xD800 && code_point <= 0xDFFF)
        {
            code_point = 0xFFFD;
            ++index;
        }
        else
        {
            ++index;
        }
        if (code_point < 0x80)
        {
            out.push_back(static_cast<char>(code_point));
        }
        else if (code_point < 0x800)
        {
            out.push_back(static_cast<char>(0xC0 | (code_point >> 6)));
            out.push_back(static_cast<char>(0x80 | (code_point & 0x3F)));
        }
        else if (code_point < 0x10000)
        {
            out.push_back(static_cast<char>(0xE0 | (code_point >> 12)));
            out.push_back(static_cast<char>(0x80 | ((code_point >> 6) & 0x3F)));
            out.push_back(static_cast<char>(0x80 | (code_point & 0x3F)));
        }
        else
        {
            out.push_back(static_cast<char>(0xF0 | (code_point >> 18)));
            out.push_back(static_cast<char>(0x80 | ((code_point >> 12) & 0x3F)));
            out.push_back(static_cast<char>(0x80 | ((code_point >> 6) & 0x3F)));
            out.push_back(static_cast<char>(0x80 | (code_point & 0x3F)));
        }
    }
    return out;
}
} // namespace regexp_detail

class RegExp
{
public:
    explicit RegExp(const std::string& pattern, bool ignore_case = false)
        : m_valid(false)
    {
        try
        {
            std::regex_constants::syntax_option_type flags = std::regex::ECMAScript;
            if (ignore_case)
            {
                flags |= std::regex::icase;
            }
            m_regex = std::wregex(regexp_detail::utf8_to_utf16_units(pattern), flags);
            m_valid = true;
        }
        catch (const std::regex_error&)
        {
            // morph check validates every pattern before codegen; this is a
            // safety net for a pattern the translator could not lower.
            m_valid = false;
        }
    }

    bool test(const JsValue& subject) const
    {
        if (!m_valid)
        {
            return false;
        }
        std::wstring text = regexp_detail::utf8_to_utf16_units(_js_to_string(subject));
        return std::regex_search(text, m_regex);
    }

    JsValue exec(const JsValue& subject) const
    {
        if (!m_valid)
        {
            return JsValue(JsNull{});
        }
        std::string input = _js_to_string(subject);
        std::wstring text = regexp_detail::utf8_to_utf16_units(input);
        std::wsmatch match;
        if (!std::regex_search(text, match, m_regex))
        {
            return JsValue(JsNull{});
        }
        JsObject result;
        for (size_t index = 0; index < match.size(); ++index)
        {
            if (match[index].matched)
            {
                result.set(
                    std::to_string(index),
                    JsValue(regexp_detail::utf16_units_to_utf8(match[index].str())));
            }
            else
            {
                result.set(std::to_string(index), JsValue(JsUndefined{}));
            }
        }
        result.set("length", JsValue(static_cast<int64_t>(match.size())));
        result.set("index", JsValue(static_cast<int64_t>(match.position())));
        result.set("input", JsValue(input));
        result.set("groups", JsValue(JsUndefined{}));
        result.set("indices", JsValue(JsUndefined{}));
        return JsValue(result);
    }

private:
    std::wregex m_regex;
    bool m_valid;
};
} // namespace morph
