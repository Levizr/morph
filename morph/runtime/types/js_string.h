#pragma once
#include <string>
#include <algorithm>
#include <cctype>
#include <vector>
struct JsNumber;

struct JsString {
    std::string value;

    JsString() : value() {}
    JsString(char* v) : value(std::string(v)) {}
    JsString(const char* v) : value(v) {}
    JsString(std::string v) : value(std::move(v)) {}
    JsString(const JsNumber& num);  // defined in js_value.h after JsNumber exists

    // ── Accessors ──

    size_t length() const { return value.length(); }
    bool empty() const { return value.empty(); }

    // ── JS String methods ──

    JsString toUpperCase() const {
        std::string out = value;
        for (auto& c : out) c = char(std::toupper(static_cast<unsigned char>(c)));
        return out;
    }

    JsString toLowerCase() const {
        std::string out = value;
        for (auto& c : out) c = char(std::tolower(static_cast<unsigned char>(c)));
        return out;
    }

    std::string to_std_string() const {
        std::string out = value;
        return out;
    }

    JsString substring(int64_t start, int64_t end = -1) const {
        size_t s = std::max(int64_t(0), std::min(int64_t(value.size()), start));
        size_t e = (end < 0) ? value.size() : std::max(int64_t(0), std::min(int64_t(value.size()), end));
        if (s > e) std::swap(s, e);
        return value.substr(s, e - s);
    }

    JsString slice(int64_t start, int64_t end = -1) const {
        int64_t len = int64_t(value.size());
        int64_t s = (start < 0) ? std::max(int64_t(0), len + start) : std::min(len, start);
        int64_t e = (end < 0) ? len : std::min(len, end);
        if (s >= e) return JsString();
        return value.substr(size_t(s), size_t(e - s));
    }

    JsString charAt(int64_t idx) const {
        if (idx < 0 || size_t(idx) >= value.size()) return JsString();
        return std::string(1, value[size_t(idx)]);
    }

    int64_t indexOf(const JsString& sub, int64_t from = 0) const {
        auto pos = value.find(sub.value, size_t(from));
        return (pos == std::string::npos) ? -1 : int64_t(pos);
    }

    JsString replace(const JsString& search, const JsString& replacement) const {
        std::string out = value;
        size_t pos = 0;
        while ((pos = out.find(search.value, pos)) != std::string::npos) {
            out.replace(pos, search.value.length(), replacement.value);
            pos += replacement.value.length();
        }
        return out;
    }

    JsString trim() const {
        auto s = value;
        auto not_space = [](unsigned char c) { return !std::isspace(c); };
        s.erase(s.begin(), std::find_if(s.begin(), s.end(), not_space));
        s.erase(std::find_if(s.rbegin(), s.rend(), not_space).base(), s.end());
        return s;
    }

    // Simplified split — returns vector, not JsArray (avoids circular dep)
    std::vector<JsString> split(const JsString& delim) const;

    // ── Operators ──

    JsString operator+(const JsString& o) const { return value + o.value; }
    JsString& operator+=(const JsString& o) { value += o.value; return *this; }

    // Indexing: str[i] returns a single-char string (JS semantics)
    JsString operator[](size_t idx) const {
        if (idx >= value.size()) return JsString();
        return std::string(1, value[idx]);
    }

    bool operator==(const JsString& o) const { return value == o.value; }
    bool operator!=(const JsString& o) const { return value != o.value; }
    bool operator<(const JsString& o) const { return value < o.value; }

    JsString toString() const { return value; }
    operator std::string() const { return value; }
};

// ── Mixed-type comparison operators ──

inline bool operator==(const std::string& a, const JsString& b) { return a == b.value; }
inline bool operator==(const JsString& a, const std::string& b) { return a.value == b; }
inline bool operator!=(const std::string& a, const JsString& b) { return a != b.value; }
inline bool operator!=(const JsString& a, const std::string& b) { return a.value != b; }

// ── std::formatter for std::format ──
// Guarded so older libc++ (macOS Xcode <= 16) still compiles.
// std::formatter<JsString> moved to the opt-in types/js_value_format.h
// (<format> costs ~1.5s parse per TU — bad for dev hot-reload).
