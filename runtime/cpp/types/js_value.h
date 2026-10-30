#pragma once
#include <variant>
#include <string>
#include <cstdint>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <algorithm>
#include "js_number.h"
#include "js_boolean.h"
#include "js_string.h"
#include "js_array.h"
#include "js_object.h"

// ── Helper types ──

struct JsValue;
using JsFunction = JsValue(*)(JsValue);

struct JsUndefined {
    bool operator==(const JsUndefined&) const { return true; }
    bool operator!=(const JsUndefined&) const { return false; }
};
struct JsNull {
    bool operator==(const JsNull&) const { return true; }
    bool operator!=(const JsNull&) const { return false; }
};

// JsString forward-declared operator+ with number:
// Implemented after JsValue is complete

// ── JsValue: top-level variant ──

// Array string keys (JS semantics, never throws): canonical all-digit
// keys are indices — a lone "0" is index 0, but "01" is a named
// property (ToString(ToUint32("01")) is "1", not "01"). Capped,
// bounds-checked by the caller. "", "-1", "1.5" are never indices.
static bool array_key_index(const std::string& key, size_t& idx) {
    if (key.empty()) return false;
    if (key.size() > 1 && key[0] == '0') return false;
    size_t v = 0;
    for (unsigned char c : key) {
        if (c < '0' || c > '9') return false;
        v = v * 10 + (size_t)(c - '0');
        if (v >= (size_t)INT64_MAX) return false;
    }
    idx = v;
    return true;
}

struct JsValue {
    std::variant<
        JsUndefined,
        JsNull,
        JsBoolean,
        JsNumber,
        JsString,
        JsArray,
        JsObject,
        JsFunction
    > inner;

    JsValue() : inner(JsUndefined{}) {}
    JsValue(JsUndefined v) : inner(v) {}
    JsValue(JsNull v) : inner(v) {}
    JsValue(JsBoolean v) : inner(v) {}
    JsValue(JsNumber v) : inner(v) {}
    JsValue(JsString v) : inner(v) {}
    JsValue(JsArray v) : inner(v) {}
    JsValue(JsObject v) : inner(v) {}
    JsValue(JsFunction v) : inner(v) {}

    // Implicit from primitives
    JsValue(bool v) : inner(JsBoolean(v)) {}
    JsValue(int v) : inner(JsNumber(v)) {}
    JsValue(int64_t v) : inner(JsNumber(v)) {}
    JsValue(double v) : inner(JsNumber(v)) {}
    JsValue(const char* v) : inner(JsString(v)) {}
    JsValue(const std::string& v) : inner(JsString(v)) {}

    // ── Type checks ──

    bool is_undefined() const { return std::holds_alternative<JsUndefined>(inner); }
    bool is_null() const { return std::holds_alternative<JsNull>(inner); }
    bool is_boolean() const { return std::holds_alternative<JsBoolean>(inner); }
    bool is_number() const { return std::holds_alternative<JsNumber>(inner); }
    bool is_string() const { return std::holds_alternative<JsString>(inner); }
    bool is_array() const { return std::holds_alternative<JsArray>(inner); }
    bool is_object() const { return std::holds_alternative<JsObject>(inner); }
    bool is_function() const { return std::holds_alternative<JsFunction>(inner); }

    // ── JS typeof ──

    std::string typeof_() const {
        if (is_undefined()) return "undefined";
        if (is_null()) return "object";
        if (is_boolean()) return "boolean";
        if (is_number()) return "number";
        if (is_string()) return "string";
        if (is_array()) return "object";  // JS: typeof [] === "object"
        if (is_object()) return "object";
        if (is_function()) return "function";
        return "undefined";
    }

    // ── Truthy / Falsy ──

    bool truthy() const {
        if (is_undefined() || is_null()) return false;
        if (is_boolean()) return std::get<JsBoolean>(inner).value;
        if (is_number()) {
            auto& n = std::get<JsNumber>(inner);
            if (n.is_int()) return std::get<int64_t>(n.value) != 0;
            if (n.is_big()) return true;  // big non-zero string = truthy
            double d = n.as_double();
            return d != 0.0 && d == d;  // NaN is falsy
        }
        if (is_string()) return !std::get<JsString>(inner).empty();
        // Objects are always truthy in JS, empty arrays included.
        if (is_array()) return true;
        if (is_object()) return true;
        if (is_function()) return true;
        return false;
    }

    explicit operator bool() const { return truthy(); }
    bool operator!() const { return !truthy(); }

    // ── Total coercions (never throw; unconvertible types yield zero values)
    //
    // Numbers and booleans convert freely; strings convert only via
    // as_string (a string is never silently parsed as a number — that
    // hides caller bugs). Used by route prop extraction and any future
    // dynamic reads; copies of JsArray/JsObject share (no deep copy).

    int64_t as_int() const {
        if (is_number()) return std::get<JsNumber>(inner).as_int();
        if (is_boolean()) return std::get<JsBoolean>(inner).value ? 1 : 0;
        return 0;
    }

    double as_double() const {
        if (is_number()) return std::get<JsNumber>(inner).as_double();
        if (is_boolean()) return std::get<JsBoolean>(inner).value ? 1.0 : 0.0;
        return 0.0;
    }

    bool as_bool() const { return truthy(); }

    std::string as_string() const {
        if (is_string()) return std::get<JsString>(inner).value;
        if (is_number()) return std::get<JsNumber>(inner).as_string();
        if (is_boolean()) return std::get<JsBoolean>(inner).value ? "true" : "false";
        if (is_null()) return "null";
        return "";
    }

    JsArray as_array() const {
        if (is_array()) return std::get<JsArray>(inner);
        return JsArray{};
    }

    JsObject as_object() const {
        if (is_object()) return std::get<JsObject>(inner);
        return JsObject{};
    }

    // ── Equality (JS == semantics simplified) ──

    bool operator==(const JsValue& o) const {
        if (is_number() && o.is_number()) {
            return std::get<JsNumber>(inner) == std::get<JsNumber>(o.inner);
        }
        if (is_string() && o.is_string()) {
            return std::get<JsString>(inner) == std::get<JsString>(o.inner);
        }
        if (is_boolean() && o.is_boolean()) {
            return std::get<JsBoolean>(inner) == std::get<JsBoolean>(o.inner);
        }
        // Object/array comparison: same identity (shared_ptr comparison)
        if (is_object() && o.is_object()) {
            return std::get<JsObject>(inner).properties
                == std::get<JsObject>(o.inner).properties;
        }
        if (is_array() && o.is_array()) {
            return std::get<JsArray>(inner).elements
                == std::get<JsArray>(o.inner).elements;
        }
        if (is_null() && o.is_null()) return true;
        if (is_undefined() && o.is_undefined()) return true;
        return false;
    }

    bool operator!=(const JsValue& o) const { return !(*this == o); }

    // ── Strict equality (===) ──

    bool strict_eq(const JsValue& o) const {
        return operator==(o);  // simplified: same as == for now
    }

    // ── Property access (delegates to JsObject/JsArray) ──

    JsValue get(const std::string& key) const {
        if (is_object()) return std::get<JsObject>(inner).get(key);
        return JsValue(JsUndefined{});
    }

    JsValue get(int64_t idx) const {
        if (is_array()) return std::get<JsArray>(inner)[idx];
        return JsValue(JsUndefined{});
    }

    JsValue operator[](const std::string& key) const {
        if (is_object()) return std::get<JsObject>(inner).get(key);
        if (is_array()) {
            const auto& arr = std::get<JsArray>(inner);
            if (key == "length") return JsValue(JsNumber((int64_t)arr.length()));
            size_t idx = 0;
            // Bounds-checked below: OOB digits read undefined, never throw.
            if (array_key_index(key, idx)) return arr[(int64_t)idx];
            return JsValue(JsUndefined{});
        }
        return JsValue(JsUndefined{});
    }

    JsValue operator[](int64_t idx) const {
        return get(idx);
    }

    // Non-const access: returns mutable reference for chained mutation (obj["a"]["b"] = v)
    JsValue& operator[](const std::string& key) {
        if (is_object()) return std::get<JsObject>(inner)[key];
        if (is_array()) {
            auto& arr = std::get<JsArray>(inner);
            // Length reads through a scratch (writes to it are dropped;
            // mirrors the OOB-write limitation on indexed access below).
            static thread_local JsValue length_scratch;
            if (key == "length") {
                length_scratch = JsValue(JsNumber((int64_t)arr.length()));
                return length_scratch;
            }
            size_t idx = 0;
            if (array_key_index(key, idx)) return arr[(int64_t)idx];
            return get_mutable_dummy();
        }
        return get_mutable_dummy();
    }

    JsValue& operator[](int64_t idx) {
        if (is_array()) return std::get<JsArray>(inner)[idx];
        return get_mutable_dummy();
    }

    // ── Function call ──

    JsValue operator()(JsValue thisArg) const {
        if (is_function()) return std::get<JsFunction>(inner)(thisArg);
        return JsValue(JsUndefined{});
    }

    // ── Length ──

    size_t length() const {
        if (is_string()) return std::get<JsString>(inner).value.size();
        if (is_array()) return std::get<JsArray>(inner).length();
        return 0;
    }
    size_t size() const { return length(); }

    // ── String method forwarding (fast-path: single branch, no string dispatch) ──

    JsString toUpperCase() const {
        if (is_string()) return std::get<JsString>(inner).toUpperCase();
        return JsString();
    }
    JsString toLowerCase() const {
        if (is_string()) return std::get<JsString>(inner).toLowerCase();
        return JsString();
    }
    JsString trim() const {
        if (is_string()) return std::get<JsString>(inner).trim();
        return JsString();
    }
    JsString charAt(int64_t idx) const {
        if (is_string()) return std::get<JsString>(inner).charAt(idx);
        return JsString();
    }
    int64_t indexOf(const JsString& sub, int64_t from = 0) const {
        if (is_string()) return std::get<JsString>(inner).indexOf(sub, from);
        return -1;
    }
    JsString substring(int64_t start, int64_t end = -1) const {
        if (is_string()) return std::get<JsString>(inner).substring(start, end);
        return JsString();
    }
    JsString slice(int64_t start, int64_t end = -1) const {
        if (is_string()) return std::get<JsString>(inner).slice(start, end);
        return JsString();
    }
    JsString replace(const JsString& search, const JsString& replacement) const {
        if (is_string()) return std::get<JsString>(inner).replace(search, replacement);
        return JsString();
    }
    std::vector<JsString> split(const JsString& delim) const {
        if (is_string()) return std::get<JsString>(inner).split(delim);
        return {};
    }

    // ── toString: delegates to inner type ──

    JsString toString() const {
        if (is_undefined()) return JsString("undefined");
        if (is_null()) return JsString("null");
        if (is_boolean()) return std::get<JsBoolean>(inner).toString();
        if (is_number()) return std::get<JsNumber>(inner).toString();
        if (is_string()) return std::get<JsString>(inner).toString();
        if (is_array()) return JsString("[object Array]");
        return JsString("[object Object]");
    }

    // ── String conversion (JS implicit coercion) ──
    // Needed so `setErr(e.message)` / `"" + e` work when the JS value flows
    // into a std::string state. Defined after _js_to_string below.
    operator std::string() const;

    // ── Array method forwarding ──

    void push(const JsValue& item) {
        if (is_array()) std::get<JsArray>(inner).push(item);
    }
    JsValue pop() {
        if (is_array()) return std::get<JsArray>(inner).pop();
        return JsValue(JsUndefined{});
    }
    JsString join() const;
    JsString join(const JsValue& sep) const;
    JsArray concat(std::initializer_list<JsValue> items) const;
    JsArray concat(const JsArray& other) const;
    JsArray concat(const std::shared_ptr<JsArray>& other) const;
    bool includes(const JsValue& item, int64_t from = 0) const;
    bool includes(const JsValue& item, const JsValue& from) const;
    int64_t indexOf(const JsValue& item, int64_t from = 0) const;
    int64_t indexOf(const JsValue& item, const JsValue& from) const;
    int64_t lastIndexOf(const JsValue& item) const;
    int64_t lastIndexOf(const JsValue& item, int64_t from) const;
    int64_t lastIndexOf(const JsValue& item, const JsValue& from) const;
    JsValue at(int64_t idx) const;
    JsValue at(const JsValue& idx) const;
    JsValue at() const;
    JsString toFixed(int64_t digits = 0) const;
    JsString toFixed(const JsValue& digits) const;
    JsString toPrecision() const;
    JsString toPrecision(int64_t sig) const;
    JsString toPrecision(const JsValue& sig) const;
    JsString toExponential() const;
    JsString toExponential(int64_t frac) const;
    JsString toExponential(const JsValue& frac) const;
    JsValue reverse() {
        if (is_array()) {
            std::get<JsArray>(inner).reverse();
            return *this;
        }
        return JsValue(JsUndefined{});
    }

    // ── Object method forwarding ──

    bool has(const std::string& key) const {
        if (is_object()) return std::get<JsObject>(inner).has(key);
        return false;
    }
    std::vector<std::string> keys() const {
        if (is_object()) return std::get<JsObject>(inner).keys();
        return {};
    }
    JsArray values() const;
    JsArray entries() const;

private:
    static JsValue& get_mutable_dummy() {
        static JsValue dummy;
        return dummy;
    }

public:
    // Sorted keys for `for-in` (objects only; anything else enumerates
    // nothing). Same order the old std::map iteration produced.
    std::vector<std::string> sorted_keys() const {
        if (is_object()) return std::get<JsObject>(inner).sorted_keys();
        return {};
    }
};

// ── JsString methods that depend on JsValue ──

inline std::vector<JsString> JsString::split(const JsString& delim) const {
    std::vector<JsString> result;
    size_t start = 0, end;
    while ((end = value.find(delim.value, start)) != std::string::npos) {
        result.push_back(value.substr(start, end - start));
        start = end + delim.value.length();
    }
    result.push_back(value.substr(start));
    return result;
}

inline JsString::JsString(const JsNumber& num) {
    value = num.as_string();
}

// Throw a catchable RangeError (the translator lowers try/catch to
// catch (JsValue&), so fixtures can assert the error name). Apps that
// call throwing helpers get -fexceptions via the build scan; under
// -fno-exceptions this still compiles (that configuration never calls it).
[[noreturn]] inline void throw_range_error(const char* what) {
#ifdef __cpp_exceptions
    throw JsValue(JsObject{{"name", JsValue("RangeError")}, {"message", JsValue(what)}});
#else
    (void)what;
    std::abort();
#endif
}


inline JsString JsNumber::toFixed(int64_t digits) const {
    if (digits < 0 || digits > 100) throw_range_error("toFixed() digits out of range");
    double d = as_double();
    if (!std::isfinite(d) || std::fabs(d) >= 1e21) return as_string();
    return JsString(js_format_fixed(d, (int)digits));
}

inline JsString JsNumber::toPrecision() const { return as_string(); }

inline JsString JsNumber::toPrecision(int64_t sig) const {
    if (sig < 1 || sig > 100) throw_range_error("toPrecision() precision out of range");
    double d = as_double();
    if (!std::isfinite(d)) return as_string();
    if (d == 0.0) {
        if (sig == 1) return JsString("0");
        return JsString("0." + std::string((size_t)sig - 1, '0'));
    }
    bool neg = d < 0;
    double mag = neg ? -d : d;
    int p = (int)sig;
    int64_t e = js_exp10(mag);
    std::string digits;
    if (p <= 18) {
        int64_t ee = e;
        unsigned long long s = js_significand(mag, p, ee);
        e = ee;
        digits = std::to_string(s);
        while ((int)digits.size() < p) digits = "0" + digits;
    } else {
        // Beyond double precision: shortest digits, zero-padded.
        std::string sd;
        int ex = 0;
        js_shortest(mag, sd, ex);
        e = ex;
        digits = sd;
        while ((int)digits.size() < p) digits += '0';
    }
    std::string out = neg ? "-" : "";
    if (e < -6 || e >= sig) {
        out += digits[0];
        if (p > 1) {
            out += '.';
            out += digits.substr(1);
        }
        out += 'e';
        out += e < 0 ? "-" : "+";
        out += std::to_string(e < 0 ? -e : e);
        return JsString(out);
    }
    if (e >= 0) {
        std::string head = digits.substr(0, (size_t)e + 1);
        std::string tail = digits.size() > (size_t)e + 1 ? digits.substr((size_t)e + 1) : "";
        out += head;
        if (!tail.empty()) {
            out += '.';
            out += tail;
        }
        return JsString(out);
    }
    out += "0.";
    out += std::string((size_t)(-e - 1), '0');
    out += digits;
    return JsString(out);
}

inline JsString JsNumber::toExponential() const {
    double d = as_double();
    if (!std::isfinite(d)) return as_string();
    bool neg = d < 0;
    double mag = neg ? -d : d;
    if (mag == 0.0) return JsString("0e+0");
    // Shortest mantissa that round-trips.
    std::string digits;
    int exp10 = 0;
    js_shortest(mag, digits, exp10);
    std::string out = neg ? "-" : "";
    out += digits[0];
    if (digits.size() > 1) {
        out += '.';
        out += digits.substr(1);
    }
    out += 'e';
    out += exp10 < 0 ? "-" : "+";
    out += std::to_string(exp10 < 0 ? -exp10 : exp10);
    return JsString(out);
}

inline JsString JsNumber::toExponential(int64_t frac) const {
    if (frac < 0 || frac > 100) throw_range_error("toExponential() fraction digits out of range");
    double d = as_double();
    if (!std::isfinite(d)) return as_string();
    bool neg = d < 0;
    double mag = neg ? -d : d;
    if (mag == 0.0) {
        std::string m = frac > 0 ? "0." + std::string((size_t)frac, '0') : "0";
        return JsString(std::string(neg ? "-" : "") + m + "e+0");
    }
    int64_t e = js_exp10(mag);
    std::string m = js_format_fixed(mag / std::pow(10.0, (double)e), (int)frac);
    if (m.size() > 2 && m[0] == '1' && m[1] == '0' && m[2] == '.') {
        m = "1." + std::string((size_t)frac, '0');
        ++e;
    } else if (m == "10") {
        m = "1";
        ++e;
    }
    std::string out = neg ? "-" : "";
    out += m;
    out += 'e';
    out += e < 0 ? "-" : "+";
    out += std::to_string(e < 0 ? -e : e);
    return JsString(out);
}

// ── JsNumber assignment/construction from JsValue ──

inline JsNumber& JsNumber::operator=(const JsValue& v) {
    if (v.is_number()) {
        auto& n = std::get<JsNumber>(v.inner);
        if (n.is_int()) value = std::get<int64_t>(n.value);
        else if (n.is_big()) value = std::get<std::string>(n.value);
        else value = std::get<double>(n.value);
    }
    // If not a number, leave as-is (JS allows this loosely)
    return *this;
}

inline JsNumber::JsNumber(const JsValue& v) : value(int64_t(0)) {
    *this = v;
}

// ── JsArray methods that depend on JsValue ──

inline void JsArray::push(const JsValue& item) {
    elements->push_back(item);
}

// JS Array.prototype.slice — copies [start, end) into a new JsArray.
// Negative indices count from the end; end may be omitted (to the end).
inline JsArray JsArray::slice(int64_t start, int64_t end) const {
    int64_t len = (int64_t)elements->size();
    if (start < 0) start = std::max<int64_t>(len + start, 0);
    if (end < 0) end = std::max<int64_t>(len + end, 0);
    end = std::min<int64_t>(end, len);
    if (start >= end) return JsArray{};
    JsArray out;
    out.elements->reserve((size_t)(end - start));
    for (int64_t i = start; i < end; ++i)
        out.elements->push_back((*elements)[(size_t)i]);
    return out;
}

inline JsValue JsArray::pop() {
    if (elements->empty()) return JsValue(JsUndefined{});
    auto back = elements->back();
    elements->pop_back();
    return back;
}

// Recursive join element: null/undefined → "", nested arrays join with
// ",", everything else uses JS string coercion. Mirrors String() on arrays.
static std::string js_join_element(const JsValue& v);

inline JsString JsArray::join(const JsValue& sep) const {
    std::string s = sep.as_string();
    std::string out;
    for (size_t i = 0; i < elements->size(); ++i) {
        if (i > 0) out += s;
        out += js_join_element((*elements)[i]);
    }
    return JsString(out);
}

inline JsString JsArray::join() const { return join(JsValue(",")); }

inline JsArray JsArray::concat(std::initializer_list<JsValue> items) const {
    JsArray out;
    for (const auto& e : *elements) out.elements->push_back(e);
    for (const auto& item : items) {
        if (item.is_array()) {
            for (const auto& e : *std::get<JsArray>(item.inner).elements)
                out.elements->push_back(e);
        } else {
            out.elements->push_back(item);
        }
    }
    return out;
}

inline JsArray JsArray::concat(const JsArray& other) const {
    JsArray out;
    for (const auto& e : *elements) out.elements->push_back(e);
    for (const auto& e : *other.elements) out.elements->push_back(e);
    return out;
}

inline JsArray JsArray::concat(const std::shared_ptr<JsArray>& other) const {
    if (!other) {
        JsArray out;
        for (const auto& e : *elements) out.elements->push_back(e);
        return out;
    }
    return concat(*other);
}

// SameValueZero: strict equal, or both NaN.
static bool js_same_value_zero(const JsValue& a, const JsValue& b) {
    if (a == b) return true;
    if (a.is_number() && b.is_number()) {
        double x = std::get<JsNumber>(a.inner).as_double();
        double y = std::get<JsNumber>(b.inner).as_double();
        return x != x && y != y;
    }
    return false;
}

inline bool JsArray::includes(const JsValue& item, int64_t from) const {
    int64_t len = (int64_t)elements->size();
    int64_t start = from < 0 ? std::max<int64_t>(len + from, 0) : std::min<int64_t>(from, len);
    for (int64_t i = start; i < len; ++i) {
        if (js_same_value_zero((*elements)[(size_t)i], item)) return true;
    }
    return false;
}

inline int64_t JsArray::indexOf(const JsValue& item, int64_t from) const {
    int64_t len = (int64_t)elements->size();
    int64_t start = from < 0 ? std::max<int64_t>(len + from, 0) : std::min<int64_t>(from, len);
    for (int64_t i = start; i < len; ++i) {
        if ((*elements)[(size_t)i] == item) return i;
    }
    return -1;
}

inline int64_t JsArray::lastIndexOf(const JsValue& item) const {
    return lastIndexOf(item, (int64_t)elements->size() - 1);
}

inline int64_t JsArray::lastIndexOf(const JsValue& item, int64_t from) const {
    int64_t len = (int64_t)elements->size();
    if (len == 0) return -1;
    int64_t start = from < 0 ? std::max<int64_t>(len + from, 0) : std::min<int64_t>(from, len - 1);
    for (int64_t i = start; i >= 0; --i) {
        if ((*elements)[(size_t)i] == item) return i;
    }
    return -1;
}

inline JsValue JsArray::at(int64_t idx) const {
    int64_t len = (int64_t)elements->size();
    if (idx < 0) idx = len + idx;
    if (idx < 0 || idx >= len) return JsValue(JsUndefined{});
    return (*elements)[(size_t)idx];
}

inline JsArray& JsArray::reverse() {
    std::reverse(elements->begin(), elements->end());
    return *this;
}

inline JsString JsArray::toString() const { return join(JsValue(",")); }

namespace morph::strutil {
inline std::string to_string(const JsArray& a) {
    return a.join(JsValue(",")).value;
}
} // namespace morph::strutil

inline void JsArray::clear() { elements->clear(); }

inline JsValue JsArray::operator[](int64_t idx) const {
    if (idx < 0 || (size_t)idx >= elements->size()) return JsValue(JsUndefined{});
    return (*elements)[idx];
}

inline JsValue& JsArray::operator[](int64_t idx) {
    // JS OOB read returns undefined (no throw, no resize to preserve length for reads).
    // Note: OOB write via `arr[i] = x` will write to a shared undefined singleton (known limitation;
    // no indexed writes in current fixtures). Future: return proxy for full JS write-extend semantics.
    static thread_local JsValue oob_undefined = JsValue(JsUndefined{});
    if (idx < 0 || (size_t)idx >= elements->size()) return oob_undefined;
    return (*elements)[idx];
}

inline JsValue JsArray::operator[](const JsNumber& idx) const {
    return (*this)[idx.as_int()];
}

inline JsValue& JsArray::operator[](const JsNumber& idx) {
    return (*this)[idx.as_int()];
}

inline JsValue JsArray::operator[](const std::string& key) const {
    if (key == "length") return JsValue(JsNumber((int64_t)length()));
    size_t idx = 0;
    if (array_key_index(key, idx)) return (*this)[(int64_t)idx];
    return JsValue(JsUndefined{});
}

inline JsValue& JsArray::operator[](const std::string& key) {
    static thread_local JsValue length_scratch;
    static thread_local JsValue missing;
    if (key == "length") {
        length_scratch = JsValue(JsNumber((int64_t)length()));
        return length_scratch;
    }
    size_t idx = 0;
    if (array_key_index(key, idx)) return (*this)[(int64_t)idx];
    return missing;
}

// Default constructors live here (not in js_array.h / js_object.h) because
// make_shared<vector<JsValue>> / make_shared<unordered_map<string, JsValue>>
// need a complete JsValue — clang rejects the instantiation from the earlier point.
inline JsArray::JsArray()
    : elements(std::make_shared<std::vector<JsValue>>()) {}

inline size_t JsArray::length() const { return elements->size(); }

inline bool JsArray::empty() const { return elements->empty(); }

inline JsArray JsArray::slice(int64_t start) const {
    return slice(start, (int64_t)elements->size());
}

inline JsArray::JsArray(std::initializer_list<JsValue> init)
    : JsArray() {
    for (const auto& val : init) {
        elements->push_back(val);
    }
}

// ── JsObject methods that depend on JsValue ──

inline JsValue JsObject::get(const std::string& key) const {
    auto it = properties->find(key);
    if (it != properties->end()) return it->second;
    return JsValue(JsUndefined{});
}

inline void JsObject::set(const std::string& key, const JsValue& val) {
    (*properties)[key] = val;
}

inline void JsObject::clear() { properties->clear(); }

inline JsObject::JsObject()
    : properties(std::make_shared<std::unordered_map<std::string, JsValue>>()) {}

inline JsObject::JsObject(std::initializer_list<std::pair<const char*, JsValue>> init)
    : JsObject() {
    for (const auto& [key, val] : init) {
        (*properties)[key] = val;
    }
}

inline bool JsObject::has(const std::string& key) const {
    return properties->find(key) != properties->end();
}

inline JsValue JsObject::operator[](const std::string& key) const {
    return get(key);
}

inline JsValue& JsObject::operator[](const std::string& key) {
    return (*properties)[key];
}

inline std::vector<std::string> JsObject::keys() const {
    std::vector<std::string> k;
    for (const auto& [key, _] : *properties) k.push_back(key);
    return k;
}

inline std::vector<std::string> JsObject::sorted_keys() const {
    auto k = keys();
    std::sort(k.begin(), k.end());
    return k;
}

inline JsArray JsObject::values() const {
    // Sorted key order (see sorted_keys): deterministic enumeration.
    JsArray out;
    for (const auto& key : sorted_keys()) out.elements->push_back(get(key));
    return out;
}

inline JsArray JsObject::entries() const {
    JsArray out;
    for (const auto& key : sorted_keys()) {
        JsArray pair;
        pair.elements->push_back(JsValue(key));
        pair.elements->push_back(get(key));
        out.elements->push_back(JsValue(pair));
    }
    return out;
}

// ── JsValue array/object forwarding (defined after JsArray/JsObject) ──

inline JsString JsValue::join() const {
    if (is_array()) return std::get<JsArray>(inner).join();
    return JsString();
}

inline JsString JsValue::join(const JsValue& sep) const {
    if (is_array()) return std::get<JsArray>(inner).join(sep);
    return JsString();
}

inline JsArray JsValue::concat(std::initializer_list<JsValue> items) const {
    if (is_array()) return std::get<JsArray>(inner).concat(items);
    JsArray out;
    out.elements->push_back(*this);
    return out.concat(items);
}

inline JsArray JsValue::concat(const JsArray& other) const {
    if (is_array()) return std::get<JsArray>(inner).concat(other);
    JsArray out;
    out.elements->push_back(*this);
    for (const auto& e : other) out.elements->push_back(e);
    return out;
}

inline JsArray JsValue::concat(const std::shared_ptr<JsArray>& other) const {
    if (is_array()) return std::get<JsArray>(inner).concat(other);
    JsArray out;
    out.elements->push_back(*this);
    if (other) {
        for (const auto& e : *other) out.elements->push_back(e);
    }
    return out;
}

inline bool JsValue::includes(const JsValue& item, int64_t from) const {
    if (is_array()) return std::get<JsArray>(inner).includes(item, from);
    if (is_string()) {
        if (!item.is_string()) return false;
        const auto& s = std::get<JsString>(inner).value;
        const auto& sub = std::get<JsString>(item.inner).value;
        if (from < 0) from = 0;
        if ((size_t)from > s.size()) return false;
        return s.find(sub, (size_t)from) != std::string::npos;
    }
    return false;
}

inline int64_t JsValue::indexOf(const JsValue& item, int64_t from) const {
    if (is_array()) return std::get<JsArray>(inner).indexOf(item, from);
    return -1;
}

inline int64_t JsValue::lastIndexOf(const JsValue& item) const {
    if (is_array()) return std::get<JsArray>(inner).lastIndexOf(item);
    return -1;
}

inline int64_t JsValue::lastIndexOf(const JsValue& item, int64_t from) const {
    if (is_array()) return std::get<JsArray>(inner).lastIndexOf(item, from);
    return -1;
}

inline JsValue JsValue::at(int64_t idx) const {
    if (is_array()) return std::get<JsArray>(inner).at(idx);
    return JsValue(JsUndefined{});
}

inline JsArray JsValue::values() const {
    if (is_object()) return std::get<JsObject>(inner).values();
    return JsArray{};
}

inline JsArray JsValue::entries() const {
    if (is_object()) return std::get<JsObject>(inner).entries();
    return JsArray{};
}

// ── Helper: convert any JsValue to its JS string representation ──

static std::string _js_to_string(const JsValue& v) {
    if (v.is_undefined()) return "undefined";
    if (v.is_null()) return "null";
    if (v.is_boolean()) return std::get<JsBoolean>(v.inner).value ? "true" : "false";
    if (v.is_number()) return std::get<JsNumber>(v.inner).as_string();
    if (v.is_string()) return std::get<JsString>(v.inner).value;
    if (v.is_array()) return "[object Array]";
    if (v.is_object()) return "[object Object]";
    if (v.is_function()) return "function";
    return "undefined";
}

// Recursive join element for Array join/String(array): null/undefined
// vanish, nested arrays join with ",", everything else coerces.
static std::string js_join_element(const JsValue& v) {
    if (v.is_undefined() || v.is_null()) return "";
    if (v.is_array()) {
        const auto& arr = std::get<JsArray>(v.inner);
        std::string out;
        for (size_t i = 0; i < arr.length(); ++i) {
            if (i > 0) out += ",";
            out += js_join_element(arr[(int64_t)i]);
        }
        return out;
    }
    return _js_to_string(v);
}

inline JsValue::operator std::string() const {
    return _js_to_string(*this);
}

// ── JsValue + JsValue (JS semantics: string concat if either is string) ──

inline JsValue operator+(const JsValue& a, const JsValue& b) {
    if (a.is_string() || b.is_string()) {
        return JsValue(JsString(_js_to_string(a) + _js_to_string(b)));
    }
    if (a.is_number() && b.is_number()) {
        return JsValue(std::get<JsNumber>(a.inner) + std::get<JsNumber>(b.inner));
    }
    return JsValue(JsUndefined{});
}

inline JsString operator+(const JsString& a, const char* b) {
    return a.value + b;
}
inline JsString operator+(const char* a, const JsString& b) {
    return a + b.value;
}
inline JsString operator+(const JsString& a, const JsNumber& b) {
    return a.value + b.as_string();
}
inline JsString operator+(const JsNumber& a, const JsString& b) {
    return a.as_string() + b.value;
}
// int overloads to resolve ambiguity between JsNumber, JsValue, and int64_t
inline JsString operator+(const JsString& a, int b) {
    return a.value + std::to_string(b);
}
inline JsString operator+(int a, const JsString& b) {
    return std::to_string(a) + b.value;
}
// int64_t overloads — codegen emits (int64_t)(x.length()) etc.; without these,
// JsString + int64_t is ambiguous between the int and JsValue overloads
inline JsString operator+(const JsString& a, int64_t b) {
    return a.value + std::to_string(b);
}
inline JsString operator+(int64_t a, const JsString& b) {
    return std::to_string(a) + b.value;
}

// JsString + JsValue (disambiguate from JsValue+JsValue via implicit conversion)
inline JsString operator+(const JsString& a, const JsValue& b) {
    return a.value + _js_to_string(b);
}
inline JsString operator+(const JsValue& a, const JsString& b) {
    return _js_to_string(a) + b.value;
}
// string-literal + JsValue (`"" + payload.message`): without these the
// literal converts equally well to JsString and JsValue, so overload
// resolution is ambiguous and the expression does not compile.
inline JsString operator+(const char* a, const JsValue& b) {
    return std::string(a) + _js_to_string(b);
}
inline JsString operator+(const JsValue& a, const char* b) {
    return _js_to_string(a) + std::string(b);
}
inline JsString operator+(const std::string& a, const JsValue& b) {
    return a + _js_to_string(b);
}
inline JsString operator+(const JsValue& a, const std::string& b) {
    return _js_to_string(a) + b;
}

// ── JsValue arithmetic (extract numbers, fall back to undefined) ──

inline JsValue operator/(const JsValue& a, const JsValue& b) {
    if (a.is_number() && b.is_number())
        return JsValue(std::get<JsNumber>(a.inner) / std::get<JsNumber>(b.inner));
    return JsValue(JsUndefined{});
}
inline JsValue operator-(const JsValue& a, const JsValue& b) {
    if (a.is_number() && b.is_number())
        return JsValue(std::get<JsNumber>(a.inner) - std::get<JsNumber>(b.inner));
    return JsValue(JsUndefined{});
}
inline JsValue operator*(const JsValue& a, const JsValue& b) {
    if (a.is_number() && b.is_number())
        return JsValue(std::get<JsNumber>(a.inner) * std::get<JsNumber>(b.inner));
    return JsValue(JsUndefined{});
}
inline JsValue operator%(const JsValue& a, const JsValue& b) {
    if (a.is_number() && b.is_number())
        return JsValue(std::get<JsNumber>(a.inner) % std::get<JsNumber>(b.inner));
    return JsValue(JsUndefined{});
}

// ── JsValue comparison with primitives ──

inline bool operator>=(const JsValue& a, int64_t b) {
    return a.is_number() && std::get<JsNumber>(a.inner).as_int() >= b;
}
inline bool operator<=(const JsValue& a, int64_t b) {
    return a.is_number() && std::get<JsNumber>(a.inner).as_int() <= b;
}
inline bool operator>(const JsValue& a, int64_t b) {
    return a.is_number() && std::get<JsNumber>(a.inner).as_int() > b;
}
inline bool operator<(const JsValue& a, int64_t b) {
    return a.is_number() && std::get<JsNumber>(a.inner).as_int() < b;
}
inline bool operator==(const JsValue& a, int64_t b) {
    return a.is_number() && std::get<JsNumber>(a.inner).as_int() == b;
}

// ── JsValue +-*/% with primitive types ──

inline JsValue operator/(const JsValue& a, int64_t b) { return a / JsValue(b); }
inline JsValue operator*(const JsValue& a, int64_t b) { return a * JsValue(b); }
inline JsValue operator-(const JsValue& a, int64_t b) { return a - JsValue(b); }
inline JsValue operator+(const JsValue& a, int64_t b) { return a + JsValue(b); }
inline JsValue operator%(const JsValue& a, int64_t b) { return a % JsValue(b); }
inline JsValue operator+(int64_t a, const JsValue& b) { return JsValue(a) + b; }
inline JsValue operator%(int64_t a, const JsValue& b) { return JsValue(a) % b; }
// Exact `int` overloads for disambiguation
inline JsValue operator/(const JsValue& a, int b) { return a / JsValue((int64_t)b); }
inline JsValue operator*(const JsValue& a, int b) { return a * JsValue((int64_t)b); }
inline JsValue operator-(const JsValue& a, int b) { return a - JsValue((int64_t)b); }
inline JsValue operator+(const JsValue& a, int b) { return a + JsValue((int64_t)b); }
inline JsValue operator%(const JsValue& a, int b) { return a % JsValue((int64_t)b); }
inline JsValue operator+(int a, const JsValue& b) { return JsValue((int64_t)a) + b; }
inline JsValue operator-(int a, const JsValue& b) { return JsValue((int64_t)a) - b; }
inline JsValue operator*(int a, const JsValue& b) { return JsValue((int64_t)a) * b; }
inline JsValue operator/(int a, const JsValue& b) { return JsValue((int64_t)a) / b; }
inline JsValue operator%(int a, const JsValue& b) { return JsValue((int64_t)a) % b; }

// ── JsNumber <-> JsValue interop (for arr[i] where arr[i] is JsValue) ──
inline JsNumber operator+(const JsNumber& a, const JsValue& b) {
    if (b.is_number()) return a + std::get<JsNumber>(b.inner);
    return a;
}
inline JsNumber operator+(const JsValue& a, const JsNumber& b) {
    if (a.is_number()) return std::get<JsNumber>(a.inner) + b;
    return b;
}
inline JsNumber operator-(const JsNumber& a, const JsValue& b) {
    if (b.is_number()) return a - std::get<JsNumber>(b.inner);
    return a;
}
inline JsNumber operator-(const JsValue& a, const JsNumber& b) {
    if (a.is_number()) return std::get<JsNumber>(a.inner) - b;
    return JsNumber(0) - b;
}
inline JsNumber operator*(const JsNumber& a, const JsValue& b) {
    if (b.is_number()) return a * std::get<JsNumber>(b.inner);
    return a;
}
inline JsNumber operator*(const JsValue& a, const JsNumber& b) {
    if (a.is_number()) return std::get<JsNumber>(a.inner) * b;
    return b;
}
inline JsNumber operator/(const JsNumber& a, const JsValue& b) {
    if (b.is_number()) return a / std::get<JsNumber>(b.inner);
    return a;
}
inline JsNumber operator/(const JsValue& a, const JsNumber& b) {
    if (a.is_number()) return std::get<JsNumber>(a.inner) / b;
    return JsNumber(0);
}
inline JsNumber operator%(const JsNumber& a, const JsValue& b) {
    if (b.is_number()) return a % std::get<JsNumber>(b.inner);
    return a;
}
inline JsNumber operator%(const JsValue& a, const JsNumber& b) {
    if (a.is_number()) return std::get<JsNumber>(a.inner) % b;
    return JsNumber(0);
}
inline JsNumber& operator+=(JsNumber& a, const JsValue& b) { a = a + b; return a; }
inline JsNumber& operator-=(JsNumber& a, const JsValue& b) { a = a - b; return a; }
inline JsNumber& operator*=(JsNumber& a, const JsValue& b) { a = a * b; return a; }
inline JsNumber& operator/=(JsNumber& a, const JsValue& b) { a = a / b; return a; }
inline JsNumber& operator%=(JsNumber& a, const JsValue& b) { a = a % b; return a; }
inline bool operator==(const JsNumber& a, const JsValue& b) { return b.is_number() && a == std::get<JsNumber>(b.inner); }
inline bool operator==(const JsValue& a, const JsNumber& b) { return a.is_number() && std::get<JsNumber>(a.inner) == b; }
inline bool operator!=(const JsNumber& a, const JsValue& b) { return !(a == b); }
inline bool operator!=(const JsValue& a, const JsNumber& b) { return !(a == b); }
inline bool operator<(const JsNumber& a, const JsValue& b) { return b.is_number() && a < std::get<JsNumber>(b.inner); }
inline bool operator<(const JsValue& a, const JsNumber& b) { return a.is_number() && std::get<JsNumber>(a.inner) < b; }
inline bool operator>(const JsNumber& a, const JsValue& b) { return b.is_number() && a > std::get<JsNumber>(b.inner); }
inline bool operator>(const JsValue& a, const JsNumber& b) { return a.is_number() && std::get<JsNumber>(a.inner) > b; }
inline bool operator<=(const JsNumber& a, const JsValue& b) { return b.is_number() && a <= std::get<JsNumber>(b.inner); }
inline bool operator<=(const JsValue& a, const JsNumber& b) { return a.is_number() && std::get<JsNumber>(a.inner) <= b; }
inline bool operator>=(const JsNumber& a, const JsValue& b) { return b.is_number() && a >= std::get<JsNumber>(b.inner); }
inline bool operator>=(const JsValue& a, const JsNumber& b) { return a.is_number() && std::get<JsNumber>(a.inner) >= b; }

// std::formatter specializations for Js* types live in
// "types/js_value_format.h" and are included by default via "js_types.h".
// Define MORPH_NO_FORMAT before including js_types.h to opt-out of the
// <format> parse cost (~1.5s per TU) in hot-reload critical paths.

// ── Helper: instanceof (JS semantics: prototype chain check) ──
// Simplified: checks if RHS constructor's prototype is in LHS's prototype chain.
// For morph's JsValue, we approximate by checking variant type or class identity.
inline bool js_instanceof(const JsValue& obj, const JsValue& ctor) {
    if (!ctor.is_function()) return false;
    // For morph's native classes wrapped in JsValue, the function carries the type info
    // This is a simplified implementation - real JS instanceof checks prototype chain
    if (obj.is_object()) {
        // Could check obj's constructor property vs ctor
        return true; // placeholder
    }
    if (obj.is_array() && ctor.is_function()) {
        // Check if ctor is Array constructor
        return true; // placeholder
    }
    return false;
}

namespace morph {
    inline bool js_instanceof(const JsValue& obj, const JsValue& ctor) {
        return ::js_instanceof(obj, ctor);
    }
}

// ── Helper: "in" operator (JS semantics: property existence in object) ──
inline bool js_has_property(const JsValue& obj, const JsValue& key) {
    if (obj.is_object()) {
        return std::get<JsObject>(obj.inner).has(key.as_string());
    }
    if (obj.is_array()) {
        // For arrays, "0" in arr checks index existence
        if (key.is_number()) {
            int64_t idx = key.as_int();
            return idx >= 0 && idx < std::get<JsArray>(obj.inner).length();
        }
        if (key.is_string() && key.as_string() == "length") return true;
        return false;
    }
    return false;
}

namespace morph {
    inline bool js_has_property(const JsValue& obj, const JsValue& key) {
        return ::js_has_property(obj, key);
    }

    // ── Destructuring helpers ──
    // Object rest: copy own enumerable keys except the destructured ones.
    inline JsObject js_object_rest(
        const JsObject& source,
        std::initializer_list<std::string> excluded)
    {
        JsObject rest;
        for (const auto& key : source.keys())
        {
            bool skip = false;
            for (const auto& name : excluded)
            {
                if (key == name)
                {
                    skip = true;
                    break;
                }
            }
            if (!skip)
            {
                rest.set(key, source.get(key));
            }
        }
        return rest;
    }

    // Object patterns can also destructure arrays: own enumerable indices
    // become string keys, and `length` is intentionally not copied.
    inline JsObject js_object_rest(
        const JsArray& source,
        std::initializer_list<std::string> excluded)
    {
        JsObject rest;
        for (size_t i = 0; i < source.length(); ++i)
        {
            std::string key = std::to_string(i);
            bool skip = false;
            for (const auto& name : excluded)
            {
                if (key == name)
                {
                    skip = true;
                    break;
                }
            }
            if (!skip)
            {
                rest.set(key, source[static_cast<int64_t>(i)]);
            }
        }
        return rest;
    }

    inline JsObject js_object_rest(
        const JsValue& source,
        std::initializer_list<std::string> excluded)
    {
        if (source.is_array())
        {
            return js_object_rest(source.as_array(), excluded);
        }
        return js_object_rest(source.as_object(), excluded);
    }

    // Array rest: the tail after the last destructured index.
    inline JsArray js_array_rest(const JsArray& source, size_t start)
    {
        return source.slice(static_cast<int64_t>(start));
    }

    inline JsArray js_array_rest(const JsValue& source, size_t start)
    {
        return js_array_rest(source.as_array(), start);
    }

    template <typename T>
    inline std::vector<T> js_array_rest(const std::vector<T>& source, size_t start)
    {
        if (start >= source.size())
        {
            return {};
        }
        return std::vector<T>(
            source.begin() + static_cast<typename std::vector<T>::difference_type>(start),
            source.end());
    }
}
// Codegen adds <format> only when `${}` is used.
