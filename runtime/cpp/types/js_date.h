#pragma once
// JavaScript Date subset: construction, Date.now/parse, toISOString, and
// local/UTC calendar getters. Timezone-aware formatting (toString,
// toLocaleString) is intentionally absent; morph check rejects it.
#include <cerrno>
#include <chrono>
#include <cctype>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <ctime>
#include <initializer_list>
#include <limits>
#include <string>
#include "js_types.h"

namespace morph
{
inline constexpr double kDateMaxTime = 8.64e15;
inline constexpr int64_t kDateMsPerDay = 86400000;

class Date
{
public:
    Date()
        : m_ms(current_time_ms())
    {
    }

    explicit Date(const JsValue& value)
        : m_ms(value_to_time(value))
    {
    }

    explicit Date(std::initializer_list<JsValue> parts)
        : m_ms(parts_to_time(parts))
    {
    }

    Date(const Date&) = default;
    Date& operator=(const Date&) = default;

    static JsNumber now()
    {
        return JsNumber(current_time_ms());
    }

    static JsNumber parse(const JsValue& value)
    {
        return JsNumber(parse_value(value));
    }

    JsNumber getTime() const
    {
        return JsNumber(m_ms);
    }

    JsNumber valueOf() const
    {
        return getTime();
    }

    JsString toISOString() const
    {
        if (!valid())
        {
            throw_range_error("Invalid time value");
        }
        return JsString(format_iso(m_ms));
    }

    JsNumber getFullYear() const
    {
        return JsNumber(local_component(Component::Year));
    }

    JsNumber getMonth() const
    {
        return JsNumber(local_component(Component::Month));
    }

    JsNumber getDate() const
    {
        return JsNumber(local_component(Component::Date));
    }

    JsNumber getDay() const
    {
        return JsNumber(local_component(Component::Day));
    }

    JsNumber getHours() const
    {
        return JsNumber(local_component(Component::Hours));
    }

    JsNumber getMinutes() const
    {
        return JsNumber(local_component(Component::Minutes));
    }

    JsNumber getSeconds() const
    {
        return JsNumber(local_component(Component::Seconds));
    }

    JsNumber getMilliseconds() const
    {
        return JsNumber(local_component(Component::Milliseconds));
    }

    JsNumber getUTCFullYear() const
    {
        return JsNumber(utc_component(Component::Year));
    }

    JsNumber getUTCMonth() const
    {
        return JsNumber(utc_component(Component::Month));
    }

    JsNumber getUTCDate() const
    {
        return JsNumber(utc_component(Component::Date));
    }

    JsNumber getUTCDay() const
    {
        return JsNumber(utc_component(Component::Day));
    }

    JsNumber getUTCHours() const
    {
        return JsNumber(utc_component(Component::Hours));
    }

    JsNumber getUTCMinutes() const
    {
        return JsNumber(utc_component(Component::Minutes));
    }

    JsNumber getUTCSeconds() const
    {
        return JsNumber(utc_component(Component::Seconds));
    }

    JsNumber getUTCMilliseconds() const
    {
        return JsNumber(utc_component(Component::Milliseconds));
    }

private:
    enum class Component
    {
        Year,
        Month,
        Date,
        Day,
        Hours,
        Minutes,
        Seconds,
        Milliseconds
    };

    struct Civil
    {
        int64_t year;
        int month;
        int day;
    };

    struct UtcParts
    {
        int64_t year;
        int month;
        int day;
        int hours;
        int minutes;
        int seconds;
        int milliseconds;
        int weekday;
    };

    double m_ms;

    bool valid() const
    {
        return std::isfinite(m_ms);
    }

    static double invalid()
    {
        return std::numeric_limits<double>::quiet_NaN();
    }

    static int64_t floor_div(int64_t value, int64_t divisor)
    {
        int64_t quotient = value / divisor;
        int64_t remainder = value % divisor;
        if (remainder != 0 && ((remainder < 0) != (divisor < 0)))
        {
            --quotient;
        }
        return quotient;
    }

    static int64_t positive_mod(int64_t value, int64_t divisor)
    {
        int64_t remainder = value % divisor;
        return remainder < 0 ? remainder + divisor : remainder;
    }

    static int64_t days_from_civil(int64_t year, int64_t month, int64_t day)
    {
        year -= month <= 2 ? 1 : 0;
        const int64_t era = (year >= 0 ? year : year - 399) / 400;
        const int64_t yoe = year - era * 400;
        const int64_t doy = (153 * (month + (month > 2 ? -3 : 9)) + 2) / 5 + day - 1;
        const int64_t doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        return era * 146097 + doe - 719468;
    }

    static Civil civil_from_days(int64_t days)
    {
        int64_t z = days + 719468;
        const int64_t era = (z >= 0 ? z : z - 146096) / 146097;
        const int64_t doe = z - era * 146097;
        const int64_t yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        int64_t year = yoe + era * 400;
        const int64_t doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        const int64_t mp = (5 * doy + 2) / 153;
        const int64_t day = doy - (153 * mp + 2) / 5 + 1;
        const int64_t month = mp + (mp < 10 ? 3 : -9);
        year += month <= 2 ? 1 : 0;
        return Civil{year, static_cast<int>(month), static_cast<int>(day)};
    }

    static double time_clip(double milliseconds)
    {
        if (!std::isfinite(milliseconds) || std::fabs(milliseconds) > kDateMaxTime)
        {
            return invalid();
        }
        return std::trunc(milliseconds);
    }

    static double current_time_ms()
    {
        const auto count = std::chrono::duration_cast<std::chrono::milliseconds>(
                               std::chrono::system_clock::now().time_since_epoch())
                               .count();
        return static_cast<double>(count);
    }

    static double value_to_time(const JsValue& value)
    {
        if (value.is_string())
        {
            return parse_string(std::get<JsString>(value.inner).value);
        }
        if (value.is_number())
        {
            return time_clip(std::get<JsNumber>(value.inner).as_double());
        }
        if (value.is_undefined())
        {
            return invalid();
        }
        if (value.is_null())
        {
            return 0.0;
        }
        if (value.is_boolean())
        {
            return std::get<JsBoolean>(value.inner).value ? 1.0 : 0.0;
        }
        return time_clip(js_to_number(value).as_double());
    }

    static double parse_value(const JsValue& value)
    {
        if (value.is_string())
        {
            return parse_string(std::get<JsString>(value.inner).value);
        }
        return parse_string(js_to_string(value).value);
    }

    static bool is_digit(char c)
    {
        return c >= '0' && c <= '9';
    }

    static bool read_digits(const std::string& text, size_t& position, int count, int& out)
    {
        if (position + static_cast<size_t>(count) > text.size())
        {
            return false;
        }
        int value = 0;
        for (int i = 0; i < count; ++i)
        {
            const char c = text[position + static_cast<size_t>(i)];
            if (!is_digit(c))
            {
                return false;
            }
            value = value * 10 + (c - '0');
        }
        position += static_cast<size_t>(count);
        out = value;
        return true;
    }

    static std::string trim(const std::string& text)
    {
        size_t start = 0;
        size_t end = text.size();
        while (start < end && std::isspace(static_cast<unsigned char>(text[start])))
        {
            ++start;
        }
        while (end > start && std::isspace(static_cast<unsigned char>(text[end - 1])))
        {
            --end;
        }
        return text.substr(start, end - start);
    }

    static double parse_string(const std::string& raw)
    {
        const std::string text = trim(raw);
        size_t position = 0;
        int year = 0;
        int month = 0;
        int day = 0;
        if (!read_digits(text, position, 4, year))
        {
            return invalid();
        }
        if (position >= text.size() || text[position] != '-')
        {
            return invalid();
        }
        ++position;
        if (!read_digits(text, position, 2, month))
        {
            return invalid();
        }
        if (position >= text.size() || text[position] != '-')
        {
            return invalid();
        }
        ++position;
        if (!read_digits(text, position, 2, day))
        {
            return invalid();
        }
        if (month < 1 || month > 12 || day < 1 || day > 31)
        {
            return invalid();
        }

        int hour = 0;
        int minute = 0;
        int second = 0;
        int millisecond = 0;
        int offset_minutes = 0;
        bool has_time = false;
        bool has_offset = false;

        if (position < text.size())
        {
            if (text[position] != 'T')
            {
                return invalid();
            }
            ++position;
            has_time = true;
            if (!read_digits(text, position, 2, hour))
            {
                return invalid();
            }
            if (position >= text.size() || text[position] != ':')
            {
                return invalid();
            }
            ++position;
            if (!read_digits(text, position, 2, minute))
            {
                return invalid();
            }
            if (position < text.size() && text[position] == ':')
            {
                ++position;
                if (!read_digits(text, position, 2, second))
                {
                    return invalid();
                }
                if (position < text.size() && text[position] == '.')
                {
                    ++position;
                    int digits = 0;
                    int fraction = 0;
                    while (position < text.size() && is_digit(text[position]) && digits < 9)
                    {
                        if (digits < 3)
                        {
                            fraction = fraction * 10 + (text[position] - '0');
                        }
                        ++digits;
                        ++position;
                    }
                    if (digits == 0)
                    {
                        return invalid();
                    }
                    while (digits < 3)
                    {
                        fraction *= 10;
                        ++digits;
                    }
                    millisecond = fraction;
                }
            }
            if (position < text.size())
            {
                if (text[position] == 'Z')
                {
                    has_offset = true;
                    ++position;
                }
                else if (text[position] == '+' || text[position] == '-')
                {
                    const int sign = text[position] == '-' ? -1 : 1;
                    ++position;
                    int offset_hour = 0;
                    int offset_minute = 0;
                    if (!read_digits(text, position, 2, offset_hour))
                    {
                        return invalid();
                    }
                    if (position < text.size() && text[position] == ':')
                    {
                        ++position;
                        if (!read_digits(text, position, 2, offset_minute))
                        {
                            return invalid();
                        }
                    }
                    else if (position + 2 <= text.size())
                    {
                        if (!read_digits(text, position, 2, offset_minute))
                        {
                            return invalid();
                        }
                    }
                    else
                    {
                        return invalid();
                    }
                    if (offset_hour > 23 || offset_minute > 59)
                    {
                        return invalid();
                    }
                    has_offset = true;
                    offset_minutes = sign * (offset_hour * 60 + offset_minute);
                }
            }
            if (position != text.size())
            {
                return invalid();
            }
            if (hour == 24)
            {
                if (minute != 0 || second != 0 || millisecond != 0)
                {
                    return invalid();
                }
                hour = 0;
                ++day;
            }
            else if (hour > 23)
            {
                return invalid();
            }
            if (minute > 59 || second > 59)
            {
                return invalid();
            }
        }

        if (!has_time)
        {
            const int64_t days = days_from_civil(year, month, day);
            return time_clip(static_cast<double>(days) * static_cast<double>(kDateMsPerDay));
        }

        if (has_offset)
        {
            const int64_t days = days_from_civil(year, month, day);
            const int64_t seconds =
                days * 86400 + static_cast<int64_t>(hour) * 3600 + static_cast<int64_t>(minute) * 60 +
                second;
            const int64_t milliseconds =
                seconds * 1000 + millisecond - static_cast<int64_t>(offset_minutes) * 60000;
            return time_clip(static_cast<double>(milliseconds));
        }

        std::tm tm{};
        tm.tm_year = year - 1900;
        tm.tm_mon = month - 1;
        tm.tm_mday = day;
        tm.tm_hour = hour;
        tm.tm_min = minute;
        tm.tm_sec = second;
        tm.tm_isdst = -1;
        errno = 0;
        const std::time_t local_seconds = std::mktime(&tm);
        if (local_seconds == static_cast<std::time_t>(-1) && errno != 0)
        {
            return invalid();
        }
        const double milliseconds =
            static_cast<double>(local_seconds) * 1000.0 + static_cast<double>(millisecond);
        return time_clip(milliseconds);
    }

    static double parts_to_time(std::initializer_list<JsValue> parts)
    {
        if (parts.size() < 2 || parts.size() > 7)
        {
            return invalid();
        }
        double fields[7] = {0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0};
        size_t index = 0;
        for (const JsValue& part : parts)
        {
            fields[index++] = js_to_number(part).as_double();
        }
        for (size_t i = 0; i < index; ++i)
        {
            if (!std::isfinite(fields[i]) || std::fabs(fields[i]) > 1000000.0)
            {
                return invalid();
            }
        }

        double year = std::trunc(fields[0]);
        double month = std::trunc(fields[1]);
        const double day = index > 2 ? std::trunc(fields[2]) : 1.0;
        const double hour = index > 3 ? std::trunc(fields[3]) : 0.0;
        const double minute = index > 4 ? std::trunc(fields[4]) : 0.0;
        const double second = index > 5 ? std::trunc(fields[5]) : 0.0;
        const double millisecond = index > 6 ? std::trunc(fields[6]) : 0.0;
        if (year >= 0.0 && year <= 99.0)
        {
            year += 1900.0;
        }
        if (year < -1000000.0 || year > 1000000.0)
        {
            return invalid();
        }

        std::tm tm{};
        tm.tm_year = static_cast<int>(year - 1900.0);
        tm.tm_mon = static_cast<int>(month);
        tm.tm_mday = static_cast<int>(day);
        tm.tm_hour = static_cast<int>(hour);
        tm.tm_min = static_cast<int>(minute);
        tm.tm_sec = static_cast<int>(second);
        tm.tm_isdst = -1;
        errno = 0;
        const std::time_t local_seconds = std::mktime(&tm);
        if (local_seconds == static_cast<std::time_t>(-1) && errno != 0)
        {
            return invalid();
        }
        const double milliseconds =
            static_cast<double>(local_seconds) * 1000.0 + millisecond;
        return time_clip(milliseconds);
    }

    static double milliseconds_of(double milliseconds)
    {
        const int64_t total = static_cast<int64_t>(milliseconds);
        const int64_t seconds = floor_div(total, 1000);
        return static_cast<double>(total - seconds * 1000);
    }

    bool local_tm(std::tm& out) const
    {
        if (!valid())
        {
            return false;
        }
        const int64_t seconds = floor_div(static_cast<int64_t>(m_ms), 1000);
        const std::time_t time = static_cast<std::time_t>(seconds);
        return localtime_r(&time, &out) != nullptr;
    }

    bool utc_parts(UtcParts& out) const
    {
        if (!valid())
        {
            return false;
        }
        const int64_t total = static_cast<int64_t>(m_ms);
        const int64_t days = floor_div(total, kDateMsPerDay);
        int64_t remainder = total - days * kDateMsPerDay;
        const Civil civil = civil_from_days(days);
        out.year = civil.year;
        out.month = civil.month - 1;
        out.day = civil.day;
        out.hours = static_cast<int>(remainder / 3600000);
        remainder %= 3600000;
        out.minutes = static_cast<int>(remainder / 60000);
        remainder %= 60000;
        out.seconds = static_cast<int>(remainder / 1000);
        out.milliseconds = static_cast<int>(remainder % 1000);
        out.weekday = static_cast<int>(positive_mod(days + 4, 7));
        return true;
    }

    double local_component(Component component) const
    {
        if (component == Component::Milliseconds)
        {
            return valid() ? milliseconds_of(m_ms) : invalid();
        }
        std::tm tm{};
        if (!local_tm(tm))
        {
            return invalid();
        }
        switch (component)
        {
            case Component::Year:
                return static_cast<double>(tm.tm_year + 1900);
            case Component::Month:
                return static_cast<double>(tm.tm_mon);
            case Component::Date:
                return static_cast<double>(tm.tm_mday);
            case Component::Day:
                return static_cast<double>(tm.tm_wday);
            case Component::Hours:
                return static_cast<double>(tm.tm_hour);
            case Component::Minutes:
                return static_cast<double>(tm.tm_min);
            case Component::Seconds:
                return static_cast<double>(tm.tm_sec);
            case Component::Milliseconds:
                break;
        }
        return invalid();
    }

    double utc_component(Component component) const
    {
        UtcParts parts{};
        if (!utc_parts(parts))
        {
            return invalid();
        }
        switch (component)
        {
            case Component::Year:
                return static_cast<double>(parts.year);
            case Component::Month:
                return static_cast<double>(parts.month);
            case Component::Date:
                return static_cast<double>(parts.day);
            case Component::Day:
                return static_cast<double>(parts.weekday);
            case Component::Hours:
                return static_cast<double>(parts.hours);
            case Component::Minutes:
                return static_cast<double>(parts.minutes);
            case Component::Seconds:
                return static_cast<double>(parts.seconds);
            case Component::Milliseconds:
                return static_cast<double>(parts.milliseconds);
        }
        return invalid();
    }

    static std::string format_iso(double milliseconds)
    {
        const int64_t total = static_cast<int64_t>(milliseconds);
        const int64_t days = floor_div(total, kDateMsPerDay);
        int64_t remainder = total - days * kDateMsPerDay;
        const Civil civil = civil_from_days(days);
        const int hour = static_cast<int>(remainder / 3600000);
        remainder %= 3600000;
        const int minute = static_cast<int>(remainder / 60000);
        remainder %= 60000;
        const int second = static_cast<int>(remainder / 1000);
        const int millisecond = static_cast<int>(remainder % 1000);
        char buffer[40];
        if (civil.year >= 0 && civil.year <= 9999)
        {
            std::snprintf(
                buffer,
                sizeof(buffer),
                "%04lld-%02d-%02dT%02d:%02d:%02d.%03dZ",
                static_cast<long long>(civil.year),
                civil.month,
                civil.day,
                hour,
                minute,
                second,
                millisecond);
        }
        else if (civil.year > 9999)
        {
            std::snprintf(
                buffer,
                sizeof(buffer),
                "+%06lld-%02d-%02dT%02d:%02d:%02d.%03dZ",
                static_cast<long long>(civil.year),
                civil.month,
                civil.day,
                hour,
                minute,
                second,
                millisecond);
        }
        else
        {
            std::snprintf(
                buffer,
                sizeof(buffer),
                "-%06lld-%02d-%02dT%02d:%02d:%02d.%03dZ",
                static_cast<long long>(-civil.year),
                civil.month,
                civil.day,
                hour,
                minute,
                second,
                millisecond);
        }
        return buffer;
    }
};
} // namespace morph

#ifndef MORPH_NO_FORMAT
template <>
struct std::formatter<morph::Date>
{
    constexpr auto parse(auto& ctx)
    {
        return ctx.begin();
    }

    auto format(const morph::Date& value, auto& ctx) const
    {
        if (std::isnan(value.getTime().as_double()))
        {
            return std::format_to(ctx.out(), "Invalid Date");
        }
        return std::format_to(ctx.out(), "{}", value.toISOString().value);
    }
};
#endif
