#pragma once
#include <string>
#include <vector>
#include <unordered_map>
#include <stdexcept>
#include <cstdlib>
#include <cstdint>

enum class JsonType { Null, Bool, Number, String, Array, Object };

class JsonValue {
public:
    JsonValue() : m_type(JsonType::Null), m_num(0), m_bool(false) {}

    static JsonValue parse(const std::string& src) {
        size_t pos = 0;
        return parseValue(src, pos);
    }

    JsonType type() const { return m_type; }

    bool isNull() const { return m_type == JsonType::Null; }
    bool asBool() const { return m_bool; }
    double asDouble() const { return m_num; }
    float asFloat() const { return (float)m_num; }
    int asInt() const { return (int)m_num; }

    const std::string& asString() const { return m_str; }

    const JsonValue& operator[](const std::string& key) const {
        static JsonValue nullVal;
        for (auto& [k, v] : m_obj)
            if (k == key) return v;
        return nullVal;
    }

    const JsonValue& operator[](size_t i) const {
        return m_arr[i];
    }

    size_t size() const {
        if (m_type == JsonType::Array) return m_arr.size();
        if (m_type == JsonType::Object) return m_obj.size();
        return 0;
    }

    bool has(const std::string& key) const {
        for (auto& [k, v] : m_obj)
            if (k == key) return true;
        return false;
    }

    // Object iteration: key at index i (requires size() > i).
    const std::string& key(size_t i) const { return m_obj[i].first; }

    // Object iteration: value at index i (requires size() > i).  Note:
    // operator[](size_t) indexes the array storage, so objects MUST use
    // key()/value() for iteration.
    const JsonValue& value(size_t i) const { return m_obj[i].second; }

    // Iteration helpers for array
    using ArrayIter = std::vector<JsonValue>::const_iterator;
    ArrayIter begin() const { return m_arr.begin(); }
    ArrayIter end() const { return m_arr.end(); }

private:
    JsonType m_type;
    double m_num;
    bool m_bool;
    std::string m_str;
    std::vector<JsonValue> m_arr;
    std::vector<std::pair<std::string, JsonValue>> m_obj;

    static void appendUtf8(std::string& s, uint32_t cp) {
        if (cp < 0x80) {
            s += (char)cp;
        } else if (cp < 0x800) {
            s += (char)(0xC0 | (cp >> 6));
            s += (char)(0x80 | (cp & 0x3F));
        } else if (cp < 0x10000) {
            s += (char)(0xE0 | (cp >> 12));
            s += (char)(0x80 | ((cp >> 6) & 0x3F));
            s += (char)(0x80 | (cp & 0x3F));
        } else if (cp < 0x110000) {
            s += (char)(0xF0 | (cp >> 18));
            s += (char)(0x80 | ((cp >> 12) & 0x3F));
            s += (char)(0x80 | ((cp >> 6) & 0x3F));
            s += (char)(0x80 | (cp & 0x3F));
        }
    }

    static void skipWS(const std::string& s, size_t& p) {
        while (p < s.size() && (s[p] == ' ' || s[p] == '\t' || s[p] == '\n' || s[p] == '\r'))
            p++;
    }

    static JsonValue parseValue(const std::string& s, size_t& p) {
        skipWS(s, p);
        if (p >= s.size()) throw std::runtime_error("Unexpected end of JSON");
        char c = s[p];
        if (c == '"') return parseString(s, p);
        if (c == '{') return parseObject(s, p);
        if (c == '[') return parseArray(s, p);
        if (c == 't' || c == 'f') return parseBool(s, p);
        if (c == 'n') return parseNull(s, p);
        if (c == '-' || (c >= '0' && c <= '9')) return parseNumber(s, p);
        throw std::runtime_error(std::string("Unexpected char: ") + c);
    }

    static JsonValue parseString(const std::string& s, size_t& p) {
        p++; // skip "
        std::string result;
        while (p < s.size()) {
            char c = s[p];
            if (c == '"') { p++; break; }
            if (c == '\\') {
                p++;
                if (p >= s.size()) break;
                switch (s[p]) {
                    case '"':  result += '"'; break;
                    case '\\': result += '\\'; break;
                    case '/':  result += '/'; break;
                    case 'b':  result += '\b'; break;
                    case 'f':  result += '\f'; break;
                    case 'n':  result += '\n'; break;
                    case 'r':  result += '\r'; break;
                    case 't':  result += '\t'; break;
                    case 'u': {
                        if (p + 4 >= s.size()) break;
                        std::string hex = s.substr(p + 1, 4);
                        uint32_t cp = (uint32_t)strtol(hex.c_str(), nullptr, 16);
                        p += 4;
                        if (cp >= 0xD800 && cp <= 0xDBFF) {
                            if (p + 6 < s.size() && s[p] == '\\' && s[p+1] == 'u') {
                                std::string loHex = s.substr(p + 2, 4);
                                uint32_t lo = (uint32_t)strtol(loHex.c_str(), nullptr, 16);
                                if (lo >= 0xDC00 && lo <= 0xDFFF) {
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                    p += 6;
                                }
                            }
                        }
                        appendUtf8(result, cp);
                        break;
                    }
                    default: result += s[p]; break;
                }
            } else {
                result += c;
            }
            p++;
        }
        JsonValue v;
        v.m_type = JsonType::String;
        v.m_str = result;
        return v;
    }

    static JsonValue parseNumber(const std::string& s, size_t& p) {
        size_t start = p;
        if (s[p] == '-') p++;
        while (p < s.size() && s[p] >= '0' && s[p] <= '9') p++;
        if (p < s.size() && s[p] == '.') {
            p++;
            while (p < s.size() && s[p] >= '0' && s[p] <= '9') p++;
        }
        if (p < s.size() && (s[p] == 'e' || s[p] == 'E')) {
            p++;
            if (p < s.size() && (s[p] == '+' || s[p] == '-')) p++;
            while (p < s.size() && s[p] >= '0' && s[p] <= '9') p++;
        }
        double val = strtod(s.c_str() + start, nullptr);
        JsonValue v;
        v.m_type = JsonType::Number;
        v.m_num = val;
        return v;
    }

    static JsonValue parseBool(const std::string& s, size_t& p) {
        if (s.compare(p, 4, "true") == 0) {
            p += 4;
            JsonValue v;
            v.m_type = JsonType::Bool;
            v.m_bool = true;
            return v;
        }
        if (s.compare(p, 5, "false") == 0) {
            p += 5;
            JsonValue v;
            v.m_type = JsonType::Bool;
            v.m_bool = false;
            return v;
        }
        throw std::runtime_error("Invalid boolean");
    }

    static JsonValue parseNull(const std::string& s, size_t& p) {
        if (s.compare(p, 4, "null") == 0) {
            p += 4;
            return JsonValue();
        }
        throw std::runtime_error("Invalid null");
    }

    static JsonValue parseArray(const std::string& s, size_t& p) {
        p++; // skip [
        JsonValue v;
        v.m_type = JsonType::Array;
        while (p < s.size()) {
            skipWS(s, p);
            if (p >= s.size()) break;
            if (s[p] == ']') { p++; break; }
            if (!v.m_arr.empty()) {
                if (s[p] != ',') throw std::runtime_error("Expected ',' in array");
                p++;
                skipWS(s, p);
            }
            v.m_arr.push_back(parseValue(s, p));
        }
        return v;
    }

    static JsonValue parseObject(const std::string& s, size_t& p) {
        p++; // skip {
        JsonValue v;
        v.m_type = JsonType::Object;
        while (p < s.size()) {
            skipWS(s, p);
            if (p >= s.size()) break;
            if (s[p] == '}') { p++; break; }
            if (!v.m_obj.empty()) {
                if (s[p] != ',') throw std::runtime_error("Expected ',' in object");
                p++;
                skipWS(s, p);
            }
            if (s[p] != '"') throw std::runtime_error("Expected string key in object");
            auto keyVal = parseString(s, p);
            skipWS(s, p);
            if (p >= s.size() || s[p] != ':') throw std::runtime_error("Expected ':' in object");
            p++;
            auto val = parseValue(s, p);
            v.m_obj.push_back({keyVal.m_str, val});
        }
        return v;
    }
};
