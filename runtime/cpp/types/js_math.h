#pragma once
#include <cmath>
#include <initializer_list>
#include <limits>
#include "js_convert.h"

// JS Math: min/max over an argument list, unary/binary numeric ops.
// Non-numbers coerce via Number() (so Math.abs("x") is NaN, not 0).
// Semantics verified against node (see types verification fixture).

namespace morph {
namespace js_math {

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
inline JsNumber min(const JsArray& arr) {    if (arr.length() == 0) return JsNumber(std::numeric_limits<double>::infinity());
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

} // namespace js_math
} // namespace morph
