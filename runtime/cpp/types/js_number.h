#pragma once
#include <variant>
#include <cmath>
#include <cstdint>
#include <cstddef>
#include <string>
#include <cstring>
#include <cstdio>
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
    JsNumber(const JsValue& v); // defined in js_value.h after JsValue complete

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
    JsNumber& operator+=(int o) { *this = *this + JsNumber((int64_t)o); return *this; }
    JsNumber& operator-=(int o) { *this = *this - JsNumber((int64_t)o); return *this; }
    JsNumber& operator*=(int o) { *this = *this * JsNumber((int64_t)o); return *this; }
    JsNumber& operator/=(int o) { *this = *this / JsNumber((int64_t)o); return *this; }
    JsNumber& operator%=(int o) { *this = *this % JsNumber((int64_t)o); return *this; }
    JsNumber& operator+=(int64_t o) { *this = *this + JsNumber(o); return *this; }
    JsNumber& operator-=(int64_t o) { *this = *this - JsNumber(o); return *this; }
    JsNumber& operator*=(int64_t o) { *this = *this * JsNumber(o); return *this; }
    JsNumber& operator/=(int64_t o) { *this = *this / JsNumber(o); return *this; }
    JsNumber& operator%=(int64_t o) { *this = *this % JsNumber(o); return *this; }

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

// Shortest round-trip digits for a finite nonzero double: digits holds
// the significant digits (no dot, no trailing zeros) and exp10 is the
// decimal exponent of the first digit. Tries increasing %g precision
// and keeps the first that parses back bitwise-identically.
static void js_shortest(double d, std::string& digits, int& exp10) {
    for (int prec = 1; prec <= 17; ++prec) {
        char buf[32];
        std::snprintf(buf, sizeof(buf), "%.*g", prec, d);
        std::string orig(buf);
        std::string s = orig;
        int ex = 0;
        auto epos = s.find('e');
        if (epos != std::string::npos) {
            ex = std::stoi(s.substr(epos + 1));
            s = s.substr(0, epos);
        }
        auto dot = s.find('.');
        int intlen = dot == std::string::npos ? (int)s.size() : (int)dot;
        std::string all;
        for (char c : s) {
            if (c != '.') all += c;
        }
        while (all.size() > 1 && all.back() == '0') all.pop_back();
        size_t lead = 0;
        while (lead + 1 < all.size() && all[lead] == '0') ++lead;
        if (lead > 0) {
            all = all.substr(lead);
            intlen -= (int)lead;
        }
        // Parse the full text back (exponent included) to check round-trip.
        char* end = nullptr;
        double back = std::strtod(orig.c_str(), &end);
        (void)end;
        if (back == d) {
            digits = all;
            exp10 = intlen - 1 + ex;
            return;
        }
    }
    digits = "0";
    exp10 = 0;
}

// JS Number.prototype.toString for doubles: plain notation for
// 10^-6 ≤ |d| < 10^21, exponential shortest otherwise.
static std::string js_double_string(double d) {
    std::string digits;
    int exp10 = 0;
    js_shortest(d < 0 ? -d : d, digits, exp10);
    std::string out = d < 0 ? "-" : "";
    if (exp10 >= -6 && exp10 < 21) {
        if (exp10 >= 0) {
            if ((int)digits.size() <= exp10 + 1) {
                out += digits + std::string((size_t)(exp10 + 1) - digits.size(), '0');
            } else {
                out += digits.substr(0, (size_t)exp10 + 1);
                out += '.';
                out += digits.substr((size_t)exp10 + 1);
            }
        } else {
            out += "0.";
            out += std::string((size_t)(-exp10 - 1), '0');
            out += digits;
        }
        return out;
    }
    out += digits[0];
    if (digits.size() > 1) {
        out += '.';
        out += digits.substr(1);
    }
    out += 'e';
    out += exp10 < 0 ? "-" : "+";
    out += std::to_string(exp10 < 0 ? -exp10 : exp10);
    return out;
}

// Fixed-decimal formatting with JS tie-breaking (ties round away from
// zero, matching V8). Exact while the scaled magnitude fits int64;
// huge magnitudes cannot tie (those doubles are integral) so printf
// is exact there too. Zero keeps the input sign only for d < 0.
static std::string js_format_fixed(double d, int frac) {
    if (d == 0.0) {
        std::string zero = (d < 0) ? "-0" : "0";
        if (frac <= 0) return zero;
        return zero + "." + std::string((size_t)frac, '0');
    }
    bool neg = d < 0;
    long double scaled = neg ? -(long double)d : (long double)d;
    for (int i = 0; i < frac; ++i) scaled *= 10.0L;
    int64_t r = 0;
    std::string digs;
    if (scaled < 4503599627370496.0L) {
        r = (int64_t)std::floor(scaled + 0.5L);
        digs = std::to_string(r);
        while (digs.size() <= (size_t)frac) digs = "0" + digs;
    } else {
        char buf[400];
        std::snprintf(buf, sizeof(buf), "%.*f", frac, neg ? -d : d);
        digs = buf;
        auto dot = digs.find('.');
        if (dot != std::string::npos) digs.erase(dot, 1);
        while (digs.size() > 1 && digs[0] == '0') digs.erase(digs.begin());
    }
    std::string out = neg ? "-" : "";
    if (frac <= 0) {
        out += digs;
    } else {
        out += digs.substr(0, digs.size() - (size_t)frac);
        out += '.';
        out += digs.substr(digs.size() - (size_t)frac);
    }
    return out;
}

// Decimal exponent of a positive finite double, adjusted so that
// 10^e ≤ mag < 10^(e+1) (log10 alone mis-rounds near powers of 10).
static int64_t js_exp10(double mag) {
    int64_t e = (int64_t)std::floor(std::log10(mag));
    while (e < 308 && std::pow(10.0, (double)(e + 1)) <= mag) ++e;
    while (e > -324 && std::pow(10.0, (double)e) > mag) --e;
    return e;
}

// Round to p significant digits (ties away from zero): returns the
// significand with exactly p digits and sets exp so that
// value = 0.significand × 10^(exp+1)... i.e. significand × 10^(exp-p+1).
static unsigned long long js_significand(double d, int p, int64_t& exp) {
    double mag = d < 0 ? -d : d;
    exp = js_exp10(mag);
    while (exp < 308 && std::pow(10.0, (double)(exp + 1)) <= mag) ++exp;
    while (exp > -324 && std::pow(10.0, (double)exp) > mag) --exp;
    int shift = (int)(exp - p + 1);
    long double m = (long double)mag;
    if (shift >= 0) {
        for (int i = 0; i < shift; ++i) m /= 10.0L;
    } else {
        for (int i = 0; i < -shift; ++i) m *= 10.0L;
    }
    unsigned long long sig = (unsigned long long)std::floor(m + 0.5L);
    unsigned long long cap = 1;
    for (int i = 0; i < p; ++i) cap *= 10ULL;
    if (sig >= cap) {
        sig /= 10ULL;
        ++exp;
    }
    return sig;
}

    // ── Conversion ──
    std::string as_string() const;
    JsString toString() const { return as_string(); }

    // ── JS Number formatting (defined in js_value.h; throw RangeError
    // JsValue for out-of-range digits like V8) ──
    JsString toFixed(int64_t digits = 0) const;
    JsString toFixed(const JsValue& digits) const;
    // No arg → ToString; explicit arg must be 1..100 significant digits.
    JsString toPrecision() const;
    JsString toPrecision(int64_t sig) const;
    JsString toPrecision(const JsValue& sig) const;
    // No arg → shortest round-trip mantissa; explicit arg must be 0..100.
    JsString toExponential() const;
    JsString toExponential(int64_t frac) const;
    JsString toExponential(const JsValue& frac) const;

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
// Exact `int` overloads to avoid ISO ambiguity with `JsString + int` (JsNumber->JsString user-defined vs int->int64_t standard)
inline JsNumber operator+(const JsNumber& a, int b) { return a + JsNumber((int64_t)b); }
inline JsNumber operator-(const JsNumber& a, int b) { return a - JsNumber((int64_t)b); }
inline JsNumber operator*(const JsNumber& a, int b) { return a * JsNumber((int64_t)b); }
inline JsNumber operator/(const JsNumber& a, int b) { return a / JsNumber((int64_t)b); }
inline JsNumber operator%(const JsNumber& a, int b) { return a % JsNumber((int64_t)b); }
inline JsNumber operator+(int a, const JsNumber& b) { return JsNumber((int64_t)a) + b; }
inline JsNumber operator-(int a, const JsNumber& b) { return JsNumber((int64_t)a) - b; }
inline JsNumber operator*(int a, const JsNumber& b) { return JsNumber((int64_t)a) * b; }
inline JsNumber operator/(int a, const JsNumber& b) { return JsNumber((int64_t)a) / b; }
inline JsNumber operator%(int a, const JsNumber& b) { return JsNumber((int64_t)a) % b; }

// ── JsNumber with double (for 3.14 * JsNumber) ──
inline JsNumber operator*(double a, const JsNumber& b) { return JsNumber(a) * b; }
inline JsNumber operator*(const JsNumber& a, double b) { return a * JsNumber(b); }
inline JsNumber operator/(double a, const JsNumber& b) { return JsNumber(a) / b; }
inline JsNumber operator/(const JsNumber& a, double b) { return a / JsNumber(b); }
inline JsNumber operator+(double a, const JsNumber& b) { return JsNumber(a) + b; }
inline JsNumber operator+(const JsNumber& a, double b) { return a + JsNumber(b); }
inline JsNumber operator-(double a, const JsNumber& b) { return JsNumber(a) - b; }
inline JsNumber operator-(const JsNumber& a, double b) { return a - JsNumber(b); }

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

// ── std::formatter for std::format ──
// See types/js_value_format.h (included by default via js_types.h).

inline std::string JsNumber::as_string() const {
    if (is_int()) return std::to_string(std::get<int64_t>(value));
    if (is_big()) return std::get<std::string>(value);
    double d = std::get<double>(value);
    if (std::isnan(d)) return "NaN";
    if (std::isinf(d)) return d > 0 ? "Infinity" : "-Infinity";
    if (d == 0.0) return "0";
    return js_double_string(d);
}
