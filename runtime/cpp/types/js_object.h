#pragma once
#include <algorithm>
#include <memory>
#include <unordered_map>
#include <string>
#include <vector>
#include <initializer_list>

struct JsValue;

struct JsObject {
    // shared_ptr for JS-like reference semantics. unordered_map: O(1)
    // hashed point lookups instead of O(log n) string tree compares.
    // Iteration order is unspecified (unlike the old std::map) — sites
    // that need the old sorted order use sorted_keys().
    std::shared_ptr<std::unordered_map<std::string, JsValue>> properties;

    // Defined in js_value.h AFTER JsValue is complete (clang requires it).
    JsObject();

    JsObject(std::initializer_list<std::pair<const char*, JsValue>> init);

    JsObject(const JsObject&) = default;
    JsObject& operator=(const JsObject&) = default;

    // ── Access ──

    JsValue get(const std::string& key) const;
    void set(const std::string& key, const JsValue& val);
    bool has(const std::string& key) const;

    // Drop every property. Shared ownership has no cycle collector, so a
    // reference cycle (a.self = a) lives until someone breaks an edge;
    // this is that edge-breaker. Same role as `= null` in JS runtimes.
    void clear();

    // Bracket access: obj["key"]
    JsValue operator[](const std::string& key) const;
    JsValue& operator[](const std::string& key);

    std::vector<std::string> keys() const;

    // Sorted keys: preserves the old std::map iteration order for
    // `for-in` and other order-sensitive enumeration.
    std::vector<std::string> sorted_keys() const;
};
