#pragma once
#include <cmath>
#include <initializer_list>
#include <limits>
#include <random>
#include "js_convert.h"

// JS Math: constants, min/max over an argument list, unary/binary numeric
// ops. Non-numbers coerce via Number() (so Math.abs("x") is NaN, not 0).
// Semantics verified against node (see types verification fixture).

namespace morph {
namespace js_math {

inline constexpr double PI = 3.14159265358979323846;
inline constexpr double E = 2.71828182845904523536;
inline constexpr double LN2 = 0.69314718055994530942;
inline constexpr double LN10 = 2.30258509299404568402;
inline constexpr double LOG2E = 1.44269504088896340736;
inline constexpr double LOG10E = 0.43429448190325182765;
inline constexpr double SQRT1_2 = 0.70710678118654752440;
inline constexpr double SQRT2 = 1.41421356237309504880;

static double num(const JsValue& v) { return js_to_number(v).as_double(); }

inline JsNumber min(std::initializer_list<JsValue> args) {
    if (args.size() == 0) return JsNumber(std::numeric_limits<double>::infinity());
    bool all_int = true;
    int64_t best_i = 0;
    double best_d = 0.0;
    bool first = true;
    for (const auto& a : args) {
        JsNumber n = js_to_number(a);
        double d = n.as_double();
        if (d != d) return JsNumber(d);
        if (!n.is_int()) all_int = false;
        if (first || d < best_d) {
            best_d = d;
            if (n.is_int()) best_i = std::get<int64_t>(n.value);
            first = false;
        }
    }
    if (all_int) return JsNumber(best_i);
    return JsNumber(best_d);
}

inline JsNumber max(std::initializer_list<JsValue> args) {
    if (args.size() == 0) return JsNumber(-std::numeric_limits<double>::infinity());
    bool all_int = true;
    int64_t best_i = 0;
    double best_d = 0.0;
    bool first = true;
    for (const auto& a : args) {
        JsNumber n = js_to_number(a);
        double d = n.as_double();
        if (d != d) return JsNumber(d);
        if (!n.is_int()) all_int = false;
        if (first || d > best_d) {
            best_d = d;
            if (n.is_int()) best_i = std::get<int64_t>(n.value);
            first = false;
        }
    }
    if (all_int) return JsNumber(best_i);
    return JsNumber(best_d);
}

// Single-spread form: Math.max(...arr) expands the array at runtime.
inline JsNumber min(const JsArray& arr) {
    if (arr.length() == 0) return JsNumber(std::numeric_limits<double>::infinity());
    bool all_int = true;
    int64_t best_i = 0;
    double best_d = 0.0;
    bool first = true;
    for (const auto& e : arr) {
        JsNumber n = js_to_number(e);
        double d = n.as_double();
        if (d != d) return JsNumber(d);
        if (!n.is_int()) all_int = false;
        if (first || d < best_d) {
            best_d = d;
            if (n.is_int()) best_i = std::get<int64_t>(n.value);
            first = false;
        }
    }
    if (all_int) return JsNumber(best_i);
    return JsNumber(best_d);
}

inline JsNumber max(const JsArray& arr) {
    if (arr.length() == 0) return JsNumber(-std::numeric_limits<double>::infinity());
    bool all_int = true;
    int64_t best_i = 0;
    double best_d = 0.0;
    bool first = true;
    for (const auto& e : arr) {
        JsNumber n = js_to_number(e);
        double d = n.as_double();
        if (d != d) return JsNumber(d);
        if (!n.is_int()) all_int = false;
        if (first || d > best_d) {
            best_d = d;
            if (n.is_int()) best_i = std::get<int64_t>(n.value);
            first = false;
        }
    }
    if (all_int) return JsNumber(best_i);
    return JsNumber(best_d);
}

// Mutated bindings lower as shared_ptr<JsArray>: spread over those too.
inline JsNumber min(const std::shared_ptr<JsArray>& arr) {
    if (!arr || arr->length() == 0) return JsNumber(std::numeric_limits<double>::infinity());
    return min(*arr);
}

inline JsNumber max(const std::shared_ptr<JsArray>& arr) {
    if (!arr || arr->length() == 0) return JsNumber(-std::numeric_limits<double>::infinity());
    return max(*arr);
}

inline JsNumber abs(const JsValue& v) {
    JsNumber n = js_to_number(v);
    if (n.is_int()) {
        int64_t i = std::get<int64_t>(n.value);
        return JsNumber(i < 0 ? -i : i);
    }
    return JsNumber(std::fabs(n.as_double()));
}

inline JsNumber floor(const JsValue& v) { return JsNumber(std::floor(num(v))); }
inline JsNumber ceil(const JsValue& v) { return JsNumber(std::ceil(num(v))); }

inline JsNumber round(const JsValue& v) {
    double d = num(v);
    if (d != d || !std::isfinite(d)) return JsNumber(d);
    return JsNumber(std::floor(d + 0.5));
}

inline JsNumber pow(const JsValue& x, const JsValue& y) {
    return JsNumber(std::pow(num(x), num(y)));
}

inline JsNumber sqrt(const JsValue& v) { return JsNumber(std::sqrt(num(v))); }
inline JsNumber trunc(const JsValue& v) { return JsNumber(std::trunc(num(v))); }
inline JsNumber cbrt(const JsValue& v) {
    double d = num(v);
    double r = std::cbrt(d);
    // glibc can return one ulp above an exact cube (cbrt(27) =>
    // 3.0000000000000004); JS prints the integer. Snap back only when the
    // rounded value cubes exactly to the input.
    double rounded = std::nearbyint(r);
    if (rounded * rounded * rounded == d) return JsNumber(rounded);
    return JsNumber(r);
}
inline JsNumber exp(const JsValue& v) { return JsNumber(std::exp(num(v))); }
inline JsNumber expm1(const JsValue& v) { return JsNumber(std::expm1(num(v))); }
inline JsNumber log(const JsValue& v) { return JsNumber(std::log(num(v))); }
inline JsNumber log2(const JsValue& v) { return JsNumber(std::log2(num(v))); }
inline JsNumber log10(const JsValue& v) { return JsNumber(std::log10(num(v))); }
inline JsNumber log1p(const JsValue& v) { return JsNumber(std::log1p(num(v))); }
inline JsNumber sin(const JsValue& v) { return JsNumber(std::sin(num(v))); }
inline JsNumber cos(const JsValue& v) { return JsNumber(std::cos(num(v))); }
inline JsNumber tan(const JsValue& v) { return JsNumber(std::tan(num(v))); }
inline JsNumber asin(const JsValue& v) { return JsNumber(std::asin(num(v))); }
inline JsNumber acos(const JsValue& v) { return JsNumber(std::acos(num(v))); }
inline JsNumber atan(const JsValue& v) { return JsNumber(std::atan(num(v))); }
inline JsNumber sinh(const JsValue& v) { return JsNumber(std::sinh(num(v))); }
inline JsNumber cosh(const JsValue& v) { return JsNumber(std::cosh(num(v))); }
inline JsNumber tanh(const JsValue& v) { return JsNumber(std::tanh(num(v))); }
inline JsNumber fround(const JsValue& v) {
    return JsNumber(static_cast<double>(static_cast<float>(num(v))));
}

inline JsNumber atan2(const JsValue& y, const JsValue& x) {
    return JsNumber(std::atan2(num(y), num(x)));
}

// Math.sign preserves -0, which matters for 1 / Math.sign(-0).
inline JsNumber sign(const JsValue& v) {
    double d = num(v);
    if (d != d || d == 0.0) return JsNumber(d);
    return JsNumber(d > 0.0 ? 1 : -1);
}

// JsNumber random() returns [0, 1). The engine is per-thread and seeded
// once from random_device.
inline JsNumber random() {
    static thread_local std::mt19937 engine{std::random_device{}()};
    static thread_local std::uniform_real_distribution<double> distribution(0.0, 1.0);
    return JsNumber(distribution(engine));
}

// Math.hypot is variadic. Infinity wins over NaN, matching the spec.
inline JsNumber hypot(std::initializer_list<JsValue> args) {
    bool has_nan = false;
    double result = 0.0;
    for (const auto& a : args) {
        double d = num(a);
        if (std::isinf(d)) return JsNumber(std::numeric_limits<double>::infinity());
        if (std::isnan(d)) {
            has_nan = true;
            continue;
        }
        result = std::hypot(result, d);
    }
    if (has_nan) return JsNumber(std::numeric_limits<double>::quiet_NaN());
    return JsNumber(result);
}

} // namespace js_math
} // namespace morph
