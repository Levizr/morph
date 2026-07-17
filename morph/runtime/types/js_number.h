#pragma once
#include <variant>
#include <cmath>
#include <cstdint>
#include <cstddef>
#include <string>
#include <cstring>
#include <type_traits>
#include "js_string.h"
#include <charconv>

struct JsValue;

struct JsNumber {
    // string holds big literals that don't fit in int64_t/double precisely
    std::variant<std::string, int64_t, double> value;

    JsNumber() : value(int64_t(0)) {}
    JsNumber(int v) : value(int64_t(v)) {}
    JsNumber(int64_t v) : value(v) {}
    JsNumber(double v) : value(v) {}
    explicit JsNumber(const char* v) : value(std::string(v)) {}
    explicit JsNumber(const std::string& v) : value(v) {}

    bool is_int() const { return std::holds_alternative<int64_t>(value); }
    bool is_double() const { return std::holds_alternative<double>(value); }
    bool is_big() const { return std::holds_alternative<std::string>(value); }

    double as_double() const {
        if (auto* i = std::get_if<int64_t>(&value)) return static_cast<double>(*i);
        if (auto* s = std::get_if<std::string>(&value)) {
            char* end = nullptr;
            double d = std::strtod(s->c_str(), &end);
            return d;
        }
        return std::get<double>(value);
    }

    int64_t as_int() const {
        if (auto* i = std::get_if<int64_t>(&value)) return *i;
        if (auto* s = std::get_if<std::string>(&value)) {
            char* end = nullptr;
            return static_cast<int64_t>(std::strtoll(s->c_str(), &end, 10));
        }
        return static_cast<int64_t>(std::get<double>(value));
    }

    // ── Arithmetic (big values fall back to double) ──

    JsNumber operator+(const JsNumber& o) const {
        if (is_int() && o.is_int()) return int64_t(std::get<int64_t>(value) + std::get<int64_t>(o.value));
        return as_double() + o.as_double();
    }

    JsNumber operator-(const JsNumber& o) const {
        if (is_int() && o.is_int()) return int64_t(std::get<int64_t>(value) - std::get<int64_t>(o.value));
        return as_double() - o.as_double();
    }

    JsNumber operator*(const JsNumber& o) const {
        if (is_int() && o.is_int()) return int64_t(std::get<int64_t>(value) * std::get<int64_t>(o.value));
        return as_double() * o.as_double();
    }

    JsNumber operator/(const JsNumber& o) const {
        double d = o.as_double();
        if (d == 0.0) return double(0.0);
        return as_double() / d;
    }

    JsNumber operator%(const JsNumber& o) const {
        if (is_int() && o.is_int()) return int64_t(std::get<int64_t>(value) % std::get<int64_t>(o.value));
        return std::fmod(as_double(), o.as_double());
    }

    // ── Bitwise ──

    JsNumber operator&(const JsNumber& o) const { return int64_t(as_int32() & o.as_int32()); }
    JsNumber operator|(const JsNumber& o) const { return int64_t(as_int32() | o.as_int32()); }
    JsNumber operator^(const JsNumber& o) const { return int64_t(as_int32() ^ o.as_int32()); }
    JsNumber operator<<(const JsNumber& o) const { return int64_t(as_int32() << (o.as_int32() & 31)); }
    JsNumber operator>>(const JsNumber& o) const { return int64_t(as_int32() >> (o.as_int32() & 31)); }
    JsNumber operator~() const { return int64_t(~as_int32()); }

    // ── Comparison ──

    bool operator==(const JsNumber& o) const {
        if (is_int() && o.is_int()) return std::get<int64_t>(value) == std::get<int64_t>(o.value);
        return as_double() == o.as_double();
    }
    bool operator!=(const JsNumber& o) const { return !(*this == o); }
    bool operator<(const JsNumber& o) const { return as_double() < o.as_double(); }
    bool operator>(const JsNumber& o) const { return as_double() > o.as_double(); }
    bool operator<=(const JsNumber& o) const { return as_double() <= o.as_double(); }
    bool operator>=(const JsNumber& o) const { return as_double() >= o.as_double(); }

    // ── Assignment from JsValue (used when mixing with JsValue expressions) ──

    JsNumber& operator=(const JsValue& v);

    // ── Compound assignment ──

    JsNumber& operator+=(const JsNumber& o) { *this = *this + o; return *this; }
    JsNumber& operator-=(const JsNumber& o) { *this = *this - o; return *this; }
    JsNumber& operator*=(const JsNumber& o) { *this = *this * o; return *this; }
    JsNumber& operator/=(const JsNumber& o) { *this = *this / o; return *this; }
    JsNumber& operator%=(const JsNumber& o) { *this = *this % o; return *this; }
    JsNumber& operator&=(const JsNumber& o) { *this = *this & o; return *this; }
    JsNumber& operator|=(const JsNumber& o) { *this = *this | o; return *this; }
    JsNumber& operator^=(const JsNumber& o) { *this = *this ^ o; return *this; }

    // ── Unary ──
    JsNumber operator-() const {
        if (is_int()) return int64_t(-std::get<int64_t>(value));
        return -as_double();
    }
    JsNumber operator+() const { return *this; }

    // ── Increment / Decrement (for loop counters) ──
    JsNumber& operator++() {
        if (is_int()) value = int64_t(std::get<int64_t>(value) + 1);
        else value = as_double() + 1.0;
        return *this;
    }
    JsNumber operator++(int) {
        JsNumber old = *this;
        ++(*this);
        return old;
    }
    JsNumber& operator--() {
        if (is_int()) value = int64_t(std::get<int64_t>(value) - 1);
        else value = as_double() - 1.0;
        return *this;
    }
    JsNumber operator--(int) {
        JsNumber old = *this;
        --(*this);
        return old;
    }

    // ── Conversion ──
    std::string as_string() const;
    JsString toString() const { return as_string(); }

private:
    int32_t as_int32() const {
        return static_cast<int32_t>(as_int());
    }
};

// ── JsNumber arithmetic with primitives (more specific than JsValue/JsString overloads) ──
inline JsNumber operator+(const JsNumber& a, int64_t b) { return a + JsNumber(b); }
inline JsNumber operator-(const JsNumber& a, int64_t b) { return a - JsNumber(b); }
inline JsNumber operator*(const JsNumber& a, int64_t b) { return a * JsNumber(b); }
inline JsNumber operator/(const JsNumber& a, int64_t b) { return a / JsNumber(b); }
inline JsNumber operator%(const JsNumber& a, int64_t b) { return a % JsNumber(b); }
inline JsNumber operator+(int64_t a, const JsNumber& b) { return JsNumber(a) + b; }
inline JsNumber operator-(int64_t a, const JsNumber& b) { return JsNumber(a) - b; }
inline JsNumber operator*(int64_t a, const JsNumber& b) { return JsNumber(a) * b; }
inline JsNumber operator/(int64_t a, const JsNumber& b) { return JsNumber(a) / b; }
inline JsNumber operator%(int64_t a, const JsNumber& b) { return JsNumber(a) % b; }

// ── JsNumber comparison with int64_t (disambiguate from JsValue overloads) ──
inline bool operator==(const JsNumber& a, int64_t b) { return a.as_double() == (double)b; }
inline bool operator!=(const JsNumber& a, int64_t b) { return a.as_double() != (double)b; }
inline bool operator==(int64_t a, const JsNumber& b) { return (double)a == b.as_double(); }
inline bool operator!=(int64_t a, const JsNumber& b) { return (double)a != b.as_double(); }

// ── JsNumber comparison with size_t (for loop: i < array.length()) ──
inline bool operator<(const JsNumber& a, size_t b) { return a.as_int() < (int64_t)b; }
inline bool operator<=(const JsNumber& a, size_t b) { return a.as_int() <= (int64_t)b; }
inline bool operator>(const JsNumber& a, size_t b) { return a.as_int() > (int64_t)b; }
inline bool operator>=(const JsNumber& a, size_t b) { return a.as_int() >= (int64_t)b; }
inline bool operator<(size_t a, const JsNumber& b) { return (int64_t)a < b.as_int(); }
inline bool operator<=(size_t a, const JsNumber& b) { return (int64_t)a <= b.as_int(); }
inline bool operator>(size_t a, const JsNumber& b) { return (int64_t)a > b.as_int(); }
inline bool operator>=(size_t a, const JsNumber& b) { return (int64_t)a >= b.as_int(); }

// ── std::formatter for std::println / std::format ──

#include <format>

template <>
struct std::formatter<JsNumber> {
    constexpr auto parse(auto& ctx) { return ctx.begin(); }
    auto format(const JsNumber& v, auto& ctx) const {
        if (v.is_int())
            return std::format_to(ctx.out(), "{}", std::get<int64_t>(v.value));
        if (v.is_big())
            return std::format_to(ctx.out(), "{}", std::get<std::string>(v.value));
        return std::format_to(ctx.out(), "{}", std::get<double>(v.value));
    }
};

inline std::string JsNumber::as_string() const {
    if (is_int()) return std::to_string(std::get<int64_t>(value));
    if (is_big()) return std::get<std::string>(value);
    auto s = std::to_string(std::get<double>(value));
    auto dot = s.find('.');
    if (dot != std::string::npos) {
        auto last = s.find_last_not_of('0');
        if (last > dot) s = s.substr(0, last + 1);
        else s = s.substr(0, dot);
    }
    return s;
}
