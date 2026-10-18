#pragma once
#include <string>
#include <cstdlib>
#include <cctype>
#include <cmath>
#include <limits>
#include "js_value.h"

// JS value conversions: String(x), Number(x), Boolean(x), parseInt,
// parseFloat and the Number.isFinite/isNaN/isInteger predicates.
// Semantics verified against node (see types verification fixture).

namespace morph {

// ── String(x): arrays join with ",", everything else coerces ──

inline JsString js_to_string() { return JsString(""); }

inline JsString js_to_string(const JsValue& v) {
    if (v.is_array()) {
        const auto& arr = std::get<JsArray>(v.inner);
        std::string out;
        for (size_t i = 0; i < arr.length(); ++i) {
            if (i > 0) out += ",";
            out += js_join_element(arr[(int64_t)i]);
        }
        return JsString(out);
    }
    return JsString(_js_to_string(v));
}

inline JsString js_to_string(const JsString& v) { return v; }
inline JsString js_to_string(const std::string& v) { return JsString(v); }
inline JsString js_to_string(const char* v) { return JsString(v); }
inline JsString js_to_string(double v) { return JsNumber(v).as_string(); }
inline JsString js_to_string(int v) { return JsNumber((int64_t)v).as_string(); }
inline JsString js_to_string(int64_t v) { return JsNumber(v).as_string(); }
inline JsString js_to_string(bool v) { return v ? JsString("true") : JsString("false"); }
inline JsString js_to_string(const JsNumber& v) { return v.as_string(); }
inline JsString js_to_string(const JsBoolean& v) { return v.toString(); }

// ── Number(x): JS ToNumber (trim, hex/binary/octal, "" → 0) ──

static double js_parse_number_text(const std::string& text, bool& ok) {
    size_t p = 0;
    while (p < text.size() && std::isspace((unsigned char)text[p])) ++p;
    size_t q = text.size();
    while (q > p && std::isspace((unsigned char)text[q - 1])) --q;
    if (p == q) {
        ok = true;
        return 0.0;
    }
    std::string s = text.substr(p, q - p);
    if (s == "Infinity" || s == "+Infinity") {
        ok = true;
        return std::numeric_limits<double>::infinity();
    }
    if (s == "-Infinity") {
        ok = true;
        return -std::numeric_limits<double>::infinity();
    }
    size_t base = 10;
    size_t start = 0;
    bool neg = false;
    if (!s.empty() && (s[0] == '+' || s[0] == '-')) {
        neg = (s[0] == '-');
        start = 1;
    }
    if (s.size() - start > 2 && s[start] == '0' &&
        (s[start + 1] == 'x' || s[start + 1] == 'X')) {
        base = 16;
        start += 2;
    } else if (s.size() - start > 2 && s[start] == '0' &&
               (s[start + 1] == 'b' || s[start + 1] == 'B')) {
        base = 2;
        start += 2;
    } else if (s.size() - start > 2 && s[start] == '0' &&
               (s[start + 1] == 'o' || s[start + 1] == 'O')) {
        base = 8;
        start += 2;
    }
    if (base != 10) {
        if (start >= s.size()) {
            ok = false;
            return std::numeric_limits<double>::quiet_NaN();
        }
        unsigned long long acc = 0;
        for (size_t i = start; i < s.size(); ++i) {
            char c = s[i];
            int digit = -1;
            if (c >= '0' && c <= '9') digit = c - '0';
            else if (c >= 'a' && c <= 'z') digit = c - 'a' + 10;
            else if (c >= 'A' && c <= 'Z') digit = c - 'A' + 10;
            if (digit < 0 || (size_t)digit >= base) {
                ok = false;
                return std::numeric_limits<double>::quiet_NaN();
            }
            acc = acc * base + (unsigned)digit;
        }
        ok = true;
        double d = (double)acc;
        return neg ? -d : d;
    }
    char* end = nullptr;
    double d = std::strtod(s.c_str(), &end);
    if (end != s.c_str() + s.size()) {
        ok = false;
        return std::numeric_limits<double>::quiet_NaN();
    }
    ok = true;
    return d;
}

inline JsNumber js_to_number(const JsValue& v) {
    if (v.is_number()) return std::get<JsNumber>(v.inner);
    if (v.is_boolean()) return JsNumber((int64_t)(std::get<JsBoolean>(v.inner).value ? 1 : 0));
    if (v.is_null()) return JsNumber((int64_t)0);
    if (v.is_undefined()) return JsNumber(std::numeric_limits<double>::quiet_NaN());
    if (v.is_string()) {
        bool ok = false;
        double d = js_parse_number_text(std::get<JsString>(v.inner).value, ok);
        if (!ok) return JsNumber(std::numeric_limits<double>::quiet_NaN());
        if (d == (double)(int64_t)d && std::fabs(d) < 9e15) return JsNumber((int64_t)d);
        return JsNumber(d);
    }
    if (v.is_array()) {
        // Number(arr) = Number(String(arr)): [] → 0, [5] → 5, [1,2] → NaN.
        const auto& arr = std::get<JsArray>(v.inner);
        if (arr.length() == 0) return JsNumber((int64_t)0);
        if (arr.length() == 1) return js_to_number(arr[(int64_t)0]);
        return JsNumber(std::numeric_limits<double>::quiet_NaN());
    }
    return JsNumber(std::numeric_limits<double>::quiet_NaN());
}

inline JsNumber js_to_number(double v) { return JsNumber(v); }
inline JsNumber js_to_number() { return JsNumber((int64_t)0); }
inline JsNumber js_to_number(int v) { return JsNumber((int64_t)v); }
inline JsNumber js_to_number(int64_t v) { return JsNumber(v); }
inline JsNumber js_to_number(bool v) { return JsNumber((int64_t)(v ? 1 : 0)); }
inline JsNumber js_to_number(const JsNumber& v) { return v; }
inline JsNumber js_to_number(const JsString& v) { return js_to_number(JsValue(v)); }
inline JsNumber js_to_number(const std::string& v) { return js_to_number(JsValue(v)); }
inline JsNumber js_to_number(const char* v) { return js_to_number(JsValue(v)); }

// ── ToIntegerOrInfinity (array indices, precision args): NaN → 0 ──

inline int64_t js_to_integer(const JsValue& v) {
    double d = js_to_number(v).as_double();
    if (d != d) return 0;
    if (d >= 9.223372036854776e18) return INT64_MAX;
    if (d <= -9.223372036854776e18) return INT64_MIN;
    return (int64_t)d;
}

// ── Boolean(x): truthiness ──

inline bool js_to_boolean(const JsValue& v) { return v.truthy(); }
inline bool js_to_boolean(bool v) { return v; }
inline bool js_to_boolean(double v) { return v != 0.0 && v == v; }
inline bool js_to_boolean(int v) { return v != 0; }
inline bool js_to_boolean(int64_t v) { return v != 0; }
inline bool js_to_boolean(const JsString& v) { return !v.value.empty(); }
inline bool js_to_boolean(const std::string& v) { return !v.empty(); }
inline bool js_to_boolean(const char* v) { return v && v[0] != '\0'; }
inline bool js_to_boolean(const JsNumber& v) {
    return js_to_boolean(JsValue(v));
}
inline bool js_to_boolean() { return false; }

// ── parseInt / parseFloat ──

static int js_digit_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'z') return c - 'a' + 10;
    if (c >= 'A' && c <= 'Z') return c - 'A' + 10;
    return -1;
}

static JsNumber js_parse_int_text(const std::string& text, int radix) {
    size_t p = 0;
    while (p < text.size() && std::isspace((unsigned char)text[p])) ++p;
    bool neg = false;
    if (p < text.size() && (text[p] == '+' || text[p] == '-')) {
        neg = (text[p] == '-');
        ++p;
    }
    int r = radix;
    if (r != 0 && (r < 2 || r > 36))
        return JsNumber(std::numeric_limits<double>::quiet_NaN());
    if (r == 0) {
        r = 10;
        if (text.size() - p > 2 && text[p] == '0' &&
            (text[p + 1] == 'x' || text[p + 1] == 'X')) {
            // 0x with no hex digits after it is NaN ("0xZZ"), not 0.
            if (p + 2 < text.size() && js_digit_val(text[p + 2]) >= 0 && js_digit_val(text[p + 2]) < 16) {
                r = 16;
                p += 2;
            } else {
                return JsNumber(std::numeric_limits<double>::quiet_NaN());
            }
        }
    } else if (r == 16) {
        if (text.size() - p > 2 && text[p] == '0' &&
            (text[p + 1] == 'x' || text[p + 1] == 'X')) {
            p += 2;
        }
    }
    long long acc = 0;
    bool any = false;
    while (p < text.size()) {
        int d = js_digit_val(text[p]);
        if (d < 0 || d >= r) break;
        any = true;
        acc = acc * r + d;
        ++p;
    }
    if (!any) return JsNumber(std::numeric_limits<double>::quiet_NaN());
    if (neg) acc = -acc;
    return JsNumber((int64_t)acc);
}

inline JsNumber js_parse_int(const JsValue& v, int radix = 0) {
    if (v.is_number() && radix == 0) {
        // parseInt(42) truncates toward zero like the string path would.
        double d = std::get<JsNumber>(v.inner).as_double();
        if (d != d) return JsNumber(d);
        return JsNumber((int64_t)(d > 0 ? std::floor(d) : std::ceil(d)));
    }
    return js_parse_int_text(_js_to_string(v), radix);
}

inline JsNumber js_parse_int(const JsString& v, int radix = 0) {
    return js_parse_int_text(v.value, radix);
}
inline JsNumber js_parse_int(const std::string& v, int radix = 0) {
    return js_parse_int_text(v, radix);
}
inline JsNumber js_parse_int(const char* v, int radix = 0) {
    return js_parse_int_text(v ? v : "", radix);
}

inline JsNumber js_parse_float(const JsValue& v) {
    if (v.is_number()) return std::get<JsNumber>(v.inner);
    std::string s = _js_to_string(v);
    size_t p = 0;
    while (p < s.size() && std::isspace((unsigned char)s[p])) ++p;
    std::string t = s.substr(p);
    if (t.starts_with("Infinity") || t.starts_with("+Infinity"))
        return JsNumber(std::numeric_limits<double>::infinity());
    if (t.starts_with("-Infinity"))
        return JsNumber(-std::numeric_limits<double>::infinity());
    char* end = nullptr;
    double d = std::strtod(t.c_str(), &end);
    if (end == t.c_str()) return JsNumber(std::numeric_limits<double>::quiet_NaN());
    return JsNumber(d);
}

inline JsNumber js_parse_float(const JsString& v) { return js_parse_float(JsValue(v)); }
inline JsNumber js_parse_float(const std::string& v) { return js_parse_float(JsValue(v)); }
inline JsNumber js_parse_float(const char* v) { return js_parse_float(JsValue(v)); }
inline JsNumber js_parse_float(double v) { return JsNumber(v); }

// ── Number.isFinite / isNaN / isInteger (no coercion) ──

inline bool js_is_finite(const JsNumber& v) {
    if (v.is_int()) return true;
    if (v.is_big()) {
        bool ok = false;
        double d = js_parse_number_text(std::get<std::string>(v.value), ok);
        return ok && std::isfinite(d);
    }
    double d = std::get<double>(v.value);
    return std::isfinite(d);
}

inline bool js_is_finite(const JsValue& v) {
    if (!v.is_number()) return false;
    return js_is_finite(std::get<JsNumber>(v.inner));
}

inline bool js_is_nan(const JsNumber& v) {
    if (v.is_int()) return false;
    if (v.is_big()) return false;
    double d = std::get<double>(v.value);
    return d != d;
}

inline bool js_is_nan(const JsValue& v) {
    if (!v.is_number()) return false;
    return js_is_nan(std::get<JsNumber>(v.inner));
}

inline bool js_is_integer(const JsNumber& v) {
    if (v.is_int()) return true;
    if (v.is_big()) {
        bool ok = false;
        double d = js_parse_number_text(std::get<std::string>(v.value), ok);
        return ok && std::isfinite(d) && std::floor(d) == d;
    }
    double d = std::get<double>(v.value);
    return std::isfinite(d) && std::floor(d) == d;
}

inline bool js_is_integer(const JsValue& v) {
    if (!v.is_number()) return false;
    return js_is_integer(std::get<JsNumber>(v.inner));
}

// ── Array.isArray: true only for real arrays (never throws) ──

inline bool js_is_array(const JsValue& v) { return v.is_array(); }
inline bool js_is_array(const JsArray&) { return true; }
inline bool js_is_array(const std::shared_ptr<JsArray>& v) { return v != nullptr; }
template <typename T>
inline bool js_is_array(const T&) { return false; }
template <typename T>
inline bool js_is_array(const std::shared_ptr<T>&) { return false; }

// ── JsValue-coercing overloads (defined here: js_to_integer lives here
// and js_value.h cannot see this header) ──

} // namespace morph

inline bool JsArray::includes(const JsValue& item, const JsValue& from) const {
    return includes(item, morph::js_to_integer(from));
}

inline int64_t JsArray::indexOf(const JsValue& item, const JsValue& from) const {
    return indexOf(item, morph::js_to_integer(from));
}

inline int64_t JsArray::lastIndexOf(const JsValue& item, const JsValue& from) const {
    return lastIndexOf(item, morph::js_to_integer(from));
}

inline JsValue JsArray::at(const JsValue& idx) const { return at(morph::js_to_integer(idx)); }

inline JsValue JsArray::at() const { return JsValue(JsUndefined{}); }

inline JsArray JsArray::slice(const JsValue& s, const JsValue& e) const {
    return slice(morph::js_to_integer(s), morph::js_to_integer(e));
}

inline JsArray JsArray::slice(const JsValue& s) const { return slice(morph::js_to_integer(s)); }

inline JsArray JsArray::slice() const { return slice((int64_t)0); }

inline JsString JsNumber::toFixed(const JsValue& d) const { return toFixed(morph::js_to_integer(d)); }

inline JsString JsNumber::toPrecision(const JsValue& s) const {
    return toPrecision(morph::js_to_integer(s));
}

inline JsString JsNumber::toExponential(const JsValue& f) const {
    return toExponential(morph::js_to_integer(f));
}

inline int64_t JsValue::indexOf(const JsValue& item, const JsValue& from) const {
    if (is_array()) return std::get<JsArray>(inner).indexOf(item, from);
    return -1;
}

inline int64_t JsValue::lastIndexOf(const JsValue& item, const JsValue& from) const {
    if (is_array()) return std::get<JsArray>(inner).lastIndexOf(item, from);
    return -1;
}

inline bool JsValue::includes(const JsValue& item, const JsValue& from) const {
    if (is_array()) return std::get<JsArray>(inner).includes(item, from);
    if (is_string()) {
        if (!item.is_string()) return false;
        const auto& s = std::get<JsString>(inner).value;
        const auto& sub = std::get<JsString>(item.inner).value;
        int64_t pos = morph::js_to_integer(from);
        if (pos < 0) pos = 0;
        if ((size_t)pos > s.size()) return false;
        return s.find(sub, (size_t)pos) != std::string::npos;
    }
    return false;
}

inline JsValue JsValue::at(const JsValue& idx) const {
    if (is_array()) return std::get<JsArray>(inner).at(idx);
    return JsValue(JsUndefined{});
}

inline JsValue JsValue::at() const { return JsValue(JsUndefined{}); }

inline JsString JsValue::toFixed(int64_t d) const {
    if (is_number()) return std::get<JsNumber>(inner).toFixed(d);
    return JsString("NaN");
}

inline JsString JsValue::toFixed(const JsValue& d) const { return toFixed(morph::js_to_integer(d)); }

inline JsString JsValue::toPrecision() const {
    if (is_number()) return std::get<JsNumber>(inner).toPrecision();
    return JsString("NaN");
}

inline JsString JsValue::toPrecision(int64_t s) const {
    if (is_number()) return std::get<JsNumber>(inner).toPrecision(s);
    return JsString("NaN");
}

inline JsString JsValue::toPrecision(const JsValue& s) const {
    return toPrecision(morph::js_to_integer(s));
}

inline JsString JsValue::toExponential() const {
    if (is_number()) return std::get<JsNumber>(inner).toExponential();
    return JsString("NaN");
}

inline JsString JsValue::toExponential(int64_t f) const {
    if (is_number()) return std::get<JsNumber>(inner).toExponential(f);
    return JsString("NaN");
}

inline JsString JsValue::toExponential(const JsValue& f) const {
    return toExponential(morph::js_to_integer(f));
}

namespace morph::strutil {
inline JsArray slice(const JsArray& a, const JsValue& b, const JsValue& c) {
    return a.slice(morph::js_to_integer(b), morph::js_to_integer(c));
}
inline JsArray slice(const JsArray& a, const JsValue& b) {
    return a.slice(morph::js_to_integer(b));
}
inline bool includes(const JsArray& a, const JsValue& b) {
    return a.includes(b);
}
inline int64_t index_of(const JsArray& a, const JsValue& b, const JsValue& c) {
    return a.indexOf(b, morph::js_to_integer(c));
}
inline int64_t last_index_of(const JsArray& a, const JsValue& b, const JsValue& c) {
    return a.lastIndexOf(b, morph::js_to_integer(c));
}
} // namespace morph::strutil
