#pragma once

// String helper functions for native std::string to mimic JavaScript string methods
// These are used when the compiler chooses native std::string over JsString

#include <string>
#include <vector>
#include <algorithm>
#include <cctype>
#include <regex>

namespace morph::str {

// Convert to uppercase
inline std::string to_upper(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(), ::toupper);
    return s;
}

// Convert to lowercase
inline std::string to_lower(std::string s) {
    std::transform(s.begin(), s.end(), s.begin(), ::tolower);
    return s;
}

// Get character at index (returns empty string if out of bounds)
inline std::string char_at(const std::string& s, int idx) {
    if (idx >= 0 && idx < (int)s.size()) {
        return std::string(1, s[idx]);
    }
    return std::string();
}

// Find index of substring (returns -1 if not found)
inline int index_of(const std::string& s, const std::string& search, int from = 0) {
    auto pos = s.find(search, from);
    return pos != std::string::npos ? (int)pos : -1;
}

// Find last index of substring
inline int last_index_of(const std::string& s, const std::string& search, int from = -1) {
    if (from < 0 || from >= (int)s.size()) from = s.size() - 1;
    auto pos = s.rfind(search, from);
    return pos != std::string::npos ? (int)pos : -1;
}

// Substring from start to end (exclusive)
inline std::string substring(const std::string& s, int start, int end) {
    if (start < 0) start = 0;
    if (start > (int)s.size()) start = s.size();
    if (end < 0 || end > (int)s.size()) end = s.size();
    if (start > end) std::swap(start, end);
    return s.substr(start, end - start);
}
inline std::string substring(const std::string& s, int start) {
    if (start < 0) start = 0;
    if (start > (int)s.size()) start = s.size();
    return s.substr(start);
}

// Substr from start with length
inline std::string substr(const std::string& s, int start, int len) {
    if (start < 0) start = 0;
    if (start > (int)s.size()) return std::string();
    if (len < 0) len = s.size() - start;
    return s.substr(start, len);
}
inline std::string substr(const std::string& s, int start) {
    if (start < 0) start = 0;
    if (start > (int)s.size()) return std::string();
    return s.substr(start);
}

// Slice (similar to substring but handles negative indices)
inline std::string slice(const std::string& s, int start, int end) {
    if (start < 0) start = s.size() + start;
    if (end < 0) end = s.size() + end;
    if (start < 0) start = 0;
    if (end < 0) end = 0;
    if (start > (int)s.size()) start = s.size();
    if (end > (int)s.size()) end = s.size();
    if (start > end) return std::string();
    return s.substr(start, end - start);
}
inline std::string slice(const std::string& s, int start) {
    if (start < 0) start = s.size() + start;
    if (start < 0) start = 0;
    if (start > (int)s.size()) start = s.size();
    return s.substr(start);
}

// Trim whitespace from both ends
inline std::string trim(std::string s) {
    const char* ws = " \t\n\r\f\v";
    size_t start = s.find_first_not_of(ws);
    if (start == std::string::npos) return std::string();
    size_t end = s.find_last_not_of(ws);
    return s.substr(start, end - start + 1);
}

// Trim from start
inline std::string trim_start(std::string s) {
    const char* ws = " \t\n\r\f\v";
    size_t start = s.find_first_not_of(ws);
    if (start == std::string::npos) return std::string();
    return s.substr(start);
}

// Trim from end
inline std::string trim_end(std::string s) {
    const char* ws = " \t\n\r\f\v";
    size_t end = s.find_last_not_of(ws);
    if (end == std::string::npos) return std::string();
    return s.substr(0, end + 1);
}

// Replace first occurrence
inline std::string replace(const std::string& s, const std::string& search, const std::string& replace) {
    if (search.empty()) return s;
    std::string result = s;
    size_t pos = result.find(search);
    if (pos != std::string::npos) {
        result.replace(pos, search.length(), replace);
    }
    return result;
}

// Replace all occurrences
inline std::string replace_all(const std::string& s, const std::string& search, const std::string& replace) {
    if (search.empty()) return s;
    std::string result = s;
    size_t pos = 0;
    while ((pos = result.find(search, pos)) != std::string::npos) {
        result.replace(pos, search.length(), replace);
        pos += replace.length();
    }
    return result;
}

// Split by separator
inline std::vector<std::string> split(const std::string& s, const std::string& sep) {
    std::vector<std::string> result;
    if (sep.empty()) {
        for (char c : s) result.emplace_back(1, c);
        return result;
    }
    size_t start = 0;
    size_t end = s.find(sep);
    while (end != std::string::npos) {
        result.push_back(s.substr(start, end - start));
        start = end + sep.length();
        end = s.find(sep, start);
    }
    result.push_back(s.substr(start));
    return result;
}

// Match regex (returns first match or empty vector)
inline std::vector<std::string> match_regex(const std::string& s, const std::string& regex) {
    try {
        std::regex re(regex);
        std::smatch match;
        if (std::regex_search(s, match, re)) {
            std::vector<std::string> result;
            for (size_t i = 0; i < match.size(); ++i) {
                result.push_back(match[i].str());
            }
            return result;
        }
    } catch (const std::regex_error&) {
        // Invalid regex, return empty
    }
    return {};
}

// Match all regex occurrences
inline std::vector<std::vector<std::string>> match_all(const std::string& s, const std::string& regex) {
    std::vector<std::vector<std::string>> result;
    try {
        std::regex re(regex);
        auto begin = std::sregex_iterator(s.begin(), s.end(), re);
        auto end = std::sregex_iterator();
        for (auto it = begin; it != end; ++it) {
            std::vector<std::string> match;
            for (size_t i = 0; i < it->size(); ++i) {
                match.push_back((*it)[i].str());
            }
            result.push_back(match);
        }
    } catch (const std::regex_error&) {
        // Invalid regex
    }
    return result;
}

// Search regex (returns position of first match, -1 if not found)
inline int search(const std::string& s, const std::string& regex) {
    try {
        std::regex re(regex);
        std::smatch match;
        if (std::regex_search(s, match, re)) {
            return (int)match.position();
        }
    } catch (const std::regex_error&) {
    }
    return -1;
}

// Pad start
inline std::string pad_start(const std::string& s, int len, const std::string& pad = " ") {
    if ((int)s.size() >= len) return s;
    std::string result;
    int pad_len = len - s.size();
    while ((int)result.size() < pad_len) result += pad;
    if ((int)result.size() > pad_len) result.resize(pad_len);
    return result + s;
}

// Pad end
inline std::string pad_end(const std::string& s, int len, const std::string& pad = " ") {
    if ((int)s.size() >= len) return s;
    std::string result = s;
    int pad_len = len - s.size();
    while ((int)result.size() < len) result += pad;
    if ((int)result.size() > len) result.resize(len);
    return result;
}

// Repeat string
inline std::string repeat(const std::string& s, int count) {
    if (count <= 0) return std::string();
    std::string result;
    result.reserve(s.size() * count);
    for (int i = 0; i < count; ++i) result += s;
    return result;
}

// Starts with
inline bool starts_with(const std::string& s, const std::string& prefix) {
    return s.rfind(prefix, 0) == 0;
}

// Ends with
inline bool ends_with(const std::string& s, const std::string& suffix) {
    if (s.size() < suffix.size()) return false;
    return s.compare(s.size() - suffix.size(), suffix.size(), suffix) == 0;
}

// Includes
inline bool includes(const std::string& s, const std::string& substr) {
    return s.find(substr) != std::string::npos;
}

// Locale compare (simplified)
inline int locale_compare(const std::string& a, const std::string& b) {
    if (a < b) return -1;
    if (a > b) return 1;
    return 0;
}

// Normalize (simplified - no-op)
inline std::string normalize(const std::string& s) {
    return s;
}

// To locale upper/lower (simplified - use standard)
inline std::string to_locale_upper(const std::string& s) {
    return to_upper(s);
}

inline std::string to_locale_lower(const std::string& s) {
    return to_lower(s);
}

// Convert number to string (for native number types)
template <typename T>
inline std::string to_string(T n) {
    return std::to_string(n);
}

} // namespace morph::str