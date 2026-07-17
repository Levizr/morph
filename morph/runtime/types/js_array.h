#pragma once
#include <memory>
#include <vector>
#include <cstdint>
#include <initializer_list>

struct JsValue;
struct JsNumber;

struct JsArray {
    // shared_ptr for JS-like reference semantics (no deep copy on assignment)
    std::shared_ptr<std::vector<JsValue>> elements;

    JsArray() : elements(std::make_shared<std::vector<JsValue>>()) {}

    JsArray(std::initializer_list<JsValue> init);

    // Copy constructor / assignment — shared_ptr copies the pointer, not the data
    JsArray(const JsArray&) = default;
    JsArray& operator=(const JsArray&) = default;

    // ── Accessors ──

    size_t length() const { return elements->size(); }
    bool empty() const { return elements->empty(); }

    // ── JS Array methods ──

    void push(const JsValue& item);
    JsValue pop();

    JsValue operator[](int64_t idx) const;
    JsValue& operator[](int64_t idx);
    JsValue operator[](const JsNumber& idx) const;
    JsValue& operator[](const JsNumber& idx);
};
