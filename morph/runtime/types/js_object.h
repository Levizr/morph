#pragma once
#include <memory>
#include <map>
#include <string>
#include <vector>
#include <initializer_list>

struct JsValue;

struct JsObject {
    // shared_ptr for JS-like reference semantics
    std::shared_ptr<std::map<std::string, JsValue>> properties;

    JsObject() : properties(std::make_shared<std::map<std::string, JsValue>>()) {}

    JsObject(std::initializer_list<std::pair<const char*, JsValue>> init);

    JsObject(const JsObject&) = default;
    JsObject& operator=(const JsObject&) = default;

    // ── Access ──

    JsValue get(const std::string& key) const;
    void set(const std::string& key, const JsValue& val);
    bool has(const std::string& key) const;

    // Bracket access: obj["key"]
    JsValue operator[](const std::string& key) const;
    JsValue& operator[](const std::string& key);

    std::vector<std::string> keys() const;
};
