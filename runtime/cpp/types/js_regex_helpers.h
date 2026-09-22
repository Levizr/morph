#pragma once

// Regex-backed string helpers (match/matchAll/search). Split out of
// js_string_helpers.h because <regex> + try/catch is incompatible with
// -fno-exceptions release builds — include only when these three
// methods are used (morpher inserts this header automatically).

#include <regex>
#include <string>
#include <vector>

namespace morph::strutil {

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

} // namespace morph::strutil
