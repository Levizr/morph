#pragma once
#include <string>
#include <cstdio>
#include <cstdlib>
#include <stdexcept>
#include "js_value.h"

// JSON.parse / JSON.stringify over JsValue. parse throws a catchable
// SyntaxError JsValue on invalid input (translator lowers try/catch to
// catch (JsValue&)). Depth-capped, no reviver/replacer support.

namespace morph {

namespace js_json_detail {

inline constexpr int MAX_DEPTH = 100;

inline void skip_ws(const std::string& s, size_t& p) {
    while (p < s.size() && (s[p] == ' ' || s[p] == '\t' || s[p] == '\n' || s[p] == '\r')) ++p;
}

[[noreturn]] inline void fail(const std::string& msg) {
#ifdef __cpp_exceptions
    throw JsValue(JsObject{{"name", JsValue("SyntaxError")}, {"message", JsValue(msg)}});
#else
    (void)msg;
    std::abort();
#endif
}

inline void expect(const std::string& s, size_t& p, char c) {
    if (p >= s.size() || s[p] != c) fail("unexpected token in JSON");
    ++p;
}

inline void expect_lit(const std::string& s, size_t& p, const char* lit) {
    for (const char* q = lit; *q; ++q, ++p) {
        if (p >= s.size() || s[p] != *q) fail("unexpected token in JSON");
    }
}

inline void append_utf8(std::string& out, unsigned cp) {
    if (cp < 0x80) {
        out += (char)cp;
    } else if (cp < 0x800) {
        out += (char)(0xC0 | (cp >> 6));
        out += (char)(0x80 | (cp & 0x3F));
    } else if (cp < 0x10000) {
        out += (char)(0xE0 | (cp >> 12));
        out += (char)(0x80 | ((cp >> 6) & 0x3F));
        out += (char)(0x80 | (cp & 0x3F));
    } else {
        out += (char)(0xF0 | (cp >> 18));
        out += (char)(0x80 | ((cp >> 12) & 0x3F));
        out += (char)(0x80 | ((cp >> 6) & 0x3F));
        out += (char)(0x80 | (cp & 0x3F));
    }
}

inline unsigned hex4(const std::string& s, size_t& p) {
    if (p + 4 > s.size()) fail("bad unicode escape in JSON");
    unsigned cp = 0;
    for (int i = 0; i < 4; ++i) {
        char c = s[p++];
        cp <<= 4;
        if (c >= '0' && c <= '9') cp += (unsigned)(c - '0');
        else if (c >= 'a' && c <= 'f') cp += (unsigned)(c - 'a' + 10);
        else if (c >= 'A' && c <= 'F') cp += (unsigned)(c - 'A' + 10);
        else fail("bad unicode escape in JSON");
    }
    return cp;
}

inline std::string parse_string(const std::string& s, size_t& p) {
    expect(s, p, '"');
    std::string out;
    while (true) {
        if (p >= s.size()) fail("unterminated string in JSON");
        char c = s[p++];
        if (c == '"') break;
        if (c == '\\') {
            if (p >= s.size()) fail("unterminated string in JSON");
            char e = s[p++];
            if (e == '"') out += '"';
            else if (e == '\\') out += '\\';
            else if (e == '/') out += '/';
            else if (e == 'b') out += '\b';
            else if (e == 'f') out += '\f';
            else if (e == 'n') out += '\n';
            else if (e == 'r') out += '\r';
            else if (e == 't') out += '\t';
            else if (e == 'u') {
                unsigned cp = hex4(s, p);
                if (cp >= 0xD800 && cp <= 0xDBFF && p + 2 <= s.size() && s[p] == '\\' &&
                    s[p + 1] == 'u') {
                    size_t q = p + 2;
                    unsigned lo = hex4(s, q);
                    if (lo >= 0xDC00 && lo <= 0xDFFF) {
                        p = q;
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                    }
                }
                if (cp > 0x10FFFF) fail("bad unicode escape in JSON");
                append_utf8(out, cp);
            } else {
                fail("bad escape in JSON");
            }
        } else if ((unsigned char)c < 0x20) {
            fail("unescaped control in JSON");
        } else {
            out += c;
        }
    }
    return out;
}

inline JsValue parse_value(const std::string& s, size_t& p, int depth);

inline JsValue parse_array(const std::string& s, size_t& p, int depth) {
    if (depth > MAX_DEPTH) fail("JSON too deeply nested");
    expect(s, p, '[');
    JsArray arr;
    skip_ws(s, p);
    if (p < s.size() && s[p] == ']') {
        ++p;
        return JsValue(arr);
    }
    while (true) {
        arr.elements->push_back(parse_value(s, p, depth + 1));
        skip_ws(s, p);
        if (p < s.size() && s[p] == ',') {
            ++p;
            continue;
        }
        expect(s, p, ']');
        break;
    }
    return JsValue(arr);
}

inline JsValue parse_object(const std::string& s, size_t& p, int depth) {
    if (depth > MAX_DEPTH) fail("JSON too deeply nested");
    expect(s, p, '{');
    JsObject obj;
    skip_ws(s, p);
    if (p < s.size() && s[p] == '}') {
        ++p;
        return JsValue(obj);
    }
    while (true) {
        skip_ws(s, p);
        std::string key = parse_string(s, p);
        skip_ws(s, p);
        expect(s, p, ':');
        obj.set(key, parse_value(s, p, depth + 1));
        skip_ws(s, p);
        if (p < s.size() && s[p] == ',') {
            ++p;
            continue;
        }
        expect(s, p, '}');
        break;
    }
    return JsValue(obj);
}

inline JsValue parse_number(const std::string& s, size_t& p) {
    size_t start = p;
    if (p < s.size() && s[p] == '-') ++p;
    if (p >= s.size()) fail("bad number in JSON");
    if (s[p] == '0') {
        ++p;
    } else if (s[p] >= '1' && s[p] <= '9') {
        while (p < s.size() && s[p] >= '0' && s[p] <= '9') ++p;
    } else {
        fail("bad number in JSON");
    }
    bool frac = false;
    if (p < s.size() && s[p] == '.') {
        frac = true;
        ++p;
        if (p >= s.size() || s[p] < '0' || s[p] > '9') fail("bad number in JSON");
        while (p < s.size() && s[p] >= '0' && s[p] <= '9') ++p;
    }
    if (p < s.size() && (s[p] == 'e' || s[p] == 'E')) {
        frac = true;
        ++p;
        if (p < s.size() && (s[p] == '+' || s[p] == '-')) ++p;
        if (p >= s.size() || s[p] < '0' || s[p] > '9') fail("bad number in JSON");
        while (p < s.size() && s[p] >= '0' && s[p] <= '9') ++p;
    }
    std::string tok = s.substr(start, p - start);
    if (!frac) {
        size_t digits = tok.size() - (tok[0] == '-' ? 1 : 0);
        if (digits >= 1 && digits <= 18) {
            // Validated digits that fit: stoll cannot throw here.
            long long i = 0;
            size_t k = (tok[0] == '-') ? 1 : 0;
            for (; k < tok.size(); ++k) i = i * 10 + (tok[k] - '0');
            if (tok[0] == '-') i = -i;
            return JsValue(JsNumber((int64_t)i));
        }
    }
    char* end = nullptr;
    double d = std::strtod(tok.c_str(), &end);
    if (end != tok.c_str() + tok.size()) fail("bad number in JSON");
    return JsValue(JsNumber(d));
}

inline JsValue parse_value(const std::string& s, size_t& p, int depth) {
    if (depth > MAX_DEPTH) fail("JSON too deeply nested");
    skip_ws(s, p);
    if (p >= s.size()) fail("unexpected end in JSON");
    char c = s[p];
    if (c == '"') return JsValue(JsString(parse_string(s, p)));
    if (c == '{') return parse_object(s, p, depth);
    if (c == '[') return parse_array(s, p, depth);
    if (c == 't') {
        expect_lit(s, p, "true");
        return JsValue(true);
    }
    if (c == 'f') {
        expect_lit(s, p, "false");
        return JsValue(false);
    }
    if (c == 'n') {
        expect_lit(s, p, "null");
        return JsValue(JsNull{});
    }
    if (c == '-' || (c >= '0' && c <= '9')) return parse_number(s, p);
    fail("unexpected token in JSON");
    return JsValue(JsUndefined{});
}

inline void escape_into(std::string& out, const std::string& s) {
    static const char* hexd = "0123456789abcdef";
    out += '"';
    for (unsigned char c : s) {
        if (c == '"') out += "\\\"";
        else if (c == '\\') out += "\\\\";
        else if (c == '\b') out += "\\b";
        else if (c == '\f') out += "\\f";
        else if (c == '\n') out += "\\n";
        else if (c == '\r') out += "\\r";
        else if (c == '\t') out += "\\t";
        else if (c < 0x20) {
            out += "\\u00";
            out += hexd[c >> 4];
            out += hexd[c & 15];
        } else {
            out += (char)c;
        }
    }
    out += '"';
}

inline std::string number_text(const JsNumber& n) {
    if (n.is_int()) return std::to_string(std::get<int64_t>(n.value));
    double d = n.as_double();
    if (d != d || !std::isfinite(d)) return "null";
    if (d == 0.0) return "0";
    return JsNumber::js_double_string(d);
}

inline bool stringify_into(std::string& out, const JsValue& v, int depth) {
    if (depth > MAX_DEPTH) fail("JSON too deeply nested");
    if (v.is_undefined() || v.is_function()) return false;
    if (v.is_null()) {
        out += "null";
        return true;
    }
    if (v.is_boolean()) {
        out += std::get<JsBoolean>(v.inner).value ? "true" : "false";
        return true;
    }
    if (v.is_number()) {
        out += number_text(std::get<JsNumber>(v.inner));
        return true;
    }
    if (v.is_string()) {
        escape_into(out, std::get<JsString>(v.inner).value);
        return true;
    }
    if (v.is_array()) {
        const auto& arr = std::get<JsArray>(v.inner);
        out += '[';
        for (size_t i = 0; i < arr.length(); ++i) {
            if (i > 0) out += ',';
            if (!stringify_into(out, arr[(int64_t)i], depth + 1)) out += "null";
        }
        out += ']';
        return true;
    }
    if (v.is_object()) {
        const auto& obj = std::get<JsObject>(v.inner);
        out += '{';
        bool first = true;
        for (const auto& key : obj.sorted_keys()) {
            const JsValue& val = obj.get(key);
            if (val.is_undefined() || val.is_function()) continue;
            if (!first) out += ',';
            first = false;
            escape_into(out, key);
            out += ':';
            if (!stringify_into(out, val, depth + 1)) out += "null";
        }
        out += '}';
        return true;
    }
    return false;
}

} // namespace js_json_detail

inline JsValue json_parse(const std::string& text) {
    size_t p = 0;
    JsValue v = js_json_detail::parse_value(text, p, 0);
    js_json_detail::skip_ws(text, p);
    if (p != text.size()) js_json_detail::fail("unexpected trailing text in JSON");
    return v;
}

inline JsValue json_parse(const JsString& text) { return json_parse(text.value); }
inline JsValue json_parse(const char* text) { return json_parse(std::string(text ? text : "")); }
inline JsValue json_parse(const JsValue& v) {
    if (v.is_string()) return json_parse(std::get<JsString>(v.inner).value);
    return json_parse(_js_to_string(v));
}

inline JsValue json_stringify(const JsValue& v) {
    std::string out;
    if (!js_json_detail::stringify_into(out, v, 0)) return JsValue(JsUndefined{});
    return JsValue(JsString(out));
}

} // namespace morph
