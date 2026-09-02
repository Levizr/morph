#pragma once
#include <variant>
#include <string>
#include <cstdint>
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
            return n.as_double() != 0.0;
        }
        if (is_string()) return !std::get<JsString>(inner).empty();
        if (is_array()) return std::get<JsArray>(inner).length() > 0;
        if (is_object()) return true;
        if (is_function()) return true;
        return false;
    }

    explicit operator bool() const { return truthy(); }
    bool operator!() const { return !truthy(); }

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
        return get(key);
    }

    JsValue operator[](int64_t idx) const {
        return get(idx);
    }

    // Non-const access: returns mutable reference for chained mutation (obj["a"]["b"] = v)
    JsValue& operator[](const std::string& key) {
        if (is_object()) return std::get<JsObject>(inner)[key];
        if (is_array()) return std::get<JsArray>(inner)[std::stoll(key)];
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

    // ── Object method forwarding ──

    bool has(const std::string& key) const {
        if (is_object()) return std::get<JsObject>(inner).has(key);
        return false;
    }
    std::vector<std::string> keys() const {
        if (is_object()) return std::get<JsObject>(inner).keys();
        return {};
    }

private:
    static JsValue& get_mutable_dummy() {
        static JsValue dummy;
        return dummy;
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
    if (num.is_int()) {
        value = std::to_string(std::get<int64_t>(num.value));
    } else if (num.is_big()) {
        value = std::get<std::string>(num.value);
    } else {
        value = std::to_string(std::get<double>(num.value));
        auto dot = value.find('.');
        if (dot != std::string::npos) {
            auto last = value.find_last_not_of('0');
            if (last > dot) value = value.substr(0, last + 1);
            else value = value.substr(0, dot);
        }
    }
}

// ── JsNumber assignment from JsValue ──

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

inline JsValue JsArray::operator[](int64_t idx) const {
    if (idx < 0 || (size_t)idx >= elements->size()) return JsValue(JsUndefined{});
    return (*elements)[idx];
}

inline JsValue& JsArray::operator[](int64_t idx) {
    return (*elements)[idx];
}

inline JsValue JsArray::operator[](const JsNumber& idx) const {
    return (*this)[idx.as_int()];
}

inline JsValue& JsArray::operator[](const JsNumber& idx) {
    return (*this)[idx.as_int()];
}

// Default constructors live here (not in js_array.h / js_object.h) because
// make_shared<vector<JsValue>> / make_shared<map<string, JsValue>> need a
// complete JsValue — clang rejects the instantiation from the earlier point.
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

inline JsObject::JsObject()
    : properties(std::make_shared<std::map<std::string, JsValue>>()) {}

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
// Codegen adds <format> only when `${}` is used.
